//! Persistence for history, bookmarks and session.

pub mod memory;
pub mod sqlite;

pub use memory::MemoryStore;
pub use sqlite::SqliteStore;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid data: {0}")]
    Invalid(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionTab {
    pub url: Url,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SessionSnapshot {
    pub tabs: Vec<SessionTab>,
    pub active: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HistoryHit {
    pub url: Url,
    pub title: String,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bookmark {
    pub url: Url,
    pub title: String,
    pub created: jiff::Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadState {
    InProgress,
    Completed,
    Cancelled,
    Failed,
}

impl DownloadState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "in_progress" => Some(Self::InProgress),
            "completed" => Some(Self::Completed),
            "cancelled" => Some(Self::Cancelled),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadRow {
    pub id: i64,
    pub url: String,
    pub path: String,
    pub bytes: u64,
    pub total: Option<u64>,
    pub state: DownloadState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionRow {
    pub id: String,
    pub version: String,
    pub manifest_json: String,
    pub path: String,
    pub enabled: bool,
}

pub trait Store {
    fn record_visit(&self, url: &Url, title: &str, at: jiff::Timestamp) -> Result<()>;
    fn set_title(&self, url: &Url, title: &str) -> Result<()>;
    fn search_history(&self, q: &str, limit: usize) -> Result<Vec<HistoryHit>>;
    fn toggle_bookmark(&self, url: &Url, title: &str) -> Result<bool>;
    fn is_bookmarked(&self, url: &Url) -> Result<bool>;
    fn bookmarks(&self) -> Result<Vec<Bookmark>>;
    fn search_bookmarks(&self, q: &str, limit: usize) -> Result<Vec<Bookmark>>;
    fn save_session(&self, s: &SessionSnapshot) -> Result<()>;
    fn load_session(&self) -> Result<Option<SessionSnapshot>>;
    fn record_download(
        &self,
        url: &str,
        path: &str,
        bytes: u64,
        total: Option<u64>,
        state: DownloadState,
    ) -> Result<i64>;
    fn list_downloads(&self) -> Result<Vec<DownloadRow>>;
    fn update_download(
        &self,
        id: i64,
        bytes: u64,
        total: Option<u64>,
        state: DownloadState,
    ) -> Result<()>;

    fn upsert_extension(
        &self,
        id: &str,
        version: &str,
        manifest_json: &str,
        path: &str,
        enabled: bool,
    ) -> Result<()>;
    fn list_extensions(&self) -> Result<Vec<ExtensionRow>>;
    fn get_extension(&self, id: &str) -> Result<Option<ExtensionRow>>;
    fn set_extension_enabled(&self, id: &str, enabled: bool) -> Result<()>;
    fn remove_extension(&self, id: &str) -> Result<()>;
}

pub(crate) fn score_at(count: i64, last_visit: jiff::Timestamp, now: jiff::Timestamp) -> f64 {
    let age_days = age_days(last_visit, now);
    let decay = 2.0_f64.powf(-age_days / 30.0);
    count as f64 * decay
}

pub(crate) fn age_days(last_visit: jiff::Timestamp, now: jiff::Timestamp) -> f64 {
    if last_visit >= now {
        return 0.0;
    }
    let d = now.duration_since(last_visit);
    let secs = d.as_secs() as f64 + d.subsec_nanos() as f64 / 1_000_000_000.0;
    secs / 86400.0
}

pub(crate) fn ts_to_micros(ts: jiff::Timestamp) -> i64 {
    ts.as_microsecond()
}

pub(crate) fn micros_to_ts(micros: i64) -> jiff::Timestamp {
    jiff::Timestamp::from_microsecond(micros).unwrap_or(jiff::Timestamp::UNIX_EPOCH)
}
