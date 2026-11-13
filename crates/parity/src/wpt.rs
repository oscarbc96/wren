use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Deserialize)]
pub struct WptReport {
    #[serde(default)]
    pub results: Vec<WptResult>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WptResult {
    #[serde(default)]
    pub test: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub subtests: Vec<WptSubtest>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WptSubtest {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rate {
    pub passed: usize,
    pub total: usize,
}

#[derive(Debug, Clone)]
pub struct DirGap {
    pub dir: String,
    pub servo: Rate,
    pub chrome: Rate,
    pub gap: f64,
}

pub fn pass_rates(report: &WptReport) -> BTreeMap<String, Rate> {
    let mut map: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for r in &report.results {
        let dir = top_level_dir(&r.test);
        let entry = map.entry(dir).or_insert((0, 0));
        if r.subtests.is_empty() {
            entry.1 += 1;
            if is_pass(&r.status) {
                entry.0 += 1;
            }
        } else {
            for s in &r.subtests {
                entry.1 += 1;
                if is_pass(&s.status) {
                    entry.0 += 1;
                }
            }
        }
    }
    map.into_iter()
        .map(|(k, (p, t))| {
            (
                k,
                Rate {
                    passed: p,
                    total: t,
                },
            )
        })
        .collect()
}

pub fn read_report(path: &std::path::Path) -> anyhow::Result<WptReport> {
    let contents = std::fs::read_to_string(path)?;
    let report: WptReport = serde_json::from_str(&contents)?;
    Ok(report)
}

pub fn is_pass(status: &str) -> bool {
    matches!(status.to_ascii_lowercase().as_str(), "pass" | "ok")
}

fn top_level_dir(test: &str) -> String {
    let t = test.trim().trim_start_matches('/');
    t.split('/').next().unwrap_or("").to_string()
}

fn pct(rate: &Rate) -> f64 {
    if rate.total == 0 {
        0.0
    } else {
        rate.passed as f64 * 100.0 / rate.total as f64
    }
}

pub fn diff_list(servo: &WptReport, chrome: &WptReport) -> Vec<DirGap> {
    let s = pass_rates(servo);
    let c = pass_rates(chrome);
    let mut dirs: BTreeSet<String> = BTreeSet::new();
    dirs.extend(s.keys().cloned());
    dirs.extend(c.keys().cloned());
    let mut out: Vec<DirGap> = dirs
        .into_iter()
        .map(|d| {
            let sr = s.get(&d).cloned().unwrap_or(Rate {
                passed: 0,
                total: 0,
            });
            let cr = c.get(&d).cloned().unwrap_or(Rate {
                passed: 0,
                total: 0,
            });
            let gap = (pct(&sr) - pct(&cr)).abs();
            DirGap {
                dir: d,
                servo: sr,
                chrome: cr,
                gap,
            }
        })
        .collect();
    out.sort_by(|a, b| {
        b.gap
            .partial_cmp(&a.gap)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.dir.cmp(&b.dir))
    });
    out
}

pub fn dirs_sorted_by_gap(servo: &WptReport, chrome: &WptReport) -> Vec<String> {
    diff_list(servo, chrome)
        .into_iter()
        .map(|d| d.dir)
        .collect()
}

pub fn diff_markdown(servo: &WptReport, chrome: &WptReport) -> String {
    let rows = diff_list(servo, chrome);
    let mut out = String::new();
    out.push_str("| Directory | Servo | Chrome | Gap (pp) |\n");
    out.push_str("|---|---|---|---|\n");
    for r in rows {
        out.push_str(&format!(
            "| {} | {}/{} ({:.1}%) | {}/{} ({:.1}%) | {:.1} |\n",
            r.dir,
            r.servo.passed,
            r.servo.total,
            pct(&r.servo),
            r.chrome.passed,
            r.chrome.total,
            pct(&r.chrome),
            r.gap
        ));
    }
    out
}

pub fn diff(servo: &WptReport, chrome: &WptReport) -> String {
    diff_markdown(servo, chrome)
}

pub fn wpt_diff(servo: &WptReport, chrome: &WptReport) -> String {
    diff_markdown(servo, chrome)
}

pub fn diff_reports(servo: &WptReport, chrome: &WptReport) -> String {
    diff_markdown(servo, chrome)
}

pub fn compare(servo: &WptReport, chrome: &WptReport) -> String {
    diff_markdown(servo, chrome)
}
