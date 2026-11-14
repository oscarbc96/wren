// Copyright 2024 Obscura contributors (Apache-2.0)
// Modified for Wren: re-targeted to the CdpBackend trait.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

use anyhow::Context;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

use crate::backend::{CdpBackend, CdpBackendFactory, CdpFailure};
use crate::http;
use crate::session::{self, SessionState};
use crate::types::{CdpRequest, CdpResponse};

const MAX_CONNECTIONS: usize = 128;

pub struct CdpHandle {
    port: u16,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl CdpHandle {
    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for CdpHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // do not join in drop to avoid blocking unexpectedly if already stopped
    }
}

pub struct CdpServer;

impl CdpServer {
    pub fn start(
        port: u16,
        backend_factory: Arc<dyn CdpBackendFactory>,
    ) -> anyhow::Result<CdpHandle> {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_for_thread = stop.clone();
        let (tx, rx) = std::sync::mpsc::channel::<anyhow::Result<u16>>();

        let thread = std::thread::Builder::new()
            .name("wren-cdp".into())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        let _ = tx.send(Err(e.into()));
                        return;
                    }
                };
                let local = tokio::task::LocalSet::new();
                local.block_on(&rt, async move {
                    let addr = SocketAddr::from(([127, 0, 0, 1], port));
                    let listener = match TcpListener::bind(addr).await {
                        Ok(l) => l,
                        Err(e) => {
                            let _ = tx.send(Err(e.into()));
                            return;
                        }
                    };
                    let actual_port = match listener.local_addr() {
                        Ok(a) => a.port(),
                        Err(e) => {
                            let _ = tx.send(Err(e.into()));
                            return;
                        }
                    };
                    let _ = tx.send(Ok(actual_port));
                    let live = Arc::new(std::sync::atomic::AtomicUsize::new(0));
                    loop {
                        if stop_for_thread.load(Ordering::Relaxed) {
                            break;
                        }
                        let accept = tokio::time::timeout(
                            std::time::Duration::from_millis(100),
                            listener.accept(),
                        )
                        .await;
                        let (stream, _) = match accept {
                            Ok(Ok(s)) => s,
                            Ok(Err(_)) => continue,
                            Err(_) => continue,
                        };
                        if live.load(Ordering::Acquire) >= MAX_CONNECTIONS {
                            drop(stream);
                            continue;
                        }
                        live.fetch_add(1, Ordering::AcqRel);
                        let bf = backend_factory.clone();
                        let stop_c = stop_for_thread.clone();
                        let live_c = live.clone();
                        tokio::task::spawn_local(async move {
                            let _ = handle_connection(stream, bf, stop_c, actual_port).await;
                            live_c.fetch_sub(1, Ordering::AcqRel);
                        });
                    }
                });
            })
            .context("spawn cdp thread")?;

        let actual_port = rx.recv().context("cdp startup channel closed")??;
        Ok(CdpHandle {
            port: actual_port,
            stop,
            thread: Some(thread),
        })
    }
}

