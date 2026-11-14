use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HarnessEntry {
    pub url: String,
    pub final_url: String,
    pub title: String,
    pub load_ms: u64,
    pub console_errors: Vec<String>,
    pub screenshot_png: String,
    pub crashed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HarnessReport {
    pub entries: Vec<HarnessEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MustWork {
    pub name: String,
    pub url: String,
    pub probe: String,
}

#[derive(Debug, Clone, Deserialize)]
struct MustWorkFile {
    #[serde(default)]
    site: Vec<MustWork>,
}

/// usable = loaded and not crashed and (ssim >= 0.6 or probe passed)
pub fn usable(loaded: bool, crashed: bool, ssim: Option<f64>, probe_passed: Option<bool>) -> bool {
    if !loaded || crashed {
        return false;
    }
    if probe_passed == Some(true) {
        return true;
    }
    match ssim {
        Some(v) => v >= 0.6,
        None => false,
    }
}

pub fn parse_harness_report(json: &str) -> anyhow::Result<HarnessReport> {
    Ok(serde_json::from_str(json)?)
}

pub fn parse_must_work_toml(text: &str) -> anyhow::Result<Vec<MustWork>> {
    let file: MustWorkFile = toml::from_str(text)?;
    Ok(file.site)
}

pub fn write_harness_report(path: &std::path::Path, report: &HarnessReport) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(report)?;
    std::fs::write(path, json)?;
    Ok(())
}

use anyhow::Context;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn run_topsites_command(
    out_dir: &Path,
    must_work: bool,
    profile_dir: Option<&Path>,
    limit: Option<usize>,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(out_dir)?;
    let (urls, must_sites) = if must_work {
        let profile = profile_dir.context("--profile-dir required with --must-work")?;
        let text = include_str!("../data/must_work.toml");
        let sites = parse_must_work_toml(text)?;
        let urls = sites.iter().map(|s| s.url.clone()).collect();
        (urls, Some((sites, profile.to_path_buf())))
    } else {
        let text = include_str!("../data/topsites.txt");
        let mut urls: Vec<String> = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(|h| format!("https://{h}"))
            .collect();
        if let Some(n) = limit {
            urls.truncate(n);
        }
        (urls, None)
    };

    let wren_json = out_dir.join("wren-harness.json");
    run_wren_harness(
        &wren_json,
        &urls,
        must_sites.as_ref().map(|(_, p)| p.as_path()),
    )?;
    let wren_report = parse_harness_report(&std::fs::read_to_string(&wren_json)?)?;

    let chrome_dir = out_dir.join("chrome");
    std::fs::create_dir_all(&chrome_dir)?;
    let chrome_paths = capture_chrome_screenshots(&chrome_dir, &urls);

    let report_path = report_path_for(out_dir);
    let md = render_topsites_report(
        &wren_report,
        &urls,
        &chrome_paths,
        must_sites.as_ref().map(|(s, _)| s.as_slice()),
    );
    if let Some(parent) = report_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&report_path, md)?;
    println!("wrote {}", report_path.display());
    Ok(())
}

fn run_wren_harness(
    out_json: &Path,
    urls: &[String],
    profile_dir: Option<&Path>,
) -> anyhow::Result<()> {
    let bin = wren_bin_path()?;
    let mut cmd = Command::new(bin);
    cmd.arg("--harness").arg(out_json);
    if let Some(p) = profile_dir {
        cmd.arg("--profile-dir").arg(p);
    } else {
        let prof = out_json.parent().unwrap_or(Path::new(".")).join("profile");
        std::fs::create_dir_all(&prof)?;
        cmd.arg("--profile-dir").arg(prof);
    }
    for u in urls {
        cmd.arg(u);
    }
    let status = cmd.status().context("run wren --harness")?;
    if !status.success() {
        anyhow::bail!("wren --harness failed: {status}");
    }
    Ok(())
}

fn wren_bin_path() -> anyhow::Result<PathBuf> {
    if let Ok(p) = std::env::var("WREN_BIN") {
        return Ok(PathBuf::from(p));
    }
    for c in ["target/release/wren", "target/debug/wren"] {
        if Path::new(c).exists() {
            return Ok(PathBuf::from(c));
        }
    }
    Err(anyhow::anyhow!(
        "wren binary not found; set WREN_BIN or build target/{{release,debug}}/wren"
    ))
}

