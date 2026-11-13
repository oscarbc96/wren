use std::collections::HashMap;
use std::sync::Mutex;

use crate::{Bookmark, Error, HistoryHit, Result, SessionSnapshot, Store, score as score_fn};
// score is crate::score (function). rename to avoid conflict.

#[derive(Default)]
struct Visit {
    title: String,
    count: i64,
    last_visit: jiff::Timestamp,
}

#[derive(Clone)]
struct BookmarkRow {
    url: String,
    title: String,
    created: jiff::Timestamp,
}

#[derive(Default)]
struct Inner {
    visits: HashMap<String, Visit>,
    bookmarks: Vec<BookmarkRow>,
    session: Option<SessionSnapshot>,
    fail_next: bool,
}

pub struct MemoryStore {
    inner: Mutex<Inner>,
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
        }
    }
}

impl MemoryStore {
    pub fn fail_next_session_save(&self) {
        let mut g = self.inner.lock().unwrap();
        g.fail_next = true;
    }
}

impl Store for MemoryStore {
    fn record_visit(&self, url: &url::Url, title: &str, at: jiff::Timestamp) -> Result<()> {
        let mut g = self.inner.lock().unwrap();
        let key = url.as_str().to_string();
        let v = g.visits.entry(key).or_insert(Visit {
            title: title.to_string(),
            count: 0,
            last_visit: at,
        });
        v.count += 1;
        if !title.is_empty() {
            v.title = title.to_string();
        }
        v.last_visit = at;
        Ok(())
    }

    fn set_title(&self, url: &url::Url, title: &str) -> Result<()> {
        let mut g = self.inner.lock().unwrap();
        let key = url.as_str().to_string();
        let v = g.visits.entry(key).or_insert(Visit {
            title: title.to_string(),
            count: 0,
            last_visit: jiff::Timestamp::now(),
        });
        v.title = title.to_string();
        Ok(())
    }

    fn search_history(&self, q: &str, limit: usize) -> Result<Vec<HistoryHit>> {
        let g = self.inner.lock().unwrap();
        let q = q.trim();
        let terms: Vec<String> = q.split_whitespace().map(|t| t.to_lowercase()).collect();
        let mut hits: Vec<HistoryHit> = g
            .visits
            .iter()
            .filter(|(url, v)| {
                if terms.is_empty() {
                    true
                } else {
                    terms.iter().all(|t| {
                        v.title.to_lowercase().contains(t) || url.to_lowercase().contains(t)
                    })
                }
            })
            .map(|(url, v)| HistoryHit {
                url: url::Url::parse(url).unwrap(),
                title: v.title.clone(),
                score: score_fn(v.count, v.last_visit),
            })
            .collect();
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        hits.truncate(limit);
        Ok(hits)
    }

    fn toggle_bookmark(&self, url: &url::Url, title: &str) -> Result<bool> {
        let mut g = self.inner.lock().unwrap();
        let key = url.as_str();
        if let Some(idx) = g.bookmarks.iter().position(|b| b.url == key) {
            g.bookmarks.remove(idx);
            Ok(false)
        } else {
            g.bookmarks.push(BookmarkRow {
                url: key.to_string(),
                title: title.to_string(),
                created: jiff::Timestamp::now(),
            });
            Ok(true)
        }
    }

    fn is_bookmarked(&self, url: &url::Url) -> Result<bool> {
        let g = self.inner.lock().unwrap();
        Ok(g.bookmarks.iter().any(|b| b.url == url.as_str()))
    }

    fn bookmarks(&self) -> Result<Vec<Bookmark>> {
        let g = self.inner.lock().unwrap();
        Ok(g.bookmarks
            .iter()
            .map(|b| Bookmark {
                url: url::Url::parse(&b.url).unwrap(),
                title: b.title.clone(),
                created: b.created,
            })
            .collect())
    }

    fn search_bookmarks(&self, q: &str, limit: usize) -> Result<Vec<Bookmark>> {
        let g = self.inner.lock().unwrap();
        let terms: Vec<String> = q.split_whitespace().map(|t| t.to_lowercase()).collect();
        let mut out: Vec<Bookmark> = g
            .bookmarks
            .iter()
            .filter(|b| {
                if terms.is_empty() {
                    true
                } else {
                    terms.iter().all(|t| {
                        b.title.to_lowercase().contains(t) || b.url.to_lowercase().contains(t)
                    })
                }
            })
            .map(|b| Bookmark {
                url: url::Url::parse(&b.url).unwrap(),
                title: b.title.clone(),
                created: b.created,
            })
            .collect();
        out.truncate(limit);
        Ok(out)
    }

    fn save_session(&self, s: &SessionSnapshot) -> Result<()> {
        let mut g = self.inner.lock().unwrap();
        let forced = g.fail_next
            || std::env::var("WREN_STORE_FAIL_SESSION_SAVE")
                .map(|v| !v.is_empty())
                .unwrap_or(false);
        if std::env::var("WREN_STORE_FAIL_SESSION_SAVE").is_ok() {
            unsafe {
                std::env::remove_var("WREN_STORE_FAIL_SESSION_SAVE");
            }
        }
        if forced {
            g.fail_next = false;
            return Err(Error::Invalid("forced session save failure".to_string()));
        }
        g.session = Some(s.clone());
        Ok(())
    }

    fn load_session(&self) -> Result<Option<SessionSnapshot>> {
        let g = self.inner.lock().unwrap();
        Ok(g.session.clone())
    }
}
