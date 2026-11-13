use store::{MemoryStore, SqliteStore, Store};
use tempfile::TempDir;
use url::Url;

fn sqlite() -> (SqliteStore, TempDir) {
    let dir = TempDir::new().unwrap();
    let s = SqliteStore::open(&dir.path().join("t.db")).unwrap();
    (s, dir)
}

fn run_visit_counts(store: &dyn Store) {
    let now = jiff::Timestamp::now();
    let a = Url::parse("https://a.example/x").unwrap();
    let b = Url::parse("https://b.example/y").unwrap();
    let c = Url::parse("https://c.example/z").unwrap();
    let old = now.checked_sub(jiff::Span::new().hours(30 * 24)).unwrap();
    let older = now.checked_sub(jiff::Span::new().hours(60 * 24)).unwrap();
    for _ in 0..3 {
        store.record_visit(&a, "Alpha", old).unwrap();
    }
    store.record_visit(&b, "Beta", now).unwrap();
    store.record_visit(&c, "Gamma", older).unwrap();

    let hits = store.search_history("", 10).unwrap();
    assert_eq!(hits[0].url, a);
    assert_eq!(hits[1].url, b);
    assert_eq!(hits[2].url, c);
}

fn run_fts(store: &dyn Store) {
    let now = jiff::Timestamp::now();
    let url = Url::parse("https://example.com/path").unwrap();
    store.record_visit(&url, "Rust language", now).unwrap();
    let hits = store.search_history("rust", 10).unwrap();
    assert_eq!(hits.len(), 1);
    let hits = store.search_history("example.com", 10).unwrap();
    assert_eq!(hits.len(), 1);
    let hits = store.search_history("nothing", 10).unwrap();
    assert!(hits.is_empty());
}

#[test]
fn visit_counts_and_recency_rank() {
    let (s, _d) = sqlite();
    run_visit_counts(&s);
    run_visit_counts(&MemoryStore::default());
}

#[test]
fn fts_matches_title_and_url() {
    let (s, _d) = sqlite();
    run_fts(&s);
    run_fts(&MemoryStore::default());
}

#[test]
fn migrations_apply_on_fresh_db() {
    let (s, _d) = sqlite();
    // fresh db usable
    let url = Url::parse("https://x.test/").unwrap();
    s.record_visit(&url, "t", jiff::Timestamp::now()).unwrap();
    let hits = s.search_history("", 10).unwrap();
    assert_eq!(hits.len(), 1);
    // memory too
    let m = MemoryStore::default();
    m.record_visit(&url, "t", jiff::Timestamp::now()).unwrap();
    assert_eq!(m.search_history("", 10).unwrap().len(), 1);
}
