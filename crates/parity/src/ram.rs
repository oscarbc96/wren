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
    pub phys_footprint_bytes: u64,
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

pub fn tree_footprint(root: Pid, procs: &[ProcInfo]) -> u64 {
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
        .map(|p| p.phys_footprint_bytes)
        .sum()
}

pub fn profile_footprint(root: Pid, procs: &[ProcInfo]) -> u64 {
    tree_footprint(root, procs)
}

pub fn footprint(root: Pid, procs: &[ProcInfo]) -> u64 {
    tree_footprint(root, procs)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RamResult {
    pub browser: String,
    pub workload: String,
    pub version: String,
    pub rss_median_bytes: u64,
    pub phys_footprint_median_bytes: u64,
    pub footprint_median_bytes: u64,
    pub samples: Vec<u64>,
    pub phys_footprint_samples: Vec<u64>,
    pub footprint_samples: Vec<u64>,
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

    // Optional settings override for memory experiments.
    if let Ok(settings) = std::env::var("WREN_SETTINGS_JSON") {
        let _ = std::fs::write(tmp.path().join("settings.json"), settings);
    }

    let mut child = cmd.spawn().context("spawn browser")?;
    std::thread::sleep(std::time::Duration::from_secs(30));

    let root_pid = sysinfo::Pid::from_u32(child.id());
    let mut rss_samples = Vec::new();
    let mut footprint_samples = Vec::new();
    for _ in 0..10 {
        let (rss, footprint) = sample_tree(root_pid);
        rss_samples.push(rss);
        footprint_samples.push(footprint);
        std::thread::sleep(std::time::Duration::from_secs(1));
    }

    let _ = child.kill();
    let _ = child.wait();

    let mut sorted_rss = rss_samples.clone();
    sorted_rss.sort();
    let rss_median = if sorted_rss.is_empty() {
        0
    } else {
        sorted_rss[sorted_rss.len() / 2]
    };
    let mut sorted_fp = footprint_samples.clone();
    sorted_fp.sort();
    let fp_median = if sorted_fp.is_empty() {
        0
    } else {
        sorted_fp[sorted_fp.len() / 2]
    };

    let result = RamResult {
        browser: browser.to_string(),
        workload: workload.to_string(),
        version: browser_version(&bin),
        rss_median_bytes: rss_median,
        phys_footprint_median_bytes: fp_median,
        footprint_median_bytes: fp_median,
        samples: rss_samples.clone(),
        phys_footprint_samples: footprint_samples.clone(),
        footprint_samples: footprint_samples.clone(),
        date: today_date(),
    };

    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).context("create out dir")?;
    }
    std::fs::write(out, serde_json::to_string_pretty(&result)?).context("write result")?;
    Ok(result)
}

fn sample_tree(root: Pid) -> (u64, u64) {
    let mut sys = sysinfo::System::new_all().expect("system");
    sys.refresh_all();
    let procs: Vec<ProcInfo> = sys
        .processes()
        .iter()
        .map(|(pid, p)| ProcInfo {
            pid: *pid,
            parent: p.parent(),
            rss_bytes: p.memory(),
            phys_footprint_bytes: phys_footprint(pid.as_u32()),
        })
        .collect();
    (tree_rss(root, &procs), tree_footprint(root, &procs))
}

fn phys_footprint(pid: u32) -> u64 {
    unsafe {
        let mut info: libc::rusage_info_v4 = std::mem::zeroed();
        if libc::proc_pid_rusage(
            pid as libc::c_int,
            libc::RUSAGE_INFO_V4,
            &mut info as *mut _ as *mut libc::rusage_info_t,
        ) == 0
        {
            info.ri_phys_footprint
        } else {
            0
        }
    }
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
