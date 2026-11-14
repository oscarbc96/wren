use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;

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
        return seg.to_string();
    }
    "download".to_string()
}

/// Bytes between two `Progress` messages.
const PROGRESS_STEP: u64 = 256 * 1024;

/// Fetches `url` into `path` on a background thread. The thread never
/// touches the store: it reports through `tx` and the window records the
/// result (the store is not shared across threads).
pub fn start_download(
    id: i64,
    url: Url,
    path: PathBuf,
    tx: mpsc::Sender<DownloadMsg>,
) -> DownloadHandle {
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_thread = cancel.clone();
    let _ = tx.send(DownloadMsg::Started {
        id,
        path: path.to_string_lossy().into_owned(),
        total: None,
    });
    thread::spawn(move || {
        let mut bytes: u64 = 0;
        let mut total: Option<u64> = None;
        let result = (|| -> Result<(), String> {
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
            total = resp.content_length();
            let mut file = File::create(&path).map_err(|e| e.to_string())?;
            let mut buf = [0u8; 16 * 1024];
            let mut last_report = 0u64;
            loop {
                if cancel_thread.load(Ordering::SeqCst) {
                    drop(file);
                    let _ = fs::remove_file(&path);
                    return Err(CANCELLED.to_string());
                }
                let n = resp.read(&mut buf).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
                bytes += n as u64;
                if bytes - last_report >= PROGRESS_STEP {
                    last_report = bytes;
                    let _ = tx.send(DownloadMsg::Progress { id, bytes, total });
                }
            }
            file.flush().map_err(|e| e.to_string())?;
            Ok(())
        })();
        let msg = match result {
            Ok(()) => DownloadMsg::Finished { id, bytes, total },
            Err(e) if e == CANCELLED => DownloadMsg::Cancelled { id, bytes, total },
            Err(error) => {
                let _ = fs::remove_file(&path);
                DownloadMsg::Failed {
                    id,
                    bytes,
                    total,
                    error,
                }
            }
        };
        let _ = tx.send(msg);
    });
    DownloadHandle { id, cancel }
}

const CANCELLED: &str = "__cancelled__";
