// Copyright 2024 Obscura contributors (Apache-2.0)
// Modified for Wren: re-targeted to the CdpBackend trait.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::{Value, json};

use crate::backend::{BackendEvent, CdpBackend, TargetInfo};
use crate::types::{CdpEvent, CdpRequest, CdpResponse};

pub struct SessionState {
    sessions: HashMap<String, String>, // session_id -> target_id
    pub default_session: Option<String>,
    discover_targets: bool,
    auto_attach: bool,
}

impl SessionState {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            default_session: None,
            discover_targets: false,
            auto_attach: false,
        }
    }

    pub fn attach(&mut self, session_id: &str, target_id: &str) {
        self.sessions
            .insert(session_id.to_string(), target_id.to_string());
    }

    pub fn target_for_session(&self, session_id: &str) -> Option<&str> {
        self.sessions.get(session_id).map(|s| s.as_str())
    }
}

impl Default for SessionState {
    fn default() -> Self {
        Self::new()
    }
}

fn target_info_json(t: &TargetInfo, attached: bool) -> Value {
    json!({
        "targetId": t.target_id,
        "type": "page",
        "title": t.title,
        "url": t.url,
        "attached": attached,
        "canAccessOpener": false,
    })
}

pub async fn handle_target_browser(
    backend: &Arc<dyn CdpBackend>,
    state: &mut SessionState,
    req: &CdpRequest,
) -> Vec<Value> {
    let method = req.method.as_str();
    let params = &req.params;
    let mut out = Vec::new();

    match method {
        "Target.setDiscoverTargets" => {
            let discover = params
                .get("discover")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            state.discover_targets = discover;
            if discover {
                let targets = backend.targets().await;
                for t in targets {
                    out.push(
                        serde_json::to_value(CdpEvent::new(
                            "Target.targetCreated",
                            json!({"targetInfo": target_info_json(&t, false)}),
                        ))
                        .unwrap(),
                    );
                }
            }
            out.push(
                serde_json::to_value(CdpResponse::success(
                    req.id,
                    json!({}),
                    req.session_id.clone(),
                ))
                .unwrap(),
            );
        }
        "Target.getTargets" => {
            let targets = backend.targets().await;
            let infos: Vec<Value> = targets
                .iter()
                .map(|t| {
                    let attached = state.sessions.values().any(|id| id == &t.target_id);
                    target_info_json(t, attached)
                })
                .collect();
            out.push(
                serde_json::to_value(CdpResponse::success(
                    req.id,
                    json!({"targetInfos": infos}),
                    req.session_id.clone(),
                ))
                .unwrap(),
            );
        }
        "Target.attachToTarget" => {
            let Some(target_id) = params.get("targetId").and_then(|v| v.as_str()) else {
                out.push(
                    serde_json::to_value(CdpResponse::error(
                        req.id,
                        -32602,
                        "targetId required".to_string(),
                        req.session_id.clone(),
                    ))
                    .unwrap(),
                );
                return out;
            };
            let session_id = format!("session-{}", uuid::Uuid::new_v4());
            state.attach(&session_id, target_id);
            // if autoAttach requested in future this is where; not specified in this call
            out.push(
                serde_json::to_value(CdpResponse::success(
                    req.id,
                    json!({"sessionId": session_id}),
                    req.session_id.clone(),
                ))
                .unwrap(),
            );
        }
        "Target.detachFromTarget" => {
            if let Some(sid) = params.get("sessionId").and_then(|v| v.as_str()) {
                state.sessions.remove(sid);
            } else if let Some(tid) = params.get("targetId").and_then(|v| v.as_str()) {
                state.sessions.retain(|_, v| v != tid);
            }
            out.push(
                serde_json::to_value(CdpResponse::success(
                    req.id,
                    json!({}),
                    req.session_id.clone(),
                ))
                .unwrap(),
            );
        }
        "Target.setAutoAttach" => {
            let auto = params
                .get("autoAttach")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            state.auto_attach = auto;
            if auto {
                let targets = backend.targets().await;
                for t in targets {
                    let session_id = format!("session-{}", uuid::Uuid::new_v4());
                    state.attach(&session_id, &t.target_id);
                    out.push(
                        serde_json::to_value(CdpEvent::with_session(
                            "Target.attachedToTarget",
                            json!({
                                "sessionId": session_id,
                                "targetInfo": target_info_json(&t, true),
                                "waitingForDebugger": false,
                            }),
                            req.session_id.clone().unwrap_or_default(),
                        ))
                        .unwrap_or_else(|_| {
                            // if no parent session, emit without session; use new() form
                            serde_json::to_value(CdpEvent::new(
                                "Target.attachedToTarget",
                                json!({
                                    "sessionId": session_id,
                                    "targetInfo": target_info_json(&t, true),
                                    "waitingForDebugger": false,
                                }),
                            ))
                            .unwrap()
                        }),
                    );
                }
            }
            out.push(
                serde_json::to_value(CdpResponse::success(
                    req.id,
                    json!({}),
                    req.session_id.clone(),
                ))
                .unwrap(),
            );
        }
        "Target.getTargetInfo" => {
            let targets = backend.targets().await;
            if let Some(tid) = params.get("targetId").and_then(|v| v.as_str()) {
                if let Some(t) = targets.iter().find(|t| t.target_id == tid) {
                    let attached = state.sessions.values().any(|id| id == tid);
                    out.push(
                        serde_json::to_value(CdpResponse::success(
                            req.id,
                            json!({"targetInfo": target_info_json(t, attached)}),
                            req.session_id.clone(),
                        ))
                        .unwrap(),
                    );
                } else {
                    out.push(
                        serde_json::to_value(CdpResponse::error(
                            req.id,
                            -32602,
                            "Target not found".to_string(),
                            req.session_id.clone(),
                        ))
                        .unwrap(),
                    );
                }
            } else {
                out.push(
                    serde_json::to_value(CdpResponse::success(
                        req.id,
                        json!({"targetInfo": {
                            "targetId": "browser",
                            "type": "browser",
                            "title": "",
                            "url": "",
                            "attached": true,
                            "canAccessOpener": false,
                        }}),
                        req.session_id.clone(),
                    ))
                    .unwrap(),
                );
            }
        }
        "Target.createTarget" | "Target.closeTarget" | "Browser.close" => {
            let res = backend.call(None, method, params.clone()).await;
            let resp = match res {
                Ok(v) => CdpResponse::success(req.id, v, req.session_id.clone()),
                Err(e) => CdpResponse::error(req.id, e.code, e.message, req.session_id.clone()),
            };
            out.push(serde_json::to_value(resp).unwrap());
        }
        "Browser.getVersion" => {
            out.push(
                serde_json::to_value(CdpResponse::success(
                    req.id,
                    json!({
                        "protocolVersion": "1.3",
                        "product": format!("Wren/{}", env!("CARGO_PKG_VERSION")),
                        "revision": "0",
                        "userAgent": format!("Wren/{}", env!("CARGO_PKG_VERSION")),
                        "jsVersion": "",
                    }),
                    req.session_id.clone(),
                ))
                .unwrap(),
            );
        }
        _ => {
            // try forwarding to backend for unknown Target/Browser? plan lists specific ones;
            // others should be -32601
            out.push(
                serde_json::to_value(CdpResponse::error(
                    req.id,
                    -32601,
                    format!("'{}' wasn't found", method),
                    req.session_id.clone(),
                ))
                .unwrap(),
            );
        }
    }

    out
}

