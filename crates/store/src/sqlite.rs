use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use rusqlite::{Connection, params};

use crate::{
    Bookmark, Error, HistoryHit, Result, SessionSnapshot, Store, micros_to_ts, score_at,
    ts_to_micros,
};

pub struct SqliteStore {
    conn: Mutex<Connection>,
    fail_next: AtomicBool,
}

impl SqliteStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA foreign_keys = ON;",
        )?;
        migrate(&mut conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
            fail_next: AtomicBool::new(false),
        })
    }

    pub fn fail_next_session_save(&self) {
        self.fail_next.store(true, Ordering::SeqCst);
    }
}

fn migrate(conn: &mut Connection) -> Result<()> {
    let mut current: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
        .unwrap_or(0);
    if current < 1 {
        let sql = include_str!("../migrations/001_init.sql");
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.execute_batch("PRAGMA user_version = 1")?;
        tx.commit()?;
        current = 1;
    }
    if current < 2 {
        let sql = include_str!("../migrations/002_downloads.sql");
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.execute_batch("PRAGMA user_version = 2")?;
        tx.commit()?;
        current = 2;
    }
    if current < 3 {
        let sql = include_str!("../migrations/003_extensions.sql");
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.execute_batch("PRAGMA user_version = 3")?;
        tx.commit()?;
    }
    Ok(())
}

