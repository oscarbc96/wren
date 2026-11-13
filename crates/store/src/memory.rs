use std::collections::HashMap;
use std::sync::Mutex;

use crate::{Bookmark, Error, HistoryHit, Result, SessionSnapshot, Store};
// score is crate::score (function). rename to avoid conflict.

#[derive(Default)]
struct Visit {
    url: String,
    url_lc: String,
    title: String,
    title_lc: String,
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
    index: HashMap<String, usize>,
    visits: Vec<Visit>,
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
        if let Some(&idx) = g.index.get(&key) {
            let v = &mut g.visits[idx];
            v.count += 1;
            if !title.is_empty() {
                v.title = title.to_string();
                v.title_lc = title.to_lowercase();
            }
            v.last_visit = at;
        } else {
            let idx = g.visits.len();
            g.visits.push(Visit {
                url: key.clone(),
                url_lc: key.to_lowercase(),
                title: title.to_string(),
                title_lc: title.to_lowercase(),
                count: 1,
                last_visit: at,
            });
            g.index.insert(key, idx);
        }
        Ok(())
    }

    fn set_title(&self, url: &url::Url, title: &str) -> Result<()> {
        let mut g = self.inner.lock().unwrap();
        let key = url.as_str().to_string();
        if let Some(&idx) = g.index.get(&key) {
            let v = &mut g.visits[idx];
            v.title = title.to_string();
            v.title_lc = title.to_lowercase();
        } else {
            let idx = g.visits.len();
            g.visits.push(Visit {
                url: key.clone(),
                url_lc: key.to_lowercase(),
                title: title.to_string(),
                title_lc: title.to_lowercase(),
                count: 0,
                last_visit: jiff::Timestamp::now(),
            });
            g.index.insert(key, idx);
        }
        Ok(())
    }

    fn search_history(&self, q: &str, limit: usize) -> Result<Vec<HistoryHit>> {
        let g = self.inner.lock().unwrap();
        let q = q.trim();
        let terms: Vec<String> = q.split_whitespace().map(|t| t.to_lowercase()).collect();
        let now = jiff::Timestamp::now();
        // Single pass, keep only top `limit` by score; no large allocation.
        let mut best: Vec<(f64, usize)> = Vec::with_capacity(limit);
        let mut min_pos = 0usize;
        let mut have_min = false;
        for (i, v) in g.visits.iter().enumerate() {
            if !terms.is_empty() {
                let mut ok = true;
                for t in &terms {
                    if !(v.title_lc.contains(t) || v.url_lc.contains(t)) {
                        ok = false;
                        break;
                    }
                }
                if !ok {
                    continue;
                }
            }
            let s = crate::score_at(v.count, v.last_visit, now);
            if best.len() < limit {
                best.push((s, i));
                if best.len() == limit {
                    min_pos = 0;
                    for (pos, (sc, _)) in best.iter().enumerate() {
                        if *sc < best[min_pos].0 {
                            min_pos = pos;
                        }
                    }
                    have_min = true;
                }
            } else if have_min && s > best[min_pos].0 {
                best[min_pos] = (s, i);
                min_pos = 0;
                for (pos, (sc, _)) in best.iter().enumerate() {
                    if *sc < best[min_pos].0 {
                        min_pos = pos;
                    }
                }
            }
        }
        best.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let hits: Vec<HistoryHit> = best
            .into_iter()
            .map(|(score, i)| {
                let v = &g.visits[i];
                HistoryHit {
                    url: url::Url::parse(&v.url).unwrap(),
                    title: v.title.clone(),
                    score,
                }
            })
            .collect();
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
