use std::process::Command;
use std::sync::{Arc, Mutex};

use url::Url;

pub fn rule_matches(rules: &[String], url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    rules.iter().any(|rule| host_matches(rule, &host))
}

fn host_matches(rule: &str, host: &str) -> bool {
    let rule = rule.trim().to_ascii_lowercase();
    if let Some(suffix) = rule.strip_prefix("*.") {
        host == suffix || host.ends_with(&format!(".{suffix}"))
    } else {
        host == rule || host.ends_with(&format!(".{rule}"))
    }
}

pub trait Opener {
    fn open(&self, url: &Url) -> Result<(), String>;
}

pub struct SystemOpener;

impl Opener for SystemOpener {
    fn open(&self, url: &Url) -> Result<(), String> {
        let url_str = url.as_str();
        let chrome = Command::new("open")
            .args(["-a", "Google Chrome", url_str])
            .status();
        match chrome {
            Ok(status) if status.success() => Ok(()),
            _ => {
                let fallback = Command::new("open").arg(url_str).status();
                match fallback {
                    Ok(status) if status.success() => {
                        Err("Google Chrome not found, opened in default browser".to_string())
                    }
                    _ => Err("Failed to open URL in a browser".to_string()),
                }
            }
        }
    }
}

#[derive(Default)]
pub struct RecordingOpener {
    opened: Arc<Mutex<Vec<Url>>>,
}

impl RecordingOpener {
    pub fn new(opened: Arc<Mutex<Vec<Url>>>) -> Self {
        Self { opened }
    }

    pub fn urls(&self) -> Vec<Url> {
        self.opened.lock().unwrap().clone()
    }
}

impl Opener for RecordingOpener {
    fn open(&self, url: &Url) -> Result<(), String> {
        self.opened.lock().unwrap().push(url.clone());
        Ok(())
    }
}
