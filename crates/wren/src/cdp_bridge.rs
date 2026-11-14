use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use cdp::backend::{BackendEvent, CdpBackend, CdpBackendFactory, CdpFailure, TargetInfo};
use cdp::server::{CdpHandle, CdpServer};
use engine::{ConsoleLevel, Engine, EngineEvent, LoadState, WebViewId};
use serde_json::{Value, json};
use tabs::{TabId, TabStrip};
use tokio::sync::{broadcast, oneshot};
use url::Url;

pub(crate) const CALL_TIMEOUT: Duration = Duration::from_secs(30);

struct CdpCommand {
    target_id: Option<String>,
    method: String,
    params: Value,
    reply: oneshot::Sender<Result<Value, CdpFailure>>,
}

struct PendingCall {
    rx: std::sync::mpsc::Receiver<Result<Value, CdpFailure>>,
    reply: Option<oneshot::Sender<Result<Value, CdpFailure>>>,
    deadline: Instant,
    target_id: Option<String>,
}

struct SharedState {
    targets: Mutex<Vec<TargetInfo>>,
    cmd_tx: mpsc::Sender<CdpCommand>,
    events_tx: broadcast::Sender<BackendEvent>,
    quit_requested: AtomicBool,
    attached_updates: Mutex<Vec<(String, bool)>>,
}

#[derive(Clone)]
struct WrenCdpBackend {
    shared: Arc<SharedState>,
}

#[async_trait(?Send)]
impl CdpBackend for WrenCdpBackend {
    async fn targets(&self) -> Vec<TargetInfo> {
        self.shared.targets.lock().unwrap().clone()
    }

    async fn call(
        &self,
        target_id: Option<&str>,
        method: &str,
        params: Value,
    ) -> Result<Value, CdpFailure> {
        let (tx, rx) = oneshot::channel();
        let cmd = CdpCommand {
            target_id: target_id.map(str::to_string),
            method: method.to_string(),
            params,
            reply: tx,
        };
        if self.shared.cmd_tx.send(cmd).is_err() {
            return Err(CdpFailure {
                code: -32000,
                message: "bridge closed".to_string(),
            });
        }
        rx.await.unwrap_or_else(|_| {
            Err(CdpFailure {
                code: -32000,
                message: "bridge closed".to_string(),
            })
        })
    }

    fn events(&self) -> broadcast::Receiver<BackendEvent> {
        self.shared.events_tx.subscribe()
    }

    fn set_attached(&self, target_id: &str, attached: bool) {
        self.shared
            .attached_updates
            .lock()
            .unwrap()
            .push((target_id.to_string(), attached));
        let mut targets = self.shared.targets.lock().unwrap();
        if let Some(t) = targets.iter_mut().find(|t| t.target_id == target_id) {
            t.attached = attached;
        }
    }
}

struct WrenCdpFactory {
    shared: Arc<SharedState>,
}

impl CdpBackendFactory for WrenCdpFactory {
    fn create(&self) -> Arc<dyn CdpBackend> {
        Arc::new(WrenCdpBackend {
            shared: self.shared.clone(),
        })
    }
}

#[derive(Default, Clone)]
pub(crate) struct TargetDomainState {
    pub page_enabled: bool,
    pub runtime_enabled: bool,
    pub dom_enabled: bool,
    pub network_enabled: bool,
    pub next_context_id: i64,
    pub context_id: i64,
    pub loader_id: String,
    pub frame_url: String,
    pub next_loader: u64,
}

pub struct CdpBridge {
    handle: Option<CdpHandle>,
    port: u16,
    cmd_rx: mpsc::Receiver<CdpCommand>,
    shared: Arc<SharedState>,
    known: HashMap<TabId, (String, String)>,
    domains: HashMap<String, TargetDomainState>,
    pending: Vec<PendingCall>,
}