async fn handle_connection(
    stream: TcpStream,
    backend_factory: Arc<dyn CdpBackendFactory>,
    stop: Arc<AtomicBool>,
    port: u16,
) -> anyhow::Result<()> {
    let backend = backend_factory.create();
    // Peek/read request head
    let mut buf = vec![0u8; 8192];
    let n = stream.peek(&mut buf).await.unwrap_or(0);
    if n == 0 {
        return Ok(());
    }
    let head = String::from_utf8_lossy(&buf[..n]).to_string();
    let request_line = head.lines().next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");

    // Host/origin guards apply to all HTTP requests including upgrades
    if !http::host_allowed(&head, port) {
        let mut consume = vec![0u8; n.min(8192)];
        let _ = stream.try_read(&mut consume);
        http::write_forbidden(stream, "request refused").await?;
        return Ok(());
    }
    if !http::origin_allowed(&head, port) {
        let mut consume = vec![0u8; n.min(8192)];
        let _ = stream.try_read(&mut consume);
        http::write_forbidden(stream, "request refused").await?;
        return Ok(());
    }

    if method == "GET" && http::is_json_endpoint(target) {
        // consume request
        let mut consume = vec![0u8; n];
        let _ = stream.try_read(&mut consume);
        let targets = backend.targets().await;
        let body = http::json_response(target, port, &targets);
        http::write_json(stream, &body).await?;
        return Ok(());
    }
    if method == "PUT" && target.starts_with("/json/new") {
        let _ = stream.try_read(&mut vec![0u8; n]);
        // forward to backend as Target.createTarget? plan states endpoints exist;
        // for task 1 tests don't exercise these, return 404/501-ish minimal.
        // But better to try forwarding if possible.
        let url = http::new_target_url(target).unwrap_or_else(|| "about:blank".to_string());
        let res = backend
            .call(None, "Target.createTarget", json!({"url": url}))
            .await;
        match res {
            Ok(v) => {
                let id = v.get("targetId").and_then(|x| x.as_str()).unwrap_or("");
                let targets = backend.targets().await;
                if let Some(t) = targets.iter().find(|t| t.target_id == id) {
                    let body = http::target_json(port, t);
                    http::write_json(stream, &body).await?;
                } else {
                    http::write_json(stream, &v.to_string()).await?;
                }
            }
            Err(e) => {
                http::write_json(stream, &json!({"error": e.message}).to_string()).await?;
            }
        }
        return Ok(());
    }
    if method == "GET" && target.starts_with("/json/close/") {
        let _ = stream.try_read(&mut vec![0u8; n]);
        let id = target.trim_start_matches("/json/close/").to_string();
        let _ = backend
            .call(None, "Target.closeTarget", json!({"targetId": id}))
            .await;
        http::write_text(stream, "Target is closing").await?;
        return Ok(());
    }

    // WebSocket upgrade
    if !http::is_websocket_upgrade(&head) {
        return Ok(());
    }
    let is_browser = target.starts_with("/devtools/browser/");
    let page_id = if target.starts_with("/devtools/page/") {
        Some(target.trim_start_matches("/devtools/page/").to_string())
    } else {
        None
    };
    let ws = tokio_tungstenite::accept_async(stream).await?;
    run_ws(ws, backend, stop, port, is_browser, page_id).await
}

async fn run_ws(
    ws: tokio_tungstenite::WebSocketStream<TcpStream>,
    backend: Arc<dyn CdpBackend>,
    stop: Arc<AtomicBool>,
    _port: u16,
    _is_browser: bool,
    default_page: Option<String>,
) -> anyhow::Result<()> {
    let (mut write, mut read) = ws.split();
    let mut state = SessionState::new();
    if let Some(pid) = default_page {
        let sid = format!("page-session-{pid}");
        state.attach(&sid, &pid);
        backend.set_attached(&pid, true);
        state.default_session = Some(sid);
    }

    let mut backend_events = backend.events();
    'conn: loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        tokio::select! {
            msg = read.next() => {
                let msg = match msg {
                    Some(Ok(m)) => m,
                    _ => break,
                };
                if let Message::Text(text) = msg {
                    if let Ok(req) = serde_json::from_str::<CdpRequest>(&text) {
                        let responses = handle_request(&backend, &mut state, req).await;
                        for r in responses {
                            let s = serde_json::to_string(&r).unwrap_or_default();
                            if write.send(Message::Text(s.into())).await.is_err() { break 'conn; }
                        }
                    }
                } else if let Message::Close(_) = msg {
                    break;
                }
            }
            ev = backend_events.recv() => {
                if let Ok(ev) = ev {
                    for out in session::events_from_backend(&mut state, ev) {
                        let s = serde_json::to_string(&out).unwrap_or_default();
                        if write.send(Message::Text(s.into())).await.is_err() { break 'conn; }
                    }
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {}
        }
    }
    // The client is gone: its targets are no longer attached through it.
    let targets: std::collections::HashSet<String> = state.sessions.values().cloned().collect();
    for tid in targets {
        backend.set_attached(&tid, false);
    }
    Ok(())
}

async fn handle_request(
    backend: &Arc<dyn CdpBackend>,
    state: &mut SessionState,
    req: CdpRequest,
) -> Vec<serde_json::Value> {
    let session_id = req
        .session_id
        .clone()
        .or_else(|| state.default_session.clone());
    // Target domain handled in cdp itself
    if req.method.starts_with("Target.") || req.method.starts_with("Browser.") {
        return session::handle_target_browser(backend, state, &req).await;
    }
    let target_id = session_id
        .as_ref()
        .and_then(|s| state.target_for_session(s))
        .map(|s| s.to_string());
    let result = backend
        .call(target_id.as_deref(), &req.method, req.params.clone())
        .await;
    let resp = match result {
        Ok(v) => CdpResponse::success(req.id, v, session_id),
        Err(CdpFailure { code, message }) => CdpResponse::error(req.id, code, message, session_id),
    };
    vec![serde_json::to_value(resp).unwrap()]
}