pub fn events_from_backend(state: &mut SessionState, ev: BackendEvent) -> Vec<Value> {
    match ev {
        BackendEvent::TargetCreated(t) => {
            if state.discover_targets {
                vec![
                    serde_json::to_value(CdpEvent::new(
                        "Target.targetCreated",
                        json!({"targetInfo": target_info_json(&t, false)}),
                    ))
                    .unwrap(),
                ]
            } else {
                vec![]
            }
        }
        BackendEvent::TargetChanged(t) => {
            if state.discover_targets {
                vec![
                    serde_json::to_value(CdpEvent::new(
                        "Target.targetInfoChanged",
                        json!({"targetInfo": target_info_json(&t, false)}),
                    ))
                    .unwrap(),
                ]
            } else {
                vec![]
            }
        }
        BackendEvent::TargetDestroyed(id) => {
            let mut detached: Vec<String> = state
                .sessions
                .iter()
                .filter(|(_, tid)| tid.as_str() == id)
                .map(|(sid, _)| sid.clone())
                .collect();
            detached.sort();
            let mut out: Vec<Value> = detached
                .into_iter()
                .map(|sid| {
                    state.sessions.remove(&sid);
                    serde_json::to_value(CdpEvent::new(
                        "Target.detachedFromTarget",
                        json!({"sessionId": sid, "targetId": id}),
                    ))
                    .unwrap()
                })
                .collect();
            if state.discover_targets {
                out.push(
                    serde_json::to_value(CdpEvent::new(
                        "Target.targetDestroyed",
                        json!({"targetId": id}),
                    ))
                    .unwrap(),
                );
            }
            out
        }
        BackendEvent::Domain {
            target_id,
            method,
            params,
        } => {
            // route to all sessions attached to target
            let sessions: Vec<String> = state
                .sessions
                .iter()
                .filter(|(_, tid)| tid.as_str() == target_id)
                .map(|(sid, _)| sid.clone())
                .collect();
            if sessions.is_empty() {
                vec![serde_json::to_value(CdpEvent::new(&method, params)).unwrap()]
            } else {
                sessions
                    .into_iter()
                    .map(|sid| {
                        serde_json::to_value(CdpEvent::with_session(&method, params.clone(), sid))
                            .unwrap()
                    })
                    .collect()
            }
        }
    }
}