fn build_fts(q: &str) -> String {
    q.split_whitespace()
        .map(|t| {
            let t = t.replace('"', "\"\"");
            format!("\"{}\"", t)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

impl Store for SqliteStore {
    fn record_visit(&self, url: &url::Url, title: &str, at: jiff::Timestamp) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let url_s = url.as_str();
        let micros = ts_to_micros(at);
        conn.execute(
            "INSERT INTO visits (url, title, count, last_visit) VALUES (?1, ?2, 1, ?3)
             ON CONFLICT(url) DO UPDATE SET
               title = CASE WHEN ?2 <> '' THEN ?2 ELSE title END,
               count = count + 1,
               last_visit = ?3",
            params![url_s, title, micros],
        )?;
        Ok(())
    }

    fn set_title(&self, url: &url::Url, title: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let url_s = url.as_str();
        let micros = ts_to_micros(jiff::Timestamp::now());
        conn.execute(
            "INSERT INTO visits (url, title, count, last_visit) VALUES (?1, ?2, 0, ?3)
             ON CONFLICT(url) DO UPDATE SET title = ?2",
            params![url_s, title, micros],
        )?;
        Ok(())
    }

    fn search_history(&self, q: &str, limit: usize) -> Result<Vec<HistoryHit>> {
        let conn = self.conn.lock().unwrap();
        let q = q.trim();
        let rows: Vec<(String, String, i64, i64)> = if q.is_empty() {
            let mut stmt = conn.prepare("SELECT url, title, count, last_visit FROM visits")?;
            stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            let fts_q = build_fts(q);
            let mut stmt = conn.prepare(
                "SELECT v.url, v.title, v.count, v.last_visit
                 FROM visits v JOIN visits_fts f ON f.rowid = v.rowid
                 WHERE visits_fts MATCH ?1",
            )?;
            stmt.query_map(params![fts_q], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?
        };
        let now = jiff::Timestamp::now();
        let mut hits: Vec<HistoryHit> = rows
            .into_iter()
            .map(|(url, title, count, last_visit)| HistoryHit {
                url: url::Url::parse(&url).unwrap(),
                title,
                score: score_at(count, micros_to_ts(last_visit), now),
            })
            .collect();
        if hits.len() > limit {
            hits.select_nth_unstable_by(limit, |a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            hits.truncate(limit);
            hits.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        } else {
            hits.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            hits.truncate(limit);
        }
        Ok(hits)
    }

    fn toggle_bookmark(&self, url: &url::Url, title: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let url_s = url.as_str();
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM bookmarks WHERE url = ?1",
                params![url_s],
                |_| Ok(true),
            )
            .unwrap_or(false);
        if exists {
            conn.execute("DELETE FROM bookmarks WHERE url = ?1", params![url_s])?;
            Ok(false)
        } else {
            let created = ts_to_micros(jiff::Timestamp::now());
            conn.execute(
                "INSERT INTO bookmarks (url, title, created) VALUES (?1, ?2, ?3)",
                params![url_s, title, created],
            )?;
            Ok(true)
        }
    }

    fn is_bookmarked(&self, url: &url::Url) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let url_s = url.as_str();
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM bookmarks WHERE url = ?1",
                params![url_s],
                |_| Ok(true),
            )
            .unwrap_or(false);
        Ok(exists)
    }

    fn bookmarks(&self) -> Result<Vec<Bookmark>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT url, title, created FROM bookmarks ORDER BY rowid ASC")?;
        let collected = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(collected
            .into_iter()
            .map(|(url, title, created)| Bookmark {
                url: url::Url::parse(&url).unwrap(),
                title,
                created: micros_to_ts(created),
            })
            .collect())
    }

    fn search_bookmarks(&self, q: &str, limit: usize) -> Result<Vec<Bookmark>> {
        let conn = self.conn.lock().unwrap();
        let terms: Vec<String> = q.split_whitespace().map(|t| t.to_lowercase()).collect();
        let mut stmt = conn.prepare("SELECT url, title, created FROM bookmarks")?;
        let collected = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut out: Vec<Bookmark> = collected
            .into_iter()
            .filter(|(url, title, _)| {
                if terms.is_empty() {
                    true
                } else {
                    terms
                        .iter()
                        .all(|t| title.to_lowercase().contains(t) || url.to_lowercase().contains(t))
                }
            })
            .map(|(url, title, created)| Bookmark {
                url: url::Url::parse(&url).unwrap(),
                title,
                created: micros_to_ts(created),
            })
            .collect();
        out.truncate(limit);
        Ok(out)
    }

    fn save_session(&self, s: &SessionSnapshot) -> Result<()> {
        let forced = self.fail_next.swap(false, Ordering::SeqCst)
            || std::env::var("WREN_STORE_FAIL_SESSION_SAVE")
                .map(|v| !v.is_empty())
                .unwrap_or(false);
        if std::env::var("WREN_STORE_FAIL_SESSION_SAVE").is_ok() {
            unsafe {
                std::env::remove_var("WREN_STORE_FAIL_SESSION_SAVE");
            }
        }
        let json = serde_json::to_string(s)?;
        if forced {
            let mut conn = self.conn.lock().unwrap();
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO session (id, json) VALUES (1, ?1)
                 ON CONFLICT(id) DO UPDATE SET json = ?1",
                params![json],
            )?;
            // force failure: return error, tx drops -> rollback
            return Err(Error::Invalid("forced session save failure".to_string()));
        }
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO session (id, json) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET json = ?1",
            params![json],
        )?;
        tx.commit()?;
        Ok(())
    }

    fn load_session(&self) -> Result<Option<SessionSnapshot>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT json FROM session WHERE id = 1")?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let json: String = row.get(0)?;
            Ok(Some(serde_json::from_str(&json)?))
        } else {
            Ok(None)
        }
    }

    fn record_download(
        &self,
        url: &str,
        path: &str,
        bytes: u64,
        total: Option<u64>,
        state: crate::DownloadState,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO downloads (url, path, bytes, total, state) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                url,
                path,
                bytes as i64,
                total.map(|t| t as i64),
                state.as_str()
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    fn list_downloads(&self) -> Result<Vec<crate::DownloadRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, url, path, bytes, total, state FROM downloads ORDER BY id ASC")?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, Option<i64>>(4)?,
                    r.get::<_, String>(5)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows
            .into_iter()
            .filter_map(|(id, url, path, bytes, total, state)| {
                let state = crate::DownloadState::parse(&state)?;
                Some(crate::DownloadRow {
                    id,
                    url,
                    path,
                    bytes: bytes.max(0) as u64,
                    total: total.map(|t| t.max(0) as u64),
                    state,
                })
            })
            .collect())
    }

    fn update_download(
        &self,
        id: i64,
        bytes: u64,
        total: Option<u64>,
        state: crate::DownloadState,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE downloads SET bytes = ?2, total = ?3, state = ?4 WHERE id = ?1",
            params![id, bytes as i64, total.map(|t| t as i64), state.as_str()],
        )?;
        Ok(())
    }

    fn upsert_extension(
        &self,
        id: &str,
        version: &str,
        manifest_json: &str,
        path: &str,
        enabled: bool,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO extensions (id, version, manifest_json, path, enabled) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
               version = excluded.version,
               manifest_json = excluded.manifest_json,
               path = excluded.path,
               enabled = excluded.enabled",
            params![id, version, manifest_json, path, if enabled { 1i64 } else { 0i64 }],
        )?;
        Ok(())
    }

    fn list_extensions(&self) -> Result<Vec<crate::ExtensionRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, version, manifest_json, path, enabled FROM extensions ORDER BY id ASC",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows
            .into_iter()
            .map(
                |(id, version, manifest_json, path, enabled)| crate::ExtensionRow {
                    id,
                    version,
                    manifest_json,
                    path,
                    enabled: enabled != 0,
                },
            )
            .collect())
    }

    fn get_extension(&self, id: &str) -> Result<Option<crate::ExtensionRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, version, manifest_json, path, enabled FROM extensions WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(r) = rows.next()? {
            Ok(Some(crate::ExtensionRow {
                id: r.get(0)?,
                version: r.get(1)?,
                manifest_json: r.get(2)?,
                path: r.get(3)?,
                enabled: r.get::<_, i64>(4)? != 0,
            }))
        } else {
            Ok(None)
        }
    }

    fn set_extension_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE extensions SET enabled = ?2 WHERE id = ?1",
            params![id, if enabled { 1i64 } else { 0i64 }],
        )?;
        Ok(())
    }

    fn remove_extension(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM extensions WHERE id = ?1", params![id])?;
        Ok(())
    }
}
