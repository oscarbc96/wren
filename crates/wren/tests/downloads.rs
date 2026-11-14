use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use engine::{EngineEvent, FakeEngine};
use gpui::TestAppContext;
use settings::Settings;
use store::MemoryStore;
use url::Url;

use wren::window::BrowserWindow;

fn test_settings(download_dir: &std::path::Path) -> Settings {
    let mut s = Settings::default();
    s.downloads.dir = download_dir.to_string_lossy().into_owned();
    s
}

fn spawn_file_server(body: Vec<u8>, content_type: &str) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let ct = content_type.to_string();
    let handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: {}\r\nConnection: close\r\n\r\n",
                body.len(),
                ct
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.write_all(&body);
        }
    });
    (format!("http://{addr}/file.bin"), handle)
}

fn spawn_slow_server(stop: Arc<AtomicBool>) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let resp = "HTTP/1.1 200 OK\r\nContent-Length: 100000\r\nConnection: close\r\n\r\n";
            let _ = stream.write_all(resp.as_bytes());
            let chunk = [b'a'; 1024];
            for _ in 0..80 {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                if stream.write_all(&chunk).is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
        }
    });
    (format!("http://{addr}/slow.bin"), handle)
}

#[gpui::test]
fn download_event_saves_file(cx: &mut TestAppContext) {
    let tmp = tempfile::TempDir::new().unwrap();
    let (url, server) = spawn_file_server(b"hello world".to_vec(), "application/octet-stream");
    let store = Arc::new(MemoryStore::default());
    let engine = FakeEngine::new();
    let settings = test_settings(tmp.path());
    cx.update(themes::init);
    let (view, cxw) = cx.add_window_view(|_window, cx| {
        BrowserWindow::with_urls_and_opener(
            engine,
            store.clone(),
            Arc::new(creds::MemoryCredStore::default()),
            Arc::new(wren::open_in_chrome::SystemOpener),
            settings,
            Vec::new(),
            cx,
        )
    });

    let webview = cxw.update(|_window, cx| view.update(cx, |w, _| w.active_webview().unwrap()));
    cxw.update(|_window, cx| {
        view.update(cx, |w, _| {
            w.engine.push(EngineEvent::Download {
                id: webview,
                url: Url::parse(&url).unwrap(),
                suggested_name: Some("file.bin".to_string()),
            });
        });
    });

    // tick until downloaded
    for _ in 0..500 {
        cxw.update(|window, cx| {
            view.update(cx, |w, cx| {
                w.tick(window, cx);
            });
        });
        let done = store
            .list_downloads()
            .unwrap()
            .iter()
            .any(|d| d.state == store::DownloadState::Completed);
        if done {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    server.join().ok();

    let path = tmp.path().join("file.bin");
    assert!(path.exists(), "file should land in downloads.dir");
    let contents = std::fs::read_to_string(&path).unwrap();
    assert_eq!(contents, "hello world");
    let rows = store.list_downloads().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].state, store::DownloadState::Completed);

    // second download with same name -> no overwrite
    let (url2, server2) = spawn_file_server(b"second".to_vec(), "application/octet-stream");
    cxw.update(|_window, cx| {
        view.update(cx, |w, _| {
            w.engine.push(EngineEvent::Download {
                id: webview,
                url: Url::parse(&url2).unwrap(),
                suggested_name: Some("file.bin".to_string()),
            });
        });
    });
    for _ in 0..500 {
        cxw.update(|window, cx| {
            view.update(cx, |w, cx| {
                w.tick(window, cx);
            });
        });
        let done = store
            .list_downloads()
            .unwrap()
            .iter()
            .filter(|d| d.state == store::DownloadState::Completed)
            .count()
            >= 2;
        if done {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    server2.join().ok();
    assert!(tmp.path().join("file (1).bin").exists());
}

#[gpui::test]
fn cancel_removes_partial(cx: &mut TestAppContext) {
    let tmp = tempfile::TempDir::new().unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let (url, server) = spawn_slow_server(stop.clone());
    let store = Arc::new(MemoryStore::default());
    let engine = FakeEngine::new();
    let settings = test_settings(tmp.path());
    cx.update(themes::init);
    let (view, cxw) = cx.add_window_view(|_window, cx| {
        BrowserWindow::with_urls_and_opener(
            engine,
            store.clone(),
            Arc::new(creds::MemoryCredStore::default()),
            Arc::new(wren::open_in_chrome::SystemOpener),
            settings,
            Vec::new(),
            cx,
        )
    });

    let webview = cxw.update(|_window, cx| view.update(cx, |w, _| w.active_webview().unwrap()));
    cxw.update(|_window, cx| {
        view.update(cx, |w, _| {
            w.engine.push(EngineEvent::Download {
                id: webview,
                url: Url::parse(&url).unwrap(),
                suggested_name: Some("slow.bin".to_string()),
            });
        });
    });

    // wait for start/progress
    let mut download_id = None;
    for _ in 0..300 {
        cxw.update(|window, cx| {
            view.update(cx, |w, cx| {
                w.tick(window, cx);
            });
        });
        let rows = store.list_downloads().unwrap();
        if let Some(r) = rows.first() {
            download_id = Some(r.id);
            if r.bytes > 0 || r.state == store::DownloadState::InProgress {
                break;
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
    let id = download_id.expect("download started");
    cxw.update(|_window, cx| {
        view.update(cx, |w, _| {
            w.cancel_download(id);
        });
    });
    stop.store(true, Ordering::SeqCst);
    for _ in 0..300 {
        cxw.update(|window, cx| {
            view.update(cx, |w, cx| {
                w.tick(window, cx);
            });
        });
        let rows = store.list_downloads().unwrap();
        if rows
            .iter()
            .any(|d| d.id == id && d.state == store::DownloadState::Cancelled)
        {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    server.join().ok();
    let rows = store.list_downloads().unwrap();
    let row = rows.iter().find(|d| d.id == id).unwrap();
    assert_eq!(row.state, store::DownloadState::Cancelled);
    assert!(
        !tmp.path().join("slow.bin").exists(),
        "partial file should be removed on cancel"
    );
}
