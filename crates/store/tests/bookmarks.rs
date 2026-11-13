use store::{MemoryStore, SqliteStore, Store};
use tempfile::TempDir;
use url::Url;

fn sqlite() -> (SqliteStore, TempDir) {
    let dir = TempDir::new().unwrap();
    let s = SqliteStore::open(&dir.path().join("t.db")).unwrap();
    (s, dir)
}

fn run_toggle(store: &dyn Store) {
    let url = Url::parse("https://example.com/").unwrap();
    assert!(store.toggle_bookmark(&url, "Example").unwrap());
    assert!(store.is_bookmarked(&url).unwrap());
    assert!(!store.toggle_bookmark(&url, "Example").unwrap());
    assert!(!store.is_bookmarked(&url).unwrap());
}

fn run_search(store: &dyn Store) {
    let u1 = Url::parse("https://a.com/").unwrap();
    let u2 = Url::parse("https://b.com/").unwrap();
    store.toggle_bookmark(&u1, "Rust lang").unwrap();
    store.toggle_bookmark(&u2, "Other").unwrap();
    let hits = store.search_bookmarks("rust", 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].url, u1);
    let all = store.bookmarks().unwrap();
    assert_eq!(all.len(), 2);
}

#[test]
fn toggle_bookmark_adds_then_removes() {
    let (s, _d) = sqlite();
    run_toggle(&s);
    run_toggle(&MemoryStore::default());
}

#[test]
fn search_bookmarks_works() {
    let (s, _d) = sqlite();
    run_search(&s);
    run_search(&MemoryStore::default());
}
