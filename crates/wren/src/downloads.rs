use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;

use store::{DownloadState, Store};
use url::Url;

#[derive(Debug, Clone)]
pub enum DownloadMsg {
    Started {
        id: i64,
        path: String,
        total: Option<u64>,
    },
    Progress {
        id: i64,
        bytes: u64,
        total: Option<u64>,
    },
    Finished {
        id: i64,
        bytes: u64,
        total: Option<u64>,
    },
    Failed {
        id: i64,
        bytes: u64,
        total: Option<u64>,
        error: String,
    },
    Cancelled {
        id: i64,
        bytes: u64,
        total: Option<u64>,
    },
}

pub struct DownloadHandle {
    pub id: i64,
    pub cancel: Arc<AtomicBool>,
}

pub fn choose_path(dir: &Path, suggested: &str) -> PathBuf {
    let _ = fs::create_dir_all(dir);
    let suggested = if suggested.is_empty() {
        "download"
    } else {
        suggested
    };
    let p = Path::new(suggested);
    let stem = p
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("download")
        .to_string();
    let ext = p
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string());
    let candidate = dir.join(suggested);
    if !candidate.exists() {
        return candidate;
    }
    for i in 1..10_000 {
        let name = match &ext {
            Some(e) if !e.is_empty() => format!("{stem} ({i}).{e}"),
            _ => format!("{stem} ({i})"),
        };
        let c = dir.join(name);
        if !c.exists() {
            return c;
        }
    }
    candidate
}

pub fn suggested_name_from_url(url: &Url, suggested: Option<&str>) -> String {
    if let Some(s) = suggested {
        let s = s.trim();
        if !s.is_empty() {
            return s.to_string();
        }
    }
    if let Some(seg) = url
        .path_segments()
        .and_then(|mut s| s.rfind(|x| !x.is_empty()))
    {
        if !seg.is_empty() {
            return seg.to_string();
        }
    }
    "download".to_string()
}

pub fn start_download(
    store: Arc<dyn Store + Send + Sync>,
    url: Url,
    suggested_name: Option<String>,
    dir: PathBuf,
    tx: mpsc::Sender<DownloadMsg>,
) -> DownloadHandle {
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_thread = cancel.clone();
    let name = suggested_name_from_url(&url, suggested_name.as_deref());
    let path = choose_path(&dir, &name);
    let path_str = path.to_string_lossy().to_string();
    let id = store
        .record_download(url.as_str(), &path_str, 0, None, DownloadState::InProgress)
        .unwrap_or(0);
    let _ = tx.send(DownloadMsg::Started {
        id,
        path: path_str.clone(),
        total: None,
    });
    thread::spawn(move || {
        let result = (|| -> Result<(u64, Option<u64>), String> {
            let client = reqwest::blocking::Client::builder()
                .redirect(reqwest::redirect::Policy::limited(10))
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .map_err(|e| e.to_string())?;
            let mut resp = client
                .get(url.clone())
                .send()
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?;
            let total = resp.content_length();
            let mut file = File::create(&path).map_err(|e| e.to_string())?;
            let mut buf = [0u8; 16 * 1024];
            let mut bytes: u64 = 0;
            loop {
                if cancel_thread.load(Ordering::SeqCst) {
                    drop(file);
                    let _ = fs::remove_file(&path);
                    return Err("__cancelled__".to_string());
                }
                let n = resp.read(&mut buf).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
                bytes += n as u64;
                let _ = store.update_download(id, bytes, total, DownloadState::InProgress);
                let _ = tx.send(DownloadMsg::Progress { id, bytes, total });
            }
            file.flush().map_err(|e| e.to_string())?;
            Ok((bytes, total))
        })();
        match result {
            Ok((bytes, total)) => {
                let _ = store.update_download(id, bytes, total, DownloadState::Completed);
                let _ = tx.send(DownloadMsg::Finished { id, bytes, total });
            }
            Err(e) if e == "__cancelled__" => {
                let _ = store.update_download(id, 0, None, DownloadState::Cancelled);
                let _ = tx.send(DownloadMsg::Cancelled {
                    id,
                    bytes: 0,
                    total: None,
                });
            }
            Err(e) => {
                let _ = store.update_download(id, 0, None, DownloadState::Failed);
                let _ = tx.send(DownloadMsg::Failed {
                    id,
                    bytes: 0,
                    total: None,
                    error: e,
                });
            }
        }
    });
    DownloadHandle { id, cancel }
}
