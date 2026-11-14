//! Ad and tracker blocking rules for Wren.
//!
//! Wraps Brave's `adblock` crate behind a small, stable API. Filter lists
//! are never committed to the repository (licensing); [`update_lists`]
//! downloads them into the data directory.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use adblock::Engine as AdblockEngine;
use adblock::lists::{FilterSet, ParseOptions};
use adblock::request::Request;
use url::Url;

/// Kind of network request, used to apply type-specific rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestKind {
    Document,
    Script,
    Image,
    Stylesheet,
    Xhr,
    Font,
    Media,
    Other,
}

/// A compiled set of blocking rules.
pub struct Blocker {
    engine: AdblockEngine,
}

impl Blocker {
    /// Build a blocker from one or more filter lists given as text.
    pub fn from_lists(lists: &[&str]) -> Self {
        let mut filter_set = FilterSet::new(false);
        for list in lists {
            filter_set.add_filter_list((*list).to_string(), ParseOptions::default());
        }
        Self {
            engine: AdblockEngine::new_with_filter_set(filter_set),
        }
    }

    /// Whether the request should be blocked.
    ///
    /// Documents are never blocked: only subresources can be.
    pub fn should_block(&self, url: &Url, source: &Url, kind: RequestKind) -> bool {
        if kind == RequestKind::Document {
            return false;
        }
        let request_type = match kind {
            RequestKind::Document => "document",
            RequestKind::Script => "script",
            RequestKind::Image => "image",
            RequestKind::Stylesheet => "stylesheet",
            RequestKind::Xhr => "xmlhttprequest",
            RequestKind::Font => "font",
            RequestKind::Media => "media",
            RequestKind::Other => "other",
        };
        let request = match Request::new(url.as_str(), source.as_str(), request_type, "GET") {
            Ok(r) => r,
            Err(_) => return false,
        };
        self.engine.check_network_request(&request).should_block()
    }
}

/// Download EasyList and EasyPrivacy into `dir`, at most once a week.
///
/// Returns the paths of the cached list files.
pub fn update_lists(dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dir)?;
    let week = Duration::from_secs(7 * 24 * 60 * 60);
    let now = SystemTime::now();
    let lists = [
        ("easylist.txt", "https://easylist.to/easylist/easylist.txt"),
        (
            "easyprivacy.txt",
            "https://easylist.to/easylist/easyprivacy.txt",
        ),
    ];
    let mut out = Vec::new();
    for (name, url) in lists {
        let path = dir.join(name);
        let fresh = path
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|m| now.duration_since(m).ok())
            .map(|age| age < week)
            .unwrap_or(false);
        if !fresh {
            let body = reqwest::blocking::get(url)?.text()?;
            std::fs::write(&path, body)?;
        }
        out.push(path);
    }
    Ok(out)
}
