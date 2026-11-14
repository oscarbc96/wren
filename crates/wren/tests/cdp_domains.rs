use std::time::Duration;

use engine::{ConsoleLevel, EngineEvent, FakeCall, FakeEngine, LoadState};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tabs::TabStrip;
use tokio_tungstenite::tungstenite::Message;
use url::Url;
use wren::cdp_bridge::CdpBridge;

fn url(s: &str) -> Url {
    Url::parse(s).unwrap()
}

async fn ws_connect(
    port: u16,
    path: &str,
) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    let url = format!("ws://127.0.0.1:{port}{path}");
    let (ws, _) = tokio_tungstenite::connect_async(url).await.unwrap();
    ws
}

async fn ws_send(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    msg: Value,
) {
    ws.send(Message::Text(msg.to_string().into()))
        .await
        .unwrap();
}

async fn ws_recv_json(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> Value {
    loop {
        let msg = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .expect("timeout")
            .expect("closed")
            .expect("ws error");
        if let Message::Text(text) = msg {
            return serde_json::from_str(&text).unwrap();
        }
    }
}

async fn _recv_until_id(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    id: i64,
) -> Value {
    loop {
        let msg = ws_recv_json(ws).await;
        if msg.get("id").and_then(|v| v.as_i64()) == Some(id) {
            return msg;
        }
    }
}

async fn _recv_until_method(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    method: &str,
) -> Value {
    loop {
        let msg = ws_recv_json(ws).await;
        if msg.get("method").and_then(|v| v.as_str()) == Some(method) {
            return msg;
        }
    }
}

async fn attach(
    bridge: &mut CdpBridge,
    engine: &mut FakeEngine,
    tabs: &mut TabStrip,
    port: u16,
    target_id: &str,
) -> (
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    String,
) {
    let mut ws = ws_connect(port, "/devtools/browser/test").await;
    ws_send(
        &mut ws,
        json!({"id":1,"method":"Target.attachToTarget","params":{"targetId":target_id,"flatten":true}}),
    )
    .await;
    let mut session_id = String::new();
    for _ in 0..200 {
        bridge.pump(engine, tabs, &[]);
        if let Ok(msg) =
            tokio::time::timeout(Duration::from_millis(20), ws_recv_json(&mut ws)).await
            && msg["id"] == 1
        {
            session_id = msg["result"]["sessionId"].as_str().unwrap().to_string();
            break;
        }
    }
    assert!(!session_id.is_empty(), "no session");
    (ws, session_id)
}

#[tokio::test]
async fn navigate_emits_load_events_in_order() {
    let mut engine = FakeEngine::default();
    let mut tabs = TabStrip::new(25);
    let tab_id = tabs.open(&mut engine, &url("https://start.test/"), true);
    let mut bridge = CdpBridge::start(0).unwrap();
    let port = bridge.port();
    bridge.pump(&mut engine, &mut tabs, &[]);
    let target = wren::cdp_bridge::tab_to_target(tab_id);
    let (mut ws, sid) = attach(&mut bridge, &mut engine, &mut tabs, port, &target).await;

    ws_send(
        &mut ws,
        json!({"id":2,"sessionId":sid,"method":"Page.enable","params":{}}),
    )
    .await;
    for _ in 0..50 {
        bridge.pump(&mut engine, &mut tabs, &[]);
        let _ = tokio::time::timeout(Duration::from_millis(10), ws_recv_json(&mut ws)).await;
    }

    ws_send(&mut ws, json!({"id":3,"sessionId":sid,"method":"Page.navigate","params":{"url":"https://nav.test/"}})).await;
    for _ in 0..20 {
        bridge.pump(&mut engine, &mut tabs, &[]);
        let _ = tokio::time::timeout(Duration::from_millis(10), ws_recv_json(&mut ws)).await;
    }

    // simulate load lifecycle
    let wv = tabs.tabs()[0].webview;
    let evs = vec![
        EngineEvent::Load {
            id: wv,
            state: LoadState::Started,
        },
        EngineEvent::UrlChanged {
            id: wv,
            url: url("https://nav.test/"),
        },
        EngineEvent::Load {
            id: wv,
            state: LoadState::Complete,
        },
    ];
    for ev in &evs {
        tabs.apply(&mut engine, ev);
    }

    let mut methods = Vec::new();
    for _ in 0..200 {
        bridge.pump(&mut engine, &mut tabs, &evs);
        if let Ok(msg) =
            tokio::time::timeout(Duration::from_millis(20), ws_recv_json(&mut ws)).await
            && let Some(m) = msg.get("method").and_then(|v| v.as_str())
        {
            methods.push(m.to_string());
            if m == "Page.loadEventFired" {
                break;
            }
        }
    }
    let s = methods.join(",");
    assert!(s.contains("Page.frameStartedLoading"), "{s}");
    assert!(s.contains("Page.frameNavigated"), "{s}");
    assert!(s.contains("Page.domContentEventFired"), "{s}");
    assert!(s.contains("Page.loadEventFired"), "{s}");
}

#[tokio::test]
async fn evaluate_by_value() {
    let mut engine = FakeEngine::default();
    let mut tabs = TabStrip::new(25);
    let tab_id = tabs.open(&mut engine, &url("https://e.test/"), true);
    let wv = tabs.tabs()[0].webview;
    let mut bridge = CdpBridge::start(0).unwrap();
    let port = bridge.port();
    bridge.pump(&mut engine, &mut tabs, &[]);
    let target = wren::cdp_bridge::tab_to_target(tab_id);
    let (mut ws, sid) = attach(&mut bridge, &mut engine, &mut tabs, port, &target).await;

    ws_send(
        &mut ws,
        json!({"id":2,"sessionId":sid,"method":"Runtime.enable","params":{}}),
    )
    .await;
    for _ in 0..50 {
        bridge.pump(&mut engine, &mut tabs, &[]);
        let _ = tokio::time::timeout(Duration::from_millis(10), ws_recv_json(&mut ws)).await;
    }

    engine.set_eval_result(wv, Ok(json!({"ok":true,"type":"string","value":"hello"})));
    ws_send(&mut ws, json!({"id":3,"sessionId":sid,"method":"Runtime.evaluate","params":{"expression":"x","returnByValue":true}})).await;
    let reply = loop {
        bridge.pump(&mut engine, &mut tabs, &[]);
        if let Ok(msg) =
            tokio::time::timeout(Duration::from_millis(20), ws_recv_json(&mut ws)).await
            && msg["id"] == 3
        {
            break msg;
        }
    };
    assert_eq!(reply["result"]["result"]["value"], "hello", "{reply}");
}

#[tokio::test]
async fn evaluate_exception_details() {
    let mut engine = FakeEngine::default();
    let mut tabs = TabStrip::new(25);
    let tab_id = tabs.open(&mut engine, &url("https://ex.test/"), true);
    let wv = tabs.tabs()[0].webview;
    let mut bridge = CdpBridge::start(0).unwrap();
    let port = bridge.port();
    bridge.pump(&mut engine, &mut tabs, &[]);
    let target = wren::cdp_bridge::tab_to_target(tab_id);
    let (mut ws, sid) = attach(&mut bridge, &mut engine, &mut tabs, port, &target).await;

    ws_send(
        &mut ws,
        json!({"id":2,"sessionId":sid,"method":"Runtime.enable","params":{}}),
    )
    .await;
    for _ in 0..50 {
        bridge.pump(&mut engine, &mut tabs, &[]);
        let _ = tokio::time::timeout(Duration::from_millis(10), ws_recv_json(&mut ws)).await;
    }

    engine.set_eval_result(
        wv,
        Ok(json!({"ok":false,"exception":{"type":"string","value":"boom"},"description":"boom"})),
    );
    ws_send(
        &mut ws,
        json!({"id":3,"sessionId":sid,"method":"Runtime.evaluate","params":{"expression":"bad()"}}),
    )
    .await;
    let reply = loop {
        bridge.pump(&mut engine, &mut tabs, &[]);
        if let Ok(msg) =
            tokio::time::timeout(Duration::from_millis(20), ws_recv_json(&mut ws)).await
            && msg["id"] == 3
        {
            break msg;
        }
    };
    assert!(reply["result"]["exceptionDetails"].is_object(), "{reply}");
}

#[tokio::test]
async fn call_function_on_with_args() {
    let mut engine = FakeEngine::default();
    let mut tabs = TabStrip::new(25);
    let tab_id = tabs.open(&mut engine, &url("https://cf.test/"), true);
    let wv = tabs.tabs()[0].webview;
    let mut bridge = CdpBridge::start(0).unwrap();
    let port = bridge.port();
    bridge.pump(&mut engine, &mut tabs, &[]);
    let target = wren::cdp_bridge::tab_to_target(tab_id);
    let (mut ws, sid) = attach(&mut bridge, &mut engine, &mut tabs, port, &target).await;

    ws_send(
        &mut ws,
        json!({"id":2,"sessionId":sid,"method":"Runtime.enable","params":{}}),
    )
    .await;
    for _ in 0..50 {
        bridge.pump(&mut engine, &mut tabs, &[]);
        let _ = tokio::time::timeout(Duration::from_millis(10), ws_recv_json(&mut ws)).await;
    }

    engine.set_eval_result(wv, Ok(json!({"ok":true,"type":"number","value":3})));
    ws_send(&mut ws, json!({"id":3,"sessionId":sid,"method":"Runtime.callFunctionOn","params":{"functionDeclaration":"function(a,b){return a+b}","arguments":[{"value":1},{"value":2}],"returnByValue":true}})).await;
    let reply = loop {
        bridge.pump(&mut engine, &mut tabs, &[]);
        if let Ok(msg) =
            tokio::time::timeout(Duration::from_millis(20), ws_recv_json(&mut ws)).await
            && msg["id"] == 3
        {
            break msg;
        }
    };
    assert_eq!(reply["result"]["result"]["value"], 3, "{reply}");
}

#[tokio::test]
async fn capture_screenshot_base64_png() {
    let mut engine = FakeEngine::default();
    let mut tabs = TabStrip::new(25);
    let tab_id = tabs.open(&mut engine, &url("https://ss.test/"), true);
    let mut bridge = CdpBridge::start(0).unwrap();
    let port = bridge.port();
    bridge.pump(&mut engine, &mut tabs, &[]);
    let target = wren::cdp_bridge::tab_to_target(tab_id);
    let (mut ws, sid) = attach(&mut bridge, &mut engine, &mut tabs, port, &target).await;

    ws_send(
        &mut ws,
        json!({"id":2,"sessionId":sid,"method":"Page.enable","params":{}}),
    )
    .await;
    for _ in 0..50 {
        bridge.pump(&mut engine, &mut tabs, &[]);
        let _ = tokio::time::timeout(Duration::from_millis(10), ws_recv_json(&mut ws)).await;
    }

    ws_send(
        &mut ws,
        json!({"id":3,"sessionId":sid,"method":"Page.captureScreenshot","params":{"format":"png"}}),
    )
    .await;
    let reply = loop {
        bridge.pump(&mut engine, &mut tabs, &[]);
        if let Ok(msg) =
            tokio::time::timeout(Duration::from_millis(20), ws_recv_json(&mut ws)).await
            && msg["id"] == 3
        {
            break msg;
        }
    };
    let data = reply["result"]["data"].as_str().unwrap();
    assert!(!data.is_empty());
}

#[tokio::test]
async fn console_api_called_forwarded() {
    let mut engine = FakeEngine::default();
    let mut tabs = TabStrip::new(25);
    let tab_id = tabs.open(&mut engine, &url("https://con.test/"), true);
    let wv = tabs.tabs()[0].webview;
    let mut bridge = CdpBridge::start(0).unwrap();
    let port = bridge.port();
    bridge.pump(&mut engine, &mut tabs, &[]);
    let target = wren::cdp_bridge::tab_to_target(tab_id);
    let (mut ws, sid) = attach(&mut bridge, &mut engine, &mut tabs, port, &target).await;

    ws_send(
        &mut ws,
        json!({"id":2,"sessionId":sid,"method":"Runtime.enable","params":{}}),
    )
    .await;
    for _ in 0..50 {
        bridge.pump(&mut engine, &mut tabs, &[]);
        let _ = tokio::time::timeout(Duration::from_millis(10), ws_recv_json(&mut ws)).await;
    }

    let ev = EngineEvent::Console {
        id: wv,
        level: ConsoleLevel::Log,
        message: "hi".to_string(),
    };
    tabs.apply(&mut engine, &ev);
    let evt = loop {
        bridge.pump(&mut engine, &mut tabs, std::slice::from_ref(&ev));
        if let Ok(msg) =
            tokio::time::timeout(Duration::from_millis(20), ws_recv_json(&mut ws)).await
            && msg.get("method").and_then(|v| v.as_str()) == Some("Runtime.consoleAPICalled")
        {
            break msg;
        }
    };
    assert_eq!(evt["params"]["type"], "log");
}

#[tokio::test]
async fn mouse_pressed_maps_to_engine_input() {
    let mut engine = FakeEngine::default();
    let mut tabs = TabStrip::new(25);
    let tab_id = tabs.open(&mut engine, &url("https://inp.test/"), true);
    let mut bridge = CdpBridge::start(0).unwrap();
    let port = bridge.port();
    bridge.pump(&mut engine, &mut tabs, &[]);
    let target = wren::cdp_bridge::tab_to_target(tab_id);
    let (mut ws, sid) = attach(&mut bridge, &mut engine, &mut tabs, port, &target).await;

    ws_send(&mut ws, json!({"id":2,"sessionId":sid,"method":"Input.dispatchMouseEvent","params":{"type":"mousePressed","x":10,"y":20,"button":"left","clickCount":1}})).await;
    for _ in 0..100 {
        bridge.pump(&mut engine, &mut tabs, &[]);
        let _ = tokio::time::timeout(Duration::from_millis(10), ws_recv_json(&mut ws)).await;
    }
    let calls = engine.calls();
    assert!(
        calls.iter().any(|c| matches!(c, FakeCall::Input(_, _))),
        "{calls:?}"
    );
}

#[tokio::test]
async fn evaluate_on_crashed_tab_errors() {
    let mut engine = FakeEngine::default();
    let mut tabs = TabStrip::new(25);
    let tab_id = tabs.open(&mut engine, &url("https://cr.test/"), true);
    let wv = tabs.tabs()[0].webview;
    let mut bridge = CdpBridge::start(0).unwrap();
    let port = bridge.port();
    bridge.pump(&mut engine, &mut tabs, &[]);
    let target = wren::cdp_bridge::tab_to_target(tab_id);
    let (mut ws, sid) = attach(&mut bridge, &mut engine, &mut tabs, port, &target).await;

    ws_send(
        &mut ws,
        json!({"id":2,"sessionId":sid,"method":"Runtime.enable","params":{}}),
    )
    .await;
    for _ in 0..50 {
        bridge.pump(&mut engine, &mut tabs, &[]);
        let _ = tokio::time::timeout(Duration::from_millis(10), ws_recv_json(&mut ws)).await;
    }

    // evaluate with no preset result -> pending in FakeEngine
    ws_send(&mut ws, json!({"id":3,"sessionId":sid,"method":"Runtime.evaluate","params":{"expression":"hang()"}})).await;
    for _ in 0..20 {
        bridge.pump(&mut engine, &mut tabs, &[]);
        let _ = tokio::time::timeout(Duration::from_millis(10), ws_recv_json(&mut ws)).await;
    }
    let crash = EngineEvent::Crashed {
        id: wv,
        reason: "boom".to_string(),
    };
    tabs.apply(&mut engine, &crash);
    engine.push(crash.clone());

    let reply = loop {
        bridge.pump(&mut engine, &mut tabs, std::slice::from_ref(&crash));
        if let Ok(msg) =
            tokio::time::timeout(Duration::from_millis(50), ws_recv_json(&mut ws)).await
            && msg["id"] == 3
        {
            break msg;
        }
    };
    assert!(reply.get("error").is_some(), "{reply}");

    // server still answers next request
    ws_send(
        &mut ws,
        json!({"id":4,"sessionId":sid,"method":"Page.getFrameTree","params":{}}),
    )
    .await;
    for _ in 0..200 {
        bridge.pump(&mut engine, &mut tabs, &[]);
        if let Ok(msg) =
            tokio::time::timeout(Duration::from_millis(20), ws_recv_json(&mut ws)).await
            && msg["id"] == 4
        {
            return;
        }
    }
    panic!("no reply to next request");
}
