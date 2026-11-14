use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cdp::backend::{BackendEvent, CdpBackend, CdpBackendFactory, CdpFailure, TargetInfo};
use cdp::server::CdpServer;
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;

#[derive(Clone)]
struct FakeBackend {
    targets: Vec<TargetInfo>,
    #[allow(clippy::type_complexity)]
    calls: Arc<Mutex<Vec<(Option<String>, String, Value)>>>,
    events_tx: broadcast::Sender<BackendEvent>,
    responses: Arc<Mutex<VecDeque<Result<Value, CdpFailure>>>>,
}

impl FakeBackend {
    fn new(targets: Vec<TargetInfo>) -> Self {
        let (events_tx, _) = broadcast::channel(64);
        Self {
            targets,
            calls: Arc::new(Mutex::new(Vec::new())),
            events_tx,
            responses: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    fn push_response(&self, resp: Result<Value, CdpFailure>) {
        self.responses.lock().unwrap().push_back(resp);
    }

    fn calls(&self) -> Vec<(Option<String>, String, Value)> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait::async_trait(?Send)]
impl CdpBackend for FakeBackend {
    async fn targets(&self) -> Vec<TargetInfo> {
        self.targets.clone()
    }

    async fn call(
        &self,
        target_id: Option<&str>,
        method: &str,
        params: Value,
    ) -> Result<Value, CdpFailure> {
        self.calls.lock().unwrap().push((
            target_id.map(str::to_string),
            method.to_string(),
            params,
        ));
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Ok(json!({})))
    }

    fn events(&self) -> broadcast::Receiver<BackendEvent> {
        self.events_tx.subscribe()
    }
}

struct FakeFactory {
    backend: FakeBackend,
}

impl CdpBackendFactory for FakeFactory {
    fn create(&self) -> Arc<dyn CdpBackend> {
        Arc::new(self.backend.clone())
    }
}

fn two_targets() -> Vec<TargetInfo> {
    vec![
        TargetInfo {
            target_id: "A".to_string(),
            url: "https://a.test/".to_string(),
            title: "A".to_string(),
            attached: false,
        },
        TargetInfo {
            target_id: "B".to_string(),
            url: "https://b.test/".to_string(),
            title: "B".to_string(),
            attached: false,
        },
    ]
}

fn start_server(backend: FakeBackend) -> (cdp::server::CdpHandle, u16) {
    let factory = Arc::new(FakeFactory { backend });
    let handle = CdpServer::start(0, factory).expect("start cdp server");
    let port = handle.port();
    (handle, port)
}

fn http_get(port: u16, path: &str, extra_headers: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n{extra_headers}\r\n"
    );
    stream.write_all(req.as_bytes()).unwrap();
    let mut resp = String::new();
    stream.read_to_string(&mut resp).unwrap();
    let status = resp
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    (status, resp)
}

