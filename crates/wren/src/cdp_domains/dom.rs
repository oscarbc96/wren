use engine::Engine;
use serde_json::{Value, json};
use tabs::Tab;

use crate::cdp_bridge::{CallResult, CdpBridge, method_not_found, ok};

const DOM_AGENT: &str = include_str!("dom_agent.js");

fn inject_agent(engine: &mut dyn Engine, tab: &Tab) {
    // idempotent due to guard in agent
    engine.run_script(tab.webview, DOM_AGENT);
}

fn eval_agent(
    _bridge: &mut CdpBridge,
    engine: &mut dyn Engine,
    tab: &Tab,
    _target_id: &str,
    expr: &str,
) -> CallResult {
    inject_agent(engine, tab);
    let js = format!(
        "(function(){{try{{var __v=(function(){{return {expr};}})();return JSON.stringify({{ok:true,type:typeof __v,value:__v}});}}catch(e){{return JSON.stringify({{ok:false,exception:{{type:'string',value:String(e)}},description:String(e)}});}}}})()"
    );
    let (tx, rx) = std::sync::mpsc::channel();
    engine.evaluate(
        tab.webview,
        &js,
        Box::new(move |res| {
            let out = match res {
                Ok(v) => {
                    let env = if let Some(s) = v.as_str() {
                        serde_json::from_str::<Value>(s).unwrap_or(v.clone())
                    } else {
                        v
                    };
                    if env.get("ok").and_then(|x| x.as_bool()) == Some(false) {
                        Err(cdp::backend::CdpFailure {
                            code: -32000,
                            message: env
                                .get("description")
                                .and_then(|d| d.as_str())
                                .unwrap_or("exception")
                                .to_string(),
                        })
                    } else {
                        Ok(env.get("value").cloned().unwrap_or(Value::Null))
                    }
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

pub(crate) fn handle(
    bridge: &mut CdpBridge,
    engine: &mut dyn Engine,
    tab: &Tab,
    target_id: &str,
    method: &str,
    params: &Value,
) -> CallResult {
    match method {
        "DOM.enable" => {
            bridge.domain_state(target_id).dom_enabled = true;
            inject_agent(engine, tab);
            ok(json!({}))
        }
        "DOM.disable" => ok(json!({})),
        "DOM.getDocument" => {
            let depth = params.get("depth").and_then(|v| v.as_i64()).unwrap_or(1);
            eval_agent(
                bridge,
                engine,
                tab,
                target_id,
                &format!("window.__wrenDom.getDocument({depth})"),
            )
        }
        "DOM.querySelector" => {
            let node_id = params.get("nodeId").and_then(|v| v.as_i64()).unwrap_or(0);
            let selector = params
                .get("selector")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let sel = serde_json::to_string(selector).unwrap_or_else(|_| "\"\"".into());
            eval_agent(
                bridge,
                engine,
                tab,
                target_id,
                &format!("window.__wrenDom.querySelector({node_id},{sel})"),
            )
        }
        "DOM.querySelectorAll" => {
            let node_id = params.get("nodeId").and_then(|v| v.as_i64()).unwrap_or(0);
            let selector = params
                .get("selector")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let sel = serde_json::to_string(selector).unwrap_or_else(|_| "\"\"".into());
            eval_agent(
                bridge,
                engine,
                tab,
                target_id,
                &format!("window.__wrenDom.querySelectorAll({node_id},{sel})"),
            )
        }
        "DOM.getOuterHTML" => {
            let node_id = params.get("nodeId").and_then(|v| v.as_i64()).unwrap_or(0);
            eval_agent(
                bridge,
                engine,
                tab,
                target_id,
                &format!("window.__wrenDom.getOuterHTML({node_id})"),
            )
        }
        "DOM.setOuterHTML" => {
            let node_id = params.get("nodeId").and_then(|v| v.as_i64()).unwrap_or(0);
            let html = params
                .get("outerHTML")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let h = serde_json::to_string(html).unwrap_or_else(|_| "\"\"".into());
            eval_agent(
                bridge,
                engine,
                tab,
                target_id,
                &format!("window.__wrenDom.setOuterHTML({node_id},{h})"),
            )
        }
        "DOM.describeNode" => {
            let node_id = params.get("nodeId").and_then(|v| v.as_i64()).unwrap_or(0);
            let depth = params.get("depth").and_then(|v| v.as_i64()).unwrap_or(1);
            eval_agent(
                bridge,
                engine,
                tab,
                target_id,
                &format!("window.__wrenDom.describeNode({node_id},{depth})"),
            )
        }
        "DOM.resolveNode" => {
            let node_id = params.get("nodeId").and_then(|v| v.as_i64()).unwrap_or(0);
            eval_agent(
                bridge,
                engine,
                tab,
                target_id,
                &format!("window.__wrenDom.resolveNode({node_id})"),
            )
        }
        "DOM.requestNode" => {
            let object_id = params
                .get("objectId")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let oid = serde_json::to_string(object_id).unwrap_or_else(|_| "\"\"".into());
            eval_agent(
                bridge,
                engine,
                tab,
                target_id,
                &format!("window.__wrenDom.requestNode({oid})"),
            )
        }
        "DOM.getBoxModel" => {
            let node_id = params.get("nodeId").and_then(|v| v.as_i64()).unwrap_or(0);
            eval_agent(
                bridge,
                engine,
                tab,
                target_id,
                &format!("window.__wrenDom.getBoxModel({node_id})"),
            )
        }
        "DOM.focus" => {
            let node_id = params.get("nodeId").and_then(|v| v.as_i64()).unwrap_or(0);
            eval_agent(
                bridge,
                engine,
                tab,
                target_id,
                &format!("window.__wrenDom.focus({node_id})"),
            )
        }
        "DOM.setAttributeValue" => {
            let node_id = params.get("nodeId").and_then(|v| v.as_i64()).unwrap_or(0);
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let value = params.get("value").and_then(|v| v.as_str()).unwrap_or("");
            let n = serde_json::to_string(name).unwrap_or_else(|_| "\"\"".into());
            let v = serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into());
            eval_agent(
                bridge,
                engine,
                tab,
                target_id,
                &format!("window.__wrenDom.setAttributeValue({node_id},{n},{v})"),
            )
        }
        "DOM.removeNode" => {
            let node_id = params.get("nodeId").and_then(|v| v.as_i64()).unwrap_or(0);
            eval_agent(
                bridge,
                engine,
                tab,
                target_id,
                &format!("window.__wrenDom.removeNode({node_id})"),
            )
        }
        "DOM.scrollIntoViewIfNeeded" => {
            let node_id = params.get("nodeId").and_then(|v| v.as_i64()).unwrap_or(0);
            eval_agent(
                bridge,
                engine,
                tab,
                target_id,
                &format!("window.__wrenDom.scrollIntoViewIfNeeded({node_id})"),
            )
        }
        _ => method_not_found(method),
    }
}
