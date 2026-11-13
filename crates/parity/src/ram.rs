use crate::workloads;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

pub type Pid = sysinfo::Pid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProcInfo {
    pub pid: Pid,
    pub parent: Option<Pid>,
    pub rss_bytes: u64,
}

pub fn tree_rss(root: Pid, procs: &[ProcInfo]) -> u64 {
    let mut in_tree = std::collections::HashSet::new();
    in_tree.insert(root);
    let mut changed = true;
    while changed {
        changed = false;
        for p in procs {
            if let Some(parent) = p.parent
                && in_tree.contains(&parent)
                && !in_tree.contains(&p.pid)
            {
                in_tree.insert(p.pid);
                changed = true;
            }
        }
    }
    procs
        .iter()
        .filter(|p| in_tree.contains(&p.pid))
        .map(|p| p.rss_bytes)
        .sum()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RamResult {
    pub browser: String,
    pub workload: String,
    pub version: String,
    pub rss_median_bytes: u64,
    pub samples: Vec<u64>,
    pub date: String,
}

pub fn run_ram(browser: &str, workload: &str, out: &Path) -> Result<RamResult> {
    let urls = workloads::load_workload(workload)?;
    let tmp = tempfile::tempdir().context("temp profile")?;
    let bin = browser_bin(browser)?;
    let mut cmd = Command::new(&bin);
    match browser {
        "wren" => {
            cmd.arg("--profile-dir").arg(tmp.path());
        }
        "chrome" => {
            cmd.arg(format!("--user-data-dir={}", tmp.path().display()));
            cmd.arg("--no-first-run");
            cmd.arg("--no-default-browser-check");
        }
        other => return Err(anyhow::anyhow!("unknown browser: {other}")),
    }
    for url in &urls {
        cmd.arg(url);
    }

    let mut child = cmd.spawn().context("spawn browser")?;
    std::thread::sleep(std::time::Duration::from_secs(30));

    let root_pid = sysinfo::Pid::from_u32(child.id());
    let mut samples = Vec::new();
    for _ in 0..10 {
        samples.push(sample_tree_rss(root_pid));
        std::thread::sleep(std::time::Duration::from_secs(1));
    }

    let _ = child.kill();
    let _ = child.wait();

    let mut sorted = samples.clone();
    sorted.sort();
    let median = if sorted.is_empty() {
        0
    } else {
        sorted[sorted.len() / 2]
    };

    let result = RamResult {
        browser: browser.to_string(),
        workload: workload.to_string(),
        version: browser_version(&bin),
        rss_median_bytes: median,
        samples,
        date: today_date(),
    };

    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).context("create out dir")?;
    }
    std::fs::write(out, serde_json::to_string_pretty(&result)?).context("write result")?;
    Ok(result)
}

fn sample_tree_rss(root: Pid) -> u64 {
    let mut sys = sysinfo::System::new_all().expect("system");
    sys.refresh_all();
    let procs: Vec<ProcInfo> = sys
        .processes()
        .iter()
        .map(|(pid, p)| ProcInfo {
            pid: *pid,
            parent: p.parent(),
            rss_bytes: p.memory(),
        })
        .collect();
    tree_rss(root, &procs)
}

fn browser_bin(browser: &str) -> Result<PathBuf> {
    match browser {
        "wren" => {
            if let Ok(p) = std::env::var("WREN_BIN") {
                return Ok(PathBuf::from(p));
            }
            for c in ["target/release/wren", "target/debug/wren"] {
                if Path::new(c).exists() {
                    return Ok(PathBuf::from(c));
                }
            }
            Ok(PathBuf::from("target/release/wren"))
        }
        "chrome" => Ok(PathBuf::from(
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        )),
        other => Err(anyhow::anyhow!("unknown browser: {other}")),
    }
}

fn browser_version(bin: &Path) -> String {
    if let Ok(out) = Command::new(bin).arg("--version").output() {
        let s = String::from_utf8_lossy(&out.stdout);
        let s = s.trim();
        if !s.is_empty() {
            return s.to_string();
        }
        let s = String::from_utf8_lossy(&out.stderr);
        let s = s.trim();
        if !s.is_empty() {
            return s.to_string();
        }
    }
    "unknown".to_string()
}

fn today_date() -> String {
    jiff::Zoned::now().date().to_string()
}
