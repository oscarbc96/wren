use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};

use async_trait::async_trait;
use cdp::backend::{BackendEvent, CdpBackend, CdpBackendFactory, CdpFailure, TargetInfo};
use cdp::server::{CdpHandle, CdpServer};
use engine::Engine;
use serde_json::{Value, json};
use tabs::{TabId, TabStrip};
use tokio::sync::{broadcast, oneshot};
use url::Url;

struct CdpCommand {
    target_id: Option<String>,
    method: String,
    params: Value,
    reply: oneshot::Sender<Result<Value, CdpFailure>>,
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

pub struct CdpBridge {
    handle: Option<CdpHandle>,
    port: u16,
    cmd_rx: mpsc::Receiver<CdpCommand>,
    shared: Arc<SharedState>,
    known: HashMap<TabId, (String, String)>,
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
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn take_quit_requested(&self) -> bool {
        self.shared.quit_requested.swap(false, Ordering::SeqCst)
    }

    pub fn pump(&mut self, engine: &mut dyn Engine, tabs: &mut TabStrip) {
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
            let result = self.handle_call(engine, tabs, &cmd.target_id, &cmd.method, &cmd.params);
            let _ = cmd.reply.send(result);
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

        // Emit events
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

    fn handle_call(
        &self,
        engine: &mut dyn Engine,
        tabs: &mut TabStrip,
        _target_id: &Option<String>,
        method: &str,
        params: &Value,
    ) -> Result<Value, CdpFailure> {
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
                Ok(json!({"targetId": tab_to_target(id)}))
            }
            "Target.closeTarget" => {
                let Some(tid) = params.get("targetId").and_then(|v| v.as_str()) else {
                    return Err(CdpFailure {
                        code: -32602,
                        message: "targetId required".to_string(),
                    });
                };
                if let Some(id) = target_to_tab(tid) {
                    tabs.close(engine, id);
                }
                Ok(json!({"success": true}))
            }
            "Browser.close" => {
                self.shared.quit_requested.store(true, Ordering::SeqCst);
                Ok(json!({}))
            }
            _ => Err(CdpFailure {
                code: -32601,
                message: format!("'{method}' wasn't found"),
            }),
        }
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
