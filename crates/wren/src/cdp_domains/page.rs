use base64::Engine as _;
use engine::Engine;
use serde_json::{Value, json};
use tabs::Tab;
use url::Url;

use crate::cdp_bridge::{CallResult, CdpBridge, err, method_not_found, ok};

pub(crate) fn handle(
    bridge: &mut CdpBridge,
    engine: &mut dyn Engine,
    tab: &Tab,
    target_id: &str,
    method: &str,
    params: &Value,
) -> CallResult {
    match method {
        "Page.enable" => {
            let st = bridge.domain_state(target_id);
            st.page_enabled = true;
            if st.frame_url.is_empty() {
                st.frame_url = tab.url.to_string();
            }
            ok(json!({}))
        }
        "Page.disable" => ok(json!({})),
        "Page.navigate" => {
            let Some(url_str) = params.get("url").and_then(|v| v.as_str()) else {
                return err(-32602, "url required");
            };
            let Ok(url) =
                Url::parse(url_str).or_else(|_| Url::parse(&format!("https://{url_str}")))
            else {
                return err(-32602, "invalid url");
            };
            let st = bridge.domain_state(target_id);
            st.next_loader += 1;
            st.loader_id = format!("loader-{}", st.next_loader);
            st.frame_url = url.to_string();
            let frame_id = target_id.to_string();
            let loader_id = st.loader_id.clone();
            engine.load(tab.webview, &url);
            ok(json!({"frameId": frame_id, "loaderId": loader_id}))
        }
        "Page.reload" => {
            engine.reload(tab.webview);
            ok(json!({}))
        }
        "Page.goBack" | "Page.navigateToHistoryEntry" => {
            // minimal: try back if requested? navigateToHistoryEntry needs history; best effort
            if method == "Page.goBack" {
                engine.back(tab.webview);
            }
            ok(json!({}))
        }
        "Page.goForward" => {
            engine.forward(tab.webview);
            ok(json!({}))
        }
        "Page.getNavigationHistory" => {
            let st = bridge.domain_state(target_id);
            let url = if st.frame_url.is_empty() {
                tab.url.to_string()
            } else {
                st.frame_url.clone()
            };
            ok(json!({
                "currentIndex": 0,
                "entries": [{"id": 0, "url": url, "userTypedURL": url, "title": tab.title, "transitionType": "typed"}]
            }))
        }
        "Page.getFrameTree" => {
            let st = bridge.domain_state(target_id);
            let url = if st.frame_url.is_empty() {
                tab.url.to_string()
            } else {
                st.frame_url.clone()
            };
            let loader = if st.loader_id.is_empty() {
                "loader-0".to_string()
            } else {
                st.loader_id.clone()
            };
            ok(json!({
                "frameTree": {
                    "frame": {
                        "id": target_id,
                        "loaderId": loader,
                        "url": url,
                        "securityOrigin": Url::parse(&url).map(|u| u.origin().ascii_serialization()).unwrap_or_default(),
                        "mimeType": "text/html"
                    }
                }
            }))
        }
        "Page.captureScreenshot" => {
            let (tx, rx) = std::sync::mpsc::channel();
            let wv = tab.webview;
            engine.screenshot(
                wv,
                Box::new(move |res| {
                    let out = match res {
                        Ok(bytes) => {
                            let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
                            Ok(json!({"data": b64}))
                        }
                        Err(e) => Err(cdp::backend::CdpFailure {
                            code: -32000,
                            message: e,
                        }),
                    };
                    let _ = tx.send(out);
                }),
            );
            CallResult::Async(rx)
        }
        "Page.setLifecycleEventsEnabled"
        | "Page.addScriptToEvaluateOnNewDocument"
        | "Page.createIsolatedWorld"
        | "Page.bringToFront"
        | "Page.setInterceptFileChooserDialog" => {
            // stubs / accepted
            if method == "Page.createIsolatedWorld" {
                return ok(json!({"executionContextId": 2}));
            }
            if method == "Page.addScriptToEvaluateOnNewDocument" {
                return ok(json!({"identifier": "1"}));
            }
            ok(json!({}))
        }
        _ => method_not_found(method),
    }
}
