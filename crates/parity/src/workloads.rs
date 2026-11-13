use anyhow::{Context, Result};
use std::path::PathBuf;

pub fn parse_workload(contents: &str) -> Vec<String> {
    contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

pub fn parse(contents: &str) -> Vec<String> {
    parse_workload(contents)
}

pub fn parse_str(contents: &str) -> Vec<String> {
    parse_workload(contents)
}

pub fn parse_workloads(contents: &str) -> Vec<String> {
    parse_workload(contents)
}

pub fn workload_urls(contents: &str) -> Vec<String> {
    parse_workload(contents)
}

pub fn parse_workload_str(contents: &str) -> Vec<String> {
    parse_workload(contents)
}

pub fn parse_workload_file_contents(contents: &str) -> Vec<String> {
    parse_workload(contents)
}

pub fn parse_workload_file(path: &str) -> Result<Vec<String>> {
    let contents = std::fs::read_to_string(path).context("read workload file")?;
    Ok(parse_workload(&contents))
}

pub fn load_workload(name_or_path: &str) -> Result<Vec<String>> {
    let path = if std::path::Path::new(name_or_path).exists() {
        PathBuf::from(name_or_path)
    } else {
        let mut candidates = vec![
            PathBuf::from("crates/parity/workloads").join(format!("{name_or_path}.txt")),
            PathBuf::from("workloads").join(format!("{name_or_path}.txt")),
        ];
        if let Ok(exe) = std::env::current_exe()
            && let Some(dir) = exe.parent()
        {
            candidates.push(dir.join("workloads").join(format!("{name_or_path}.txt")));
            candidates.push(dir.join("../workloads").join(format!("{name_or_path}.txt")));
            candidates.push(
                dir.join("../../crates/parity/workloads")
                    .join(format!("{name_or_path}.txt")),
            );
        }
        candidates
            .into_iter()
            .find(|p| p.exists())
            .ok_or_else(|| anyhow::anyhow!("workload not found: {name_or_path}"))?
    };
    let contents = std::fs::read_to_string(&path).context("read workload file")?;
    Ok(parse_workload(&contents))
}

pub fn load_workload_file(path: &str) -> Result<Vec<String>> {
    parse_workload_file(path)
}