impl CdpBridge {
    pub fn start(port: u16) -> anyhow::Result<Self> {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (events_tx, _) = broadcast::channel(256);
        let shared = Arc::new(SharedState {
            targets: Mutex::new(Vec::new()),
            cmd_tx,
            events_tx,
            quit_requested: AtomicBool::new(false),
            attached_updates: Mutex::new(Vec::new()),
        });
        let factory = Arc::new(WrenCdpFactory {
            shared: shared.clone(),
        });
        let handle = CdpServer::start(port, factory)?;
        let actual_port = handle.port();
        Ok(Self {
            handle: Some(handle),
            port: actual_port,
            cmd_rx,
            shared,
            known: HashMap::new(),
            domains: HashMap::new(),
            pending: Vec::new(),
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn take_quit_requested(&self) -> bool {
        self.shared.quit_requested.swap(false, Ordering::SeqCst)
    }

    pub fn pump(&mut self, engine: &mut dyn Engine, tabs: &mut TabStrip, events: &[EngineEvent]) {
        // Apply attach state changes from cdp sessions
        let updates: Vec<(String, bool)> = {
            let mut u = self.shared.attached_updates.lock().unwrap();
            std::mem::take(&mut *u)
        };
        for (target_id, attached) in updates {
            if let Some(id) = target_to_tab(&target_id) {
                tabs.set_attached(id, attached);
            }
        }

        // Drain commands
        while let Ok(cmd) = self.cmd_rx.try_recv() {
            self.dispatch(engine, tabs, cmd);
        }

        // Map engine events to CDP domain events
        self.emit_domain_events(tabs, events);

        // Complete pending
        let mut i = 0;
        while i < self.pending.len() {
            let mut done: Option<Result<Value, CdpFailure>> = None;
            if let Ok(res) = self.pending[i].rx.try_recv() {
                done = Some(res);
            } else if Instant::now() >= self.pending[i].deadline {
                done = Some(Err(CdpFailure {
                    code: -32000,
                    message: "timeout".to_string(),
                }));
            }
            if let Some(res) = done {
                let mut p = self.pending.remove(i);
                if let Some(reply) = p.reply.take() {
                    let _ = reply.send(res);
                }
            } else {
                i += 1;
            }
        }

        // Refresh targets cache and emit diff events
        let mut current_targets: Vec<TargetInfo> = Vec::new();
        let mut current_map: HashMap<TabId, (String, String, bool)> = HashMap::new();
        for tab in tabs.tabs() {
            let target_id = tab_to_target(tab.id);
            let info = TargetInfo {
                target_id: target_id.clone(),
                url: tab.url.to_string(),
                title: tab.title.clone(),
                attached: tab.attached,
            };
            current_targets.push(info);
            current_map.insert(
                tab.id,
                (tab.url.to_string(), tab.title.clone(), tab.attached),
            );
        }
        *self.shared.targets.lock().unwrap() = current_targets;

        let mut to_remove = Vec::new();
        for (id, (url, title)) in &self.known {
            if !current_map.contains_key(id) {
                to_remove.push(*id);
                let _ = self
                    .shared
                    .events_tx
                    .send(BackendEvent::TargetDestroyed(tab_to_target(*id)));
            } else if let Some((nurl, ntitle, _)) = current_map.get(id)
                && (nurl != url || ntitle != title)
            {
                let _ = self
                    .shared
                    .events_tx
                    .send(BackendEvent::TargetChanged(TargetInfo {
                        target_id: tab_to_target(*id),
                        url: nurl.clone(),
                        title: ntitle.clone(),
                        attached: current_map.get(id).map(|x| x.2).unwrap_or(false),
                    }));
            }
        }
        for id in to_remove {
            self.known.remove(&id);
            self.domains.remove(&tab_to_target(id));
        }
        for (id, (url, title, attached)) in &current_map {
            if !self.known.contains_key(id) {
                self.known.insert(*id, (url.clone(), title.clone()));
                let _ = self
                    .shared
                    .events_tx
                    .send(BackendEvent::TargetCreated(TargetInfo {
                        target_id: tab_to_target(*id),
                        url: url.clone(),
                        title: title.clone(),
                        attached: *attached,
                    }));
            } else {
                self.known.insert(*id, (url.clone(), title.clone()));
            }
        }
    }

    fn dispatch(&mut self, engine: &mut dyn Engine, tabs: &mut TabStrip, cmd: CdpCommand) {
        match self.handle_call(
            engine,
            tabs,
            cmd.target_id.clone(),
            &cmd.method,
            &cmd.params,
        ) {
            CallResult::Immediate(result) => {
                let _ = cmd.reply.send(result);
            }
            CallResult::Async(rx) => {
                self.pending.push(PendingCall {
                    rx,
                    reply: Some(cmd.reply),
                    deadline: Instant::now() + CALL_TIMEOUT,
                    target_id: cmd.target_id,
                });
            }
        }
    }

    fn fail_pending_for_target(&mut self, target_id: &str, message: &str) {
        let mut i = 0;
        while i < self.pending.len() {
            if self.pending[i].target_id.as_deref() == Some(target_id) {
                let mut p = self.pending.remove(i);
                if let Some(reply) = p.reply.take() {
                    let _ = reply.send(Err(CdpFailure {
                        code: -32000,
                        message: message.to_string(),
                    }));
                }
            } else {
                i += 1;
            }
        }
    }

    pub(crate) fn emit_domain_events(&mut self, tabs: &TabStrip, events: &[EngineEvent]) {
        let mut wv_to_target: HashMap<WebViewId, String> = HashMap::new();
        for tab in tabs.tabs() {
            wv_to_target.insert(tab.webview, tab_to_target(tab.id));
        }
        for ev in events {
            match ev {
                EngineEvent::Load { id, state } => {
                    let Some(target_id) = wv_to_target.get(id).cloned() else {
                        continue;
                    };
                    let st = self.domains.entry(target_id.clone()).or_default();
                    if !st.page_enabled {
                        continue;
                    }
                    match state {
                        LoadState::Started => {
                            st.next_loader += 1;
                            st.loader_id = format!("loader-{}", st.next_loader);
                            let loader = st.loader_id.clone();
                            let fid = target_id.clone();
                            self.send_domain(
                                &target_id,
                                "Page.frameStartedLoading",
                                json!({"frameId": fid, "loaderId": loader}),
                            );
                        }
                        LoadState::Complete => {
                            let ts = 0.0;
                            let loader = st.loader_id.clone();
                            let fid = target_id.clone();
                            let network_enabled = st.network_enabled;
                            self.send_domain(
                                &target_id,
                                "Page.domContentEventFired",
                                json!({"timestamp": ts}),
                            );
                            self.send_domain(
                                &target_id,
                                "Page.loadEventFired",
                                json!({"timestamp": ts}),
                            );
                            self.send_domain(&target_id, "Page.lifecycleEvent", json!({"frameId": fid.clone(), "loaderId": loader.clone(), "name":"DOMContentLoaded", "timestamp": ts}));
                            self.send_domain(&target_id, "Page.lifecycleEvent", json!({"frameId": fid.clone(), "loaderId": loader.clone(), "name":"load", "timestamp": ts}));
                            self.send_domain(&target_id, "Page.lifecycleEvent", json!({"frameId": fid.clone(), "loaderId": loader.clone(), "name":"networkIdle", "timestamp": ts}));
                            if network_enabled {
                                self.send_domain(
                                    &target_id,
                                    "Network.loadingFinished",
                                    json!({
                                        "requestId": format!("{loader}-doc"),
                                        "timestamp": ts,
                                        "encodedDataLength": 0.0
                                    }),
                                );
                            }
                            self.send_domain(
                                &target_id,
                                "Page.frameStoppedLoading",
                                json!({"frameId": target_id}),
                            );
                        }
                        LoadState::Failed(_) => {
                            self.send_domain(
                                &target_id,
                                "Page.frameStoppedLoading",
                                json!({"frameId": target_id}),
                            );
                        }
                    }
                }
                EngineEvent::UrlChanged { id, url } => {
                    let Some(target_id) = wv_to_target.get(id).cloned() else {
                        continue;
                    };
                    let (page_enabled, loader, runtime_enabled, ctx_id) = {
                        let st = self.domains.entry(target_id.clone()).or_default();
                        st.frame_url = url.to_string();
                        let loader = if st.loader_id.is_empty() {
                            "loader-0".to_string()
                        } else {
                            st.loader_id.clone()
                        };
                        if st.runtime_enabled {
                            st.next_context_id += 1;
                            st.context_id = st.next_context_id;
                        }
                        (st.page_enabled, loader, st.runtime_enabled, st.context_id)
                    };
                    let dom_enabled = self
                        .domains
                        .get(&target_id)
                        .map(|s| s.dom_enabled)
                        .unwrap_or(false);
                    if page_enabled {
                        self.send_domain(&target_id, "Page.frameNavigated", json!({
                            "frame": {"id": target_id, "loaderId": loader, "url": url.to_string(), "securityOrigin": url.origin().ascii_serialization(), "mimeType":"text/html"}
                        }));
                    }
                    if runtime_enabled {
                        self.send_domain(&target_id, "Runtime.executionContextsCleared", json!({}));
                        self.send_domain(&target_id, "Runtime.executionContextCreated", json!({
                            "context": {"id": ctx_id, "origin": url.to_string(), "name":"", "auxData": {"isDefault": true, "type":"default", "frameId": target_id}}
                        }));
                    }
                    if dom_enabled {
                        self.send_domain(&target_id, "DOM.documentUpdated", json!({}));
                    }
                }
                EngineEvent::Request {
                    id,
                    request_id,
                    url,
                    method,
                    is_document,
                } => {
                    let Some(target_id) = wv_to_target.get(id).cloned() else {
                        continue;
                    };
                    let (network_enabled, loader, frame_url) = {
                        let st = self.domains.entry(target_id.clone()).or_default();
                        let loader = if st.loader_id.is_empty() {
                            "loader-0".to_string()
                        } else {
                            st.loader_id.clone()
                        };
                        (st.network_enabled, loader, st.frame_url.clone())
                    };
                    if !network_enabled {
                        continue;
                    }
                    self.send_domain(
                        &target_id,
                        "Network.requestWillBeSent",
                        json!({
                            "requestId": request_id.to_string(),
                            "loaderId": loader,
                            "documentURL": frame_url,
                            "request": {
                                "url": url.to_string(),
                                "method": method,
                                "headers": {}
                            },
                            "timestamp": 0.0,
                            "wallTime": 0.0,
                            "initiator": {"type": if *is_document { "other" } else { "script" }},
                            "type": if *is_document { "Document" } else { "Other" },
                            "frameId": target_id
                        }),
                    );
                }
                EngineEvent::TitleChanged { .. } => {}
                EngineEvent::Console { id, level, message } => {
                    let Some(target_id) = wv_to_target.get(id).cloned() else {
                        continue;
                    };
                    let st = self.domains.entry(target_id.clone()).or_default();
                    if !st.runtime_enabled {
                        continue;
                    }
                    let typ = match level {
                        ConsoleLevel::Log => "log",
                        ConsoleLevel::Info => "info",
                        ConsoleLevel::Warn => "warning",
                        ConsoleLevel::Error => "error",
                        ConsoleLevel::Debug => "debug",
                    };
                    let ctx = st.context_id;
                    self.send_domain(
                        &target_id,
                        "Runtime.consoleAPICalled",
                        json!({
                            "type": typ,
                            "args": [{"type":"string","value": message}],
                            "executionContextId": ctx,
                            "timestamp": 0.0
                        }),
                    );
                }
                EngineEvent::Crashed { id, .. } => {
                    let Some(target_id) = wv_to_target.get(id).cloned() else {
                        continue;
                    };
                    // pending calls will be failed by engine reply Err("target crashed") in most cases,
                    // but also fail any still waiting here.
                    self.fail_pending_for_target(&target_id, "target crashed");
                }
                _ => {}
            }
        }
    }

    pub(crate) fn send_domain(&self, target_id: &str, method: &str, params: Value) {
        let _ = self.shared.events_tx.send(BackendEvent::Domain {
            target_id: target_id.to_string(),
            method: method.to_string(),
            params,
        });
    }

    fn handle_call(
        &mut self,
        engine: &mut dyn Engine,
        tabs: &mut TabStrip,
        target_id: Option<String>,
        method: &str,
        params: &Value,
    ) -> CallResult {
        match method {
            "Target.createTarget" => {
                let url_str = params
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("about:blank");
                let url = Url::parse(url_str)
                    .or_else(|_| Url::parse(&format!("https://{url_str}")))
                    .unwrap_or_else(|_| Url::parse("about:blank").unwrap());
                let id = tabs.open(engine, &url, true);
                CallResult::Immediate(Ok(json!({"targetId": tab_to_target(id)})))
            }
            "Target.closeTarget" => {
                let Some(tid) = params.get("targetId").and_then(|v| v.as_str()) else {
                    return CallResult::Immediate(Err(CdpFailure {
                        code: -32602,
                        message: "targetId required".to_string(),
                    }));
                };
                if let Some(id) = target_to_tab(tid) {
                    tabs.close(engine, id);
                }
                CallResult::Immediate(Ok(json!({"success": true})))
            }
            "Browser.close" => {
                self.shared.quit_requested.store(true, Ordering::SeqCst);
                CallResult::Immediate(Ok(json!({})))
            }
            _ => {
                let Some(tid) = target_id else {
                    return CallResult::Immediate(Err(CdpFailure {
                        code: -32601,
                        message: format!("'{method}' wasn't found"),
                    }));
                };
                self.handle_domain(engine, tabs, &tid, method, params)
            }
        }
    }

    fn handle_domain(
        &mut self,
        engine: &mut dyn Engine,
        tabs: &mut TabStrip,
        target_id: &str,
        method: &str,
        params: &Value,
    ) -> CallResult {
        let Some(tab) = target_to_tab(target_id)
            .and_then(|id| tabs.tabs().iter().find(|t| t.id == id).cloned())
        else {
            return CallResult::Immediate(Err(CdpFailure {
                code: -32000,
                message: "target not found".to_string(),
            }));
        };
        let domain = method.split('.').next().unwrap_or("");
        match domain {
            "Page" => {
                crate::cdp_domains::page::handle(self, engine, &tab, target_id, method, params)
            }
            "Runtime" => {
                crate::cdp_domains::runtime::handle(self, engine, &tab, target_id, method, params)
            }
            "DOM" => crate::cdp_domains::dom::handle(self, engine, &tab, target_id, method, params),
            "Network" => {
                if method == "Network.enable" {
                    self.domain_state(target_id).network_enabled = true;
                } else if method == "Network.disable" {
                    self.domain_state(target_id).network_enabled = false;
                }
                crate::cdp_domains::network::handle(method, params)
            }
            "Input" => crate::cdp_domains::input::handle(engine, &tab, method, params),
            "Console" => crate::cdp_domains::console::handle(method),
            "Log" | "Emulation" | "CSS" | "Debugger" | "Performance" | "Overlay" | "Profiler"
            | "HeapProfiler" | "Security" | "ServiceWorker" | "Inspector" | "Audits" => {
                crate::cdp_domains::emulation::handle(method, params)
            }
            _ => CallResult::Immediate(Err(CdpFailure {
                code: -32601,
                message: format!("'{method}' wasn't found"),
            })),
        }
    }

    pub(crate) fn domain_state(&mut self, target_id: &str) -> &mut TargetDomainState {
        self.domains.entry(target_id.to_string()).or_default()
    }
}

impl Drop for CdpBridge {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.stop();
        }
    }
}

pub fn tab_to_target(id: TabId) -> String {
    format!("{:032X}", id.0)
}

pub fn target_to_tab(s: &str) -> Option<TabId> {
    u64::from_str_radix(s, 16).ok().map(TabId)
}

/// The CDP port for this run: the CLI flag wins, then the setting when enabled.
pub fn cdp_port(flag: Option<u16>, settings: &settings::CdpSettings) -> Option<u16> {
    flag.or_else(|| settings.enabled.then_some(settings.port))
}

pub(crate) enum CallResult {
    Immediate(Result<Value, CdpFailure>),
    Async(mpsc::Receiver<Result<Value, CdpFailure>>),
}

pub(crate) fn ok(v: Value) -> CallResult {
    CallResult::Immediate(Ok(v))
}

pub(crate) fn err(code: i64, message: impl Into<String>) -> CallResult {
    CallResult::Immediate(Err(CdpFailure {
        code,
        message: message.into(),
    }))
}

pub(crate) fn method_not_found(method: &str) -> CallResult {
    err(-32601, format!("'{method}' wasn't found"))
}
