use std::time::Duration;

use engine::FakeEngine;
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use settings::Settings;
use tabs::TabStrip;
use tokio_tungstenite::tungstenite::Message;
use url::Url;
use wren::cdp_bridge::CdpBridge;

fn url(s: &str) -> Url {
    Url::parse(s).unwrap()
}

fn http_get(port: u16, path: &str) -> (u16, String) {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let req = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).unwrap();
    let mut resp = String::new();
    stream.read_to_string(&mut resp).unwrap();
    let status = resp
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    let body = resp
        .split_once("\r\n\r\n")
        .map(|(_, b)| b)
        .unwrap_or("")
        .to_string();
    (status, body)
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

fn pump_until<F: FnMut(&TabStrip) -> bool>(
    bridge: &mut CdpBridge,
    engine: &mut FakeEngine,
    tabs: &mut TabStrip,
    mut pred: F,
) {
    for _ in 0..300 {
        bridge.pump(engine, tabs);
        if pred(tabs) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn targets_mirror_tabs() {
    let mut engine = FakeEngine::default();
    let mut tabs = TabStrip::new(25);
    tabs.open(&mut engine, &url("https://a.test/"), true);
    tabs.open(&mut engine, &url("https://b.test/"), false);

    let mut bridge = CdpBridge::start(0).expect("start bridge");
    let port = bridge.port();
    bridge.pump(&mut engine, &mut tabs);

    let (status, body) = http_get(port, "/json/list");
    assert_eq!(status, 200);
    let list: Value = serde_json::from_str(&body).unwrap();
    let arr = list.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    let urls: Vec<_> = arr.iter().map(|e| e["url"].as_str().unwrap()).collect();
    assert!(urls.contains(&"https://a.test/"));
    assert!(urls.contains(&"https://b.test/"));
}

#[tokio::test]
async fn create_target_opens_tab() {
    let mut engine = FakeEngine::default();
    let mut tabs = TabStrip::new(25);
    let mut bridge = CdpBridge::start(0).expect("start bridge");
    let port = bridge.port();
    bridge.pump(&mut engine, &mut tabs);

    let mut ws = ws_connect(port, "/devtools/browser/test").await;
    ws_send(
        &mut ws,
        json!({"id":1,"method":"Target.createTarget","params":{"url":"https://created.test/"}}),
    )
    .await;

    pump_until(&mut bridge, &mut engine, &mut tabs, |tabs| {
        tabs.tabs()
            .iter()
            .any(|t| t.url.as_str() == "https://created.test/")
    });
    assert_eq!(tabs.tabs().len(), 1);
    assert_eq!(tabs.tabs()[0].url.as_str(), "https://created.test/");

    let reply = ws_recv_json(&mut ws).await;
    assert_eq!(reply["id"], 1);
    assert!(reply["result"]["targetId"].as_str().is_some());
}

#[tokio::test]
async fn close_target_closes_tab() {
    let mut engine = FakeEngine::default();
    let mut tabs = TabStrip::new(25);
    let id = tabs.open(&mut engine, &url("https://close.test/"), true);
    let mut bridge = CdpBridge::start(0).expect("start bridge");
    let port = bridge.port();
    bridge.pump(&mut engine, &mut tabs);
    let target_id = wren::cdp_bridge::tab_to_target(id);

    let mut ws = ws_connect(port, "/devtools/browser/test").await;
    ws_send(
        &mut ws,
        json!({"id":1,"method":"Target.closeTarget","params":{"targetId": target_id}}),
    )
    .await;
    pump_until(&mut bridge, &mut engine, &mut tabs, |tabs| {
        tabs.tabs().is_empty()
    });
    assert!(tabs.tabs().is_empty());
    let reply = ws_recv_json(&mut ws).await;
    assert_eq!(reply["id"], 1);
}

#[tokio::test]
async fn ui_close_detaches_session() {
    let mut engine = FakeEngine::default();
    let mut tabs = TabStrip::new(25);
    let id = tabs.open(&mut engine, &url("https://ui.test/"), true);
    let mut bridge = CdpBridge::start(0).expect("start bridge");
    let port = bridge.port();
    bridge.pump(&mut engine, &mut tabs);
    let target_id = wren::cdp_bridge::tab_to_target(id);

    let mut ws = ws_connect(port, "/devtools/browser/test").await;
    ws_send(
        &mut ws,
        json!({"id":1,"method":"Target.setDiscoverTargets","params":{"discover":true}}),
    )
    .await;
    // drain reply + targetCreated
    let mut got_reply = false;
    for _ in 0..5 {
        bridge.pump(&mut engine, &mut tabs);
        if let Ok(msg) =
            tokio::time::timeout(Duration::from_millis(50), ws_recv_json(&mut ws)).await
        {
            if msg["id"] == 1 {
                got_reply = true;
            }
            if got_reply { /* got at least reply */ }
        }
        if got_reply {
            break;
        }
    }
    // ensure we consumed any created events
    for _ in 0..3 {
        bridge.pump(&mut engine, &mut tabs);
        let _ = tokio::time::timeout(Duration::from_millis(30), ws_recv_json(&mut ws)).await;
    }

    ws_send(
        &mut ws,
        json!({"id":2,"method":"Target.attachToTarget","params":{"targetId":target_id,"flatten":true}}),
    )
    .await;
    pump_until(&mut bridge, &mut engine, &mut tabs, |tabs| {
        tabs.tabs().first().map(|t| t.attached).unwrap_or(false)
    });
    // read attach reply
    let mut session_id = Value::Null;
    for _ in 0..20 {
        bridge.pump(&mut engine, &mut tabs);
        if let Ok(msg) =
            tokio::time::timeout(Duration::from_millis(50), ws_recv_json(&mut ws)).await
            && msg["id"] == 2
        {
            session_id = msg["result"]["sessionId"].clone();
            break;
        }
    }
    assert!(!session_id.is_null(), "expected attach reply");

    // close tab through tab strip action
    tabs.close(&mut engine, id);

    // pump to emit destroyed/detached events
    let mut methods = Vec::new();
    for _ in 0..50 {
        bridge.pump(&mut engine, &mut tabs);
        if let Ok(msg) =
            tokio::time::timeout(Duration::from_millis(50), ws_recv_json(&mut ws)).await
            && let Some(m) = msg["method"].as_str()
        {
            if m == "Target.detachedFromTarget" {
                assert_eq!(msg["params"]["sessionId"], session_id);
            }
            methods.push(m.to_string());
        }
        if methods.contains(&"Target.detachedFromTarget".to_string())
            && methods.contains(&"Target.targetDestroyed".to_string())
        {
            break;
        }
    }
    assert!(
        methods.contains(&"Target.detachedFromTarget".to_string()),
        "methods: {methods:?}"
    );
    assert!(
        methods.contains(&"Target.targetDestroyed".to_string()),
        "methods: {methods:?}"
    );

    // connection stays open: following Target.getTargets answers
    ws_send(
        &mut ws,
        json!({"id":3,"method":"Target.getTargets","params":{}}),
    )
    .await;
    for _ in 0..50 {
        bridge.pump(&mut engine, &mut tabs);
        if let Ok(msg) =
            tokio::time::timeout(Duration::from_millis(50), ws_recv_json(&mut ws)).await
            && msg["id"] == 3
        {
            assert!(msg["result"]["targetInfos"].is_array());
            return;
        }
    }
    panic!("did not get Target.getTargets reply");
}

#[test]
fn attached_tab_never_sleeps() {
    let mut engine = FakeEngine::default();
    let mut tabs = TabStrip::new(25);
    let active = tabs.open(&mut engine, &url("https://active.test/"), true);
    let bg = tabs.open(&mut engine, &url("https://bg.test/"), false);
    let attached = tabs.open(&mut engine, &url("https://attached.test/"), false);
    let now = jiff::Timestamp::from_second(3600).unwrap();
    let after = jiff::SignedDuration::from_secs(60);
    for t in tabs.tabs_mut() {
        t.last_active = jiff::Timestamp::from_second(0).unwrap();
    }
    tabs.set_attached(attached, true);
    let candidates = tabs.sleep_candidates(now, after);
    assert!(candidates.contains(&bg), "bg should sleep: {candidates:?}");
    assert!(
        !candidates.contains(&attached),
        "attached must not sleep: {candidates:?}"
    );
    assert!(!candidates.contains(&active));
    // timer elapses / sleep application
    for id in candidates {
        tabs.sleep(&mut engine, id);
    }
    let att = tabs.tabs().iter().find(|t| t.id == attached).unwrap();
    assert_ne!(att.state, tabs::TabState::Slept);
    let _ = active;
}

#[test]
fn cdp_off_by_default() {
    let settings = Settings::default();
    assert!(!settings.cdp.enabled);
    assert_eq!(settings.cdp.port, 9222);
    assert_eq!(wren::cdp_bridge::cdp_port(None, &settings.cdp), None);
    assert_eq!(
        wren::cdp_bridge::cdp_port(Some(9333), &settings.cdp),
        Some(9333)
    );
}