fn chrome_bin_path() -> PathBuf {
    PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")
}

fn capture_chrome_screenshots(dir: &Path, urls: &[String]) -> Vec<Option<PathBuf>> {
    let chrome = chrome_bin_path();
    urls.iter()
        .enumerate()
        .map(|(i, url)| capture_single_chrome_screenshot(&chrome, dir, i, url))
        .collect()
}

fn capture_single_chrome_screenshot(
    chrome: &Path,
    dir: &Path,
    i: usize,
    url: &str,
) -> Option<PathBuf> {
    let out = dir.join(format!("chrome-{i}.png"));
    let status = Command::new(chrome)
        .arg("--headless=new")
        .arg(format!("--screenshot={}", out.display()))
        .arg("--window-size=1280,720")
        .arg("--hide-scrollbars")
        .arg("--virtual-time-budget=20000")
        .arg(url)
        .status()
        .ok()?;
    if status.success() && out.exists() {
        Some(out)
    } else {
        None
    }
}

fn compare_screenshots_ssim(a: &Path, b: &Path) -> Option<f64> {
    let img1 = image::open(a).ok()?.to_rgb8();
    let img2 = image::open(b).ok()?.to_rgb8();
    if img1.dimensions() != img2.dimensions() {
        return None;
    }
    let result = image_compare::rgb_hybrid_compare(&img1, &img2).ok()?;
    Some(result.score)
}

fn render_topsites_report(
    wren: &HarnessReport,
    urls: &[String],
    chrome_paths: &[Option<PathBuf>],
    must_sites: Option<&[MustWork]>,
) -> String {
    let date = jiff::Zoned::now().date().to_string();
    let mut out = String::new();
    out.push_str(&format!("# Top-sites report {date}\n\n"));
    out.push_str("| URL | Wren | Chrome shot | SSIM | Usable |\n");
    out.push_str("|---|---|---|---:|---|\n");
    for (i, url) in urls.iter().enumerate() {
        let entry = wren.entries.get(i);
        let chrome = chrome_paths.get(i).and_then(|p| p.as_ref());
        let (wren_status, ssim, usable_flag) = match entry {
            Some(e) => {
                let loaded = !e.crashed && e.load_ms < 20000;
                let ssim = chrome.and_then(|cp| {
                    let wp = PathBuf::from(&e.screenshot_png);
                    let wp = if wp.is_absolute() {
                        wp
                    } else {
                        // screenshot_png may be relative to report dir; try as-is
                        wp
                    };
                    compare_screenshots_ssim(&wp, cp)
                });
                let _has_probe = must_sites
                    .and_then(|sites| sites.get(i))
                    .map(|s| !s.probe.is_empty())
                    .unwrap_or(false);
                let u = usable(loaded, e.crashed, ssim, None);
                let status = if e.crashed {
                    "crashed".to_string()
                } else if loaded {
                    format!("ok ({} ms)", e.load_ms)
                } else {
                    "timeout/fail".to_string()
                };
                (status, ssim, u)
            }
            None => ("missing".to_string(), None, false),
        };
        let ssim_str = ssim
            .map(|v| format!("{v:.3}"))
            .unwrap_or_else(|| "-".to_string());
        let chrome_str = if chrome.is_some() { "yes" } else { "no" };
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            url,
            wren_status,
            chrome_str,
            ssim_str,
            if usable_flag { "yes" } else { "no" }
        ));
    }
    out
}

fn report_path_for(out_dir: &Path) -> PathBuf {
    let date = jiff::Zoned::now().date().to_string();
    let name = format!("{date}-topsites.md");
    // prefer docs/research/perf/<date>-topsites.md in cwd if docs exists, else in out_dir
    let preferred = Path::new("docs/research/perf").join(&name);
    if Path::new("docs/research/perf").exists()
        || std::fs::create_dir_all("docs/research/perf").is_ok()
    {
        preferred
    } else {
        out_dir.join(name)
    }
}
