use std::time::Duration;

use engine::{EngineEvent, FakeEngine};
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
async fn get_document_has_html_root() {
    let mut engine = FakeEngine::default();
    let mut tabs = TabStrip::new(25);
    let tab_id = tabs.open(&mut engine, &url("https://dom.test/"), true);
    let wv = tabs.tabs()[0].webview;
    let mut bridge = CdpBridge::start(0).unwrap();
    let port = bridge.port();
    bridge.pump(&mut engine, &mut tabs, &[]);
    let target = wren::cdp_bridge::tab_to_target(tab_id);
    let (mut ws, sid) = attach(&mut bridge, &mut engine, &mut tabs, port, &target).await;

    ws_send(
        &mut ws,
        json!({"id":2,"sessionId":sid,"method":"DOM.enable","params":{}}),
    )
    .await;
    for _ in 0..50 {
        bridge.pump(&mut engine, &mut tabs, &[]);
        let _ = tokio::time::timeout(Duration::from_millis(10), ws_recv_json(&mut ws)).await;
    }

    engine.set_eval_result(
        wv,
        Ok(json!({"ok":true,"type":"object","value":{
            "root":{"nodeId":1,"backendNodeId":1,"nodeType":9,"nodeName":"#document","childNodeCount":1,
                "children":[{"nodeId":2,"backendNodeId":2,"nodeType":1,"nodeName":"HTML","childNodeCount":0,"children":[],"attributes":[]}]}
        }})),
    );
    ws_send(
        &mut ws,
        json!({"id":3,"sessionId":sid,"method":"DOM.getDocument","params":{"depth":-1}}),
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
    assert_eq!(reply["result"]["root"]["nodeName"], "#document", "{reply}");
    assert_eq!(
        reply["result"]["root"]["children"][0]["nodeName"], "HTML",
        "{reply}"
    );
}

#[tokio::test]
async fn query_selector_returns_node_id() {
    let mut engine = FakeEngine::default();
    let mut tabs = TabStrip::new(25);
    let tab_id = tabs.open(&mut engine, &url("https://domq.test/"), true);
    let wv = tabs.tabs()[0].webview;
    let mut bridge = CdpBridge::start(0).unwrap();
    let port = bridge.port();
    bridge.pump(&mut engine, &mut tabs, &[]);
    let target = wren::cdp_bridge::tab_to_target(tab_id);
    let (mut ws, sid) = attach(&mut bridge, &mut engine, &mut tabs, port, &target).await;

    ws_send(
        &mut ws,
        json!({"id":2,"sessionId":sid,"method":"DOM.enable","params":{}}),
    )
    .await;
    for _ in 0..50 {
        bridge.pump(&mut engine, &mut tabs, &[]);
        let _ = tokio::time::timeout(Duration::from_millis(10), ws_recv_json(&mut ws)).await;
    }

    engine.set_eval_result(
        wv,
        Ok(json!({"ok":true,"type":"object","value":{"nodeId":5}})),
    );
    ws_send(&mut ws, json!({"id":3,"sessionId":sid,"method":"DOM.querySelector","params":{"nodeId":1,"selector":"#x"}})).await;
    let reply = loop {
        bridge.pump(&mut engine, &mut tabs, &[]);
        if let Ok(msg) =
            tokio::time::timeout(Duration::from_millis(20), ws_recv_json(&mut ws)).await
            && msg["id"] == 3
        {
            break msg;
        }
    };
    assert_eq!(reply["result"]["nodeId"], 5, "{reply}");
}

#[tokio::test]
async fn get_box_model_quads() {
    let mut engine = FakeEngine::default();
    let mut tabs = TabStrip::new(25);
    let tab_id = tabs.open(&mut engine, &url("https://dombox.test/"), true);
    let wv = tabs.tabs()[0].webview;
    let mut bridge = CdpBridge::start(0).unwrap();
    let port = bridge.port();
    bridge.pump(&mut engine, &mut tabs, &[]);
    let target = wren::cdp_bridge::tab_to_target(tab_id);
    let (mut ws, sid) = attach(&mut bridge, &mut engine, &mut tabs, port, &target).await;

    ws_send(
        &mut ws,
        json!({"id":2,"sessionId":sid,"method":"DOM.enable","params":{}}),
    )
    .await;
    for _ in 0..50 {
        bridge.pump(&mut engine, &mut tabs, &[]);
        let _ = tokio::time::timeout(Duration::from_millis(10), ws_recv_json(&mut ws)).await;
    }

    engine.set_eval_result(wv, Ok(json!({"ok":true,"type":"object","value":{"model":{"content":[0,0,10,0,10,20,0,20],"padding":[0,0,10,0,10,20,0,20],"border":[0,0,10,0,10,20,0,20],"margin":[0,0,10,0,10,20,0,20],"width":10,"height":20}}})));
    ws_send(
        &mut ws,
        json!({"id":3,"sessionId":sid,"method":"DOM.getBoxModel","params":{"nodeId":5}}),
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
    assert_eq!(reply["result"]["model"]["width"], 10, "{reply}");
    assert_eq!(reply["result"]["model"]["height"], 20, "{reply}");
}

#[tokio::test]
async fn request_will_be_sent_from_engine_event() {
    let mut engine = FakeEngine::default();
    let mut tabs = TabStrip::new(25);
    let tab_id = tabs.open(&mut engine, &url("https://net.test/"), true);
    let wv = tabs.tabs()[0].webview;
    let mut bridge = CdpBridge::start(0).unwrap();
    let port = bridge.port();
    bridge.pump(&mut engine, &mut tabs, &[]);
    let target = wren::cdp_bridge::tab_to_target(tab_id);
    let (mut ws, sid) = attach(&mut bridge, &mut engine, &mut tabs, port, &target).await;

    ws_send(
        &mut ws,
        json!({"id":2,"sessionId":sid,"method":"Network.enable","params":{}}),
    )
    .await;
    for _ in 0..50 {
        bridge.pump(&mut engine, &mut tabs, &[]);
        let _ = tokio::time::timeout(Duration::from_millis(10), ws_recv_json(&mut ws)).await;
    }

    let ev = EngineEvent::Request {
        id: wv,
        request_id: 7,
        url: url("https://net.test/res"),
        method: "GET".to_string(),
        is_document: false,
    };
    tabs.apply(&mut engine, &ev);
    let evt = loop {
        bridge.pump(&mut engine, &mut tabs, std::slice::from_ref(&ev));
        if let Ok(msg) =
            tokio::time::timeout(Duration::from_millis(20), ws_recv_json(&mut ws)).await
            && msg.get("method").and_then(|v| v.as_str()) == Some("Network.requestWillBeSent")
        {
            break msg;
        }
    };
    assert_eq!(
        evt["params"]["request"]["url"], "https://net.test/res",
        "{evt}"
    );
}

#[tokio::test]
async fn stub_enable_returns_empty() {
    let mut engine = FakeEngine::default();
    let mut tabs = TabStrip::new(25);
    let tab_id = tabs.open(&mut engine, &url("https://stub.test/"), true);
    let mut bridge = CdpBridge::start(0).unwrap();
    let port = bridge.port();
    bridge.pump(&mut engine, &mut tabs, &[]);
    let target = wren::cdp_bridge::tab_to_target(tab_id);
    let (mut ws, sid) = attach(&mut bridge, &mut engine, &mut tabs, port, &target).await;

    for (i, method) in [
        "CSS.enable",
        "Debugger.enable",
        "Performance.enable",
        "Emulation.setDeviceMetricsOverride",
    ]
    .iter()
    .enumerate()
    {
        let id = (i as i64) + 10;
        ws_send(
            &mut ws,
            json!({"id":id,"sessionId":sid,"method":method,"params":{}}),
        )
        .await;
        let reply = loop {
            bridge.pump(&mut engine, &mut tabs, &[]);
            if let Ok(msg) =
                tokio::time::timeout(Duration::from_millis(50), ws_recv_json(&mut ws)).await
                && msg["id"] == id
            {
                break msg;
            }
        };
        assert_eq!(reply["result"], json!({}), "{method} -> {reply}");
    }
}
