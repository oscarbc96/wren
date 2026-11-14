// Copyright 2024 Obscura contributors (Apache-2.0)
// Modified for Wren: re-targeted to the CdpBackend trait.

use serde_json::json;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use crate::backend::TargetInfo;

pub fn header_value<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    head.lines().skip(1).find_map(|line| {
        let (k, v) = line.split_once(':')?;
        if k.eq_ignore_ascii_case(name) {
            Some(v.trim())
        } else {
            None
        }
    })
}

pub fn host_allowed(head: &str, port: u16) -> bool {
    let Some(host) = header_value(head, "host") else {
        return false;
    };
    host == format!("127.0.0.1:{port}") || host == format!("localhost:{port}")
}

pub fn origin_allowed(head: &str, port: u16) -> bool {
    let Some(origin) = header_value(head, "origin") else {
        return true;
    };
    let origin = origin.trim_end_matches('/');
    origin == format!("http://127.0.0.1:{port}")
        || origin == format!("http://localhost:{port}")
        || origin == "wren://devtools"
}

pub fn is_websocket_upgrade(head: &str) -> bool {
    head.lines()
        .next()
        .map(|l| l.starts_with("GET "))
        .unwrap_or(false)
        && header_value(head, "upgrade")
            .map(|v| v.eq_ignore_ascii_case("websocket"))
            .unwrap_or(false)
}

pub fn is_json_endpoint(target: &str) -> bool {
    let path = target.split('?').next().unwrap_or(target);
    matches!(
        path,
        "/json/version" | "/json/version/" | "/json" | "/json/" | "/json/list" | "/json/list/"
    )
}

pub fn new_target_url(target: &str) -> Option<String> {
    let q = target.split_once('?')?.1;
    if q.is_empty() {
        None
    } else {
        Some(q.to_string())
    }
}

pub fn json_response(target: &str, port: u16, targets: &[TargetInfo]) -> String {
    let path = target.split('?').next().unwrap_or(target);
    match path {
        "/json/version" | "/json/version/" => json!({
            "Browser": format!("Wren/{}", env!("CARGO_PKG_VERSION")),
            "Protocol-Version": "1.3",
            "webSocketDebuggerUrl": format!("ws://127.0.0.1:{port}/devtools/browser/{}", uuid::Uuid::new_v4()),
        })
        .to_string(),
        _ => {
            let list: Vec<_> = targets.iter().map(|t| target_json_value(port, t)).collect();
            serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string())
        }
    }
}

pub fn target_json_value(port: u16, t: &TargetInfo) -> serde_json::Value {
    json!({
        "id": t.target_id,
        "type": "page",
        "url": t.url,
        "title": t.title,
        "webSocketDebuggerUrl": format!("ws://127.0.0.1:{port}/devtools/page/{}", t.target_id),
        "devtoolsFrontendUrl": format!("wren://devtools/inspector.html?ws=127.0.0.1:{port}/devtools/page/{}", t.target_id),
    })
}

pub fn target_json(port: u16, t: &TargetInfo) -> String {
    target_json_value(port, t).to_string()
}

pub async fn write_json(mut stream: TcpStream, body: &str) -> anyhow::Result<()> {
    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(resp.as_bytes()).await?;
    let _ = stream.shutdown().await;
    Ok(())
}

pub async fn write_text(mut stream: TcpStream, body: &str) -> anyhow::Result<()> {
    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(resp.as_bytes()).await?;
    let _ = stream.shutdown().await;
    Ok(())
}

pub async fn write_forbidden(mut stream: TcpStream, reason: &str) -> anyhow::Result<()> {
    let body = format!("{{\"error\":\"{reason}\"}}");
    let resp = format!(
        "HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(resp.as_bytes()).await?;
    let _ = stream.shutdown().await;
    Ok(())
}