fn http_body(resp: &str) -> &str {
    resp.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or("")
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

#[tokio::test]
async fn json_version_lists_browser_ws() {
    let (_handle, port) = start_server(FakeBackend::new(two_targets()));
    let (status, resp) = http_get(port, "/json/version", "");
    assert_eq!(status, 200);
    let body: Value = serde_json::from_str(http_body(&resp)).unwrap();
    assert!(body["Browser"].as_str().unwrap().starts_with("Wren/"));
    assert!(
        body["webSocketDebuggerUrl"]
            .as_str()
            .unwrap()
            .contains("/devtools/browser/")
    );
}

#[tokio::test]
async fn json_list_has_two_pages() {
    let (_handle, port) = start_server(FakeBackend::new(two_targets()));
    let (status, resp) = http_get(port, "/json/list", "");
    assert_eq!(status, 200);
    let body: Value = serde_json::from_str(http_body(&resp)).unwrap();
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert!(arr.iter().all(|e| e["type"] == "page"));
}

#[tokio::test]
async fn attach_flatten_routes_by_session() {
    let backend = FakeBackend::new(two_targets());
    let backend_clone = backend.clone();
    let (_handle, port) = start_server(backend);
    let mut ws = ws_connect(port, "/devtools/browser/test").await;

    ws_send(
        &mut ws,
        json!({"id":1,"method":"Target.attachToTarget","params":{"targetId":"A","flatten":true}}),
    )
    .await;
    let attach_reply = ws_recv_json(&mut ws).await;
    assert_eq!(attach_reply["id"], 1);
    let session_id = attach_reply["result"]["sessionId"]
        .as_str()
        .unwrap()
        .to_string();

    ws_send(&mut ws, json!({"id":2,"method":"Runtime.evaluate","params":{"expression":"1+1"},"sessionId":session_id})).await;
    let eval_reply = ws_recv_json(&mut ws).await;
    assert_eq!(eval_reply["id"], 2);
    assert_eq!(eval_reply["sessionId"], session_id);

    let calls = backend_clone.calls();
    assert!(
        calls.iter().any(|(tid, method, _)| {
            tid.as_deref() == Some("A") && method == "Runtime.evaluate"
        }),
        "calls: {calls:?}"
    );
}

#[tokio::test]
async fn unknown_method_errors_with_same_id() {
    let backend = FakeBackend::new(two_targets());
    backend.push_response(Err(CdpFailure {
        code: -32601,
        message: "'Nope.nope' wasn't found".to_string(),
    }));
    let (_handle, port) = start_server(backend);
    let mut ws = ws_connect(port, "/devtools/browser/test").await;

    ws_send(&mut ws, json!({"id":7,"method":"Nope.nope","params":{}})).await;
    let reply = ws_recv_json(&mut ws).await;
    assert_eq!(reply["id"], 7);
    assert_eq!(reply["error"]["code"], -32601);
}

#[tokio::test]
async fn foreign_origin_upgrade_refused() {
    let (_handle, port) = start_server(FakeBackend::new(two_targets()));
    let (status, _) = http_get(
        port,
        "/devtools/browser/test",
        "Upgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nOrigin: https://evil.example\r\n",
    );
    assert_eq!(status, 403);
}

#[tokio::test]
async fn bad_host_header_refused() {
    let (_handle, port) = start_server(FakeBackend::new(two_targets()));
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let req = format!(
        "GET /json/version HTTP/1.1\r\nHost: evil.example:{port}\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).unwrap();
    let mut resp = String::new();
    stream.read_to_string(&mut resp).unwrap();
    let status = resp
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    assert_eq!(status, 403);
}

#[tokio::test]
async fn discover_targets_emits_created() {
    let backend = FakeBackend::new(two_targets());
    let (_handle, port) = start_server(backend);
    let mut ws = ws_connect(port, "/devtools/browser/test").await;

    ws_send(
        &mut ws,
        json!({"id":1,"method":"Target.setDiscoverTargets","params":{"discover":true}}),
    )
    .await;
    let mut created = 0;
    for _ in 0..8 {
        let msg = tokio::time::timeout(Duration::from_secs(5), ws_recv_json(&mut ws))
            .await
            .expect("timeout waiting for events/reply");
        if msg.get("id") == Some(&json!(1)) {
            continue;
        }
        if msg.get("method") == Some(&json!("Target.targetCreated")) {
            created += 1;
        }
        if created >= 2 {
            break;
        }
    }
    assert_eq!(created, 2);
}

#[tokio::test(flavor = "current_thread")]
async fn destroyed_target_detaches_session_and_keeps_connection() {
    let backend = FakeBackend::new(two_targets());
    let backend_clone = backend.clone();
    let (_handle, port) = start_server(backend);
    let mut ws = ws_connect(port, "/devtools/browser/test").await;

    ws_send(
        &mut ws,
        json!({"id":1,"method":"Target.setDiscoverTargets","params":{"discover":true}}),
    )
    .await;
    // reply plus two targetCreated events
    let mut seen_reply = false;
    for _ in 0..3 {
        if ws_recv_json(&mut ws).await["id"] == 1 {
            seen_reply = true;
        }
    }
    assert!(seen_reply);
    ws_send(
        &mut ws,
        json!({"id":2,"method":"Target.attachToTarget","params":{"targetId":"A","flatten":true}}),
    )
    .await;
    let mut session_id = Value::Null;
    while session_id.is_null() {
        let msg = ws_recv_json(&mut ws).await;
        if msg["id"] == 2 {
            session_id = msg["result"]["sessionId"].clone();
        }
    }

    backend_clone
        .events_tx
        .send(BackendEvent::TargetDestroyed("A".into()))
        .unwrap();
    let mut methods = Vec::new();
    while methods.len() < 2 {
        let msg = ws_recv_json(&mut ws).await;
        if let Some(m) = msg["method"].as_str() {
            if m == "Target.detachedFromTarget" {
                assert_eq!(msg["params"]["sessionId"], session_id);
            }
            if m.starts_with("Target.") && m != "Target.attachedToTarget" {
                methods.push(m.to_string());
            }
        }
    }
    assert_eq!(methods, vec!["Target.detachedFromTarget", "Target.targetDestroyed"]);

    ws_send(&mut ws, json!({"id":3,"method":"Target.getTargets","params":{}})).await;
    loop {
        let msg = ws_recv_json(&mut ws).await;
        if msg["id"] == 3 {
            assert!(msg["result"]["targetInfos"].is_array());
            break;
        }
    }
}
