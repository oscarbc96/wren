use engine::Engine;
use serde_json::{Value, json};
use tabs::Tab;

use crate::cdp_bridge::{CallResult, CdpBridge, method_not_found, ok};

fn envelope_to_remote(envelope: &Value) -> Value {
    // envelope: {ok, type, subtype?, value?, description?, objectId?, exception?}
    let okv = envelope
        .get("ok")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !okv {
        return json!({"type":"undefined"});
    }
    let mut obj = serde_json::Map::new();
    if let Some(t) = envelope.get("type").and_then(|v| v.as_str()) {
        obj.insert("type".into(), json!(t));
    } else {
        obj.insert("type".into(), json!("object"));
    }
    if let Some(s) = envelope.get("subtype").and_then(|v| v.as_str()) {
        obj.insert("subtype".into(), json!(s));
    }
    if let Some(v) = envelope.get("value") {
        obj.insert("value".into(), v.clone());
    }
    if let Some(d) = envelope.get("description").and_then(|v| v.as_str()) {
        obj.insert("description".into(), json!(d));
    }
    if let Some(id) = envelope.get("objectId").and_then(|v| v.as_str()) {
        obj.insert("objectId".into(), json!(id));
    }
    Value::Object(obj)
}

fn wrap_evaluate(expression: &str, _return_by_value: bool, _await_promise: bool) -> String {
    // Simple wrapper producing envelope. For FakeEngine tests JS is ignored and preset envelope returned.
    // Best-effort real wrapper:
    format!(
        "(function(){{try{{var __v=(function(){{ return ({expression}); }})();return JSON.stringify({{ok:true,type:typeof __v,value:__v}});}}catch(e){{return JSON.stringify({{ok:false,exception:{{type:'string',value:String(e)}},description:String(e)}});}}}})()"
    )
}

fn wrap_call(function_declaration: &str, args: &[Value], _return_by_value: bool) -> String {
    let mut argv = String::from("[");
    for (i, a) in args.iter().enumerate() {
        if i > 0 {
            argv.push(',');
        }
        if let Some(v) = a.get("value") {
            argv.push_str(&v.to_string());
        } else if let Some(s) = a.get("unserializableValue").and_then(|v| v.as_str()) {
            argv.push_str(&format!("\"{s}\""));
        } else {
            argv.push_str("undefined");
        }
    }
    argv.push(']');
    format!(
        "(function(){{try{{var __f=({function_declaration});var __v=__f.apply(null,{argv});return JSON.stringify({{ok:true,type:typeof __v,value:__v}});}}catch(e){{return JSON.stringify({{ok:false,exception:{{type:'string',value:String(e)}},description:String(e)}});}}}})()"
    )
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
        "Runtime.enable" => {
            let st = bridge.domain_state(target_id);
            st.runtime_enabled = true;
            if st.context_id == 0 {
                st.next_context_id = 1;
                st.context_id = 1;
            }
            let ctx = st.context_id;
            let fid = target_id.to_string();
            let origin = tab.url.to_string();
            // emit executionContextCreated
            bridge.send_domain(target_id, "Runtime.executionContextCreated", json!({
                "context": {"id": ctx, "origin": origin, "name":"", "auxData": {"isDefault": true, "type":"default", "frameId": fid}}
            }));
            ok(json!({}))
        }
        "Runtime.disable" => ok(json!({})),
        "Runtime.evaluate" => {
            let expr = params
                .get("expression")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let ret = params
                .get("returnByValue")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let await_p = params
                .get("awaitPromise")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let js = wrap_evaluate(expr, ret, await_p);
            let (tx, rx) = std::sync::mpsc::channel();
            engine.evaluate(tab.webview, &js, Box::new(move |res| {
                let out = match res {
                    Ok(v) => {
                        // v may be envelope object or stringified JSON envelope
                        let env = if let Some(s) = v.as_str() { serde_json::from_str::<Value>(s).unwrap_or(v.clone()) } else { v };
                        let okv = env.get("ok").and_then(|x| x.as_bool()).unwrap_or(true);
                        if !okv || env.get("exception").is_some() || env.get("ok").and_then(|x| x.as_bool()) == Some(false) {
                            let ex = env.get("exception").cloned().unwrap_or_else(|| json!({"type":"string","value": env.get("description").and_then(|d| d.as_str()).unwrap_or("exception")}));
                            Ok(json!({
                                "result": envelope_to_remote(&env),
                                "exceptionDetails": {
                                    "exceptionId": 1,
                                    "text": env.get("description").and_then(|d| d.as_str()).unwrap_or("exception"),
                                    "lineNumber": 0,
                                    "columnNumber": 0,
                                    "exception": ex
                                }
                            }))
                        } else {
                            Ok(json!({"result": envelope_to_remote(&env)}))
                        }
                    }
                    Err(e) => Err(cdp::backend::CdpFailure { code: -32000, message: e }),
                };
                let _ = tx.send(out);
            }));
            CallResult::Async(rx)
        }
        "Runtime.callFunctionOn" => {
            let func = params
                .get("functionDeclaration")
                .and_then(|v| v.as_str())
                .unwrap_or("function(){}");
            let args = params
                .get("arguments")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let ret = params
                .get("returnByValue")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let js = wrap_call(func, &args, ret);
            let (tx, rx) = std::sync::mpsc::channel();
            engine.evaluate(tab.webview, &js, Box::new(move |res| {
                let out = match res {
                    Ok(v) => {
                        let env = if let Some(s) = v.as_str() { serde_json::from_str::<Value>(s).unwrap_or(v.clone()) } else { v };
                        let okv = env.get("ok").and_then(|x| x.as_bool()).unwrap_or(true);
                        if !okv || env.get("exception").is_some() || env.get("ok").and_then(|x| x.as_bool()) == Some(false) {
                            let ex = env.get("exception").cloned().unwrap_or_else(|| json!({"type":"string","value": env.get("description").and_then(|d| d.as_str()).unwrap_or("exception")}));
                            Ok(json!({
                                "result": envelope_to_remote(&env),
                                "exceptionDetails": {
                                    "exceptionId": 1,
                                    "text": env.get("description").and_then(|d| d.as_str()).unwrap_or("exception"),
                                    "lineNumber": 0,
                                    "columnNumber": 0,
                                    "exception": ex
                                }
                            }))
                        } else {
                            Ok(json!({"result": envelope_to_remote(&env)}))
                        }
                    }
                    Err(e) => Err(cdp::backend::CdpFailure { code: -32000, message: e }),
                };
                let _ = tx.send(out);
            }));
            CallResult::Async(rx)
        }
        "Runtime.getProperties" => ok(json!({"result": []})),
        "Runtime.releaseObject"
        | "Runtime.releaseObjectGroup"
        | "Runtime.runIfWaitingForDebugger" => ok(json!({})),
        _ => method_not_found(method),
    }
}
