use omnibox::{SuggestionKind, suggest};
use store::{MemoryStore, Store};
use url::Url;

#[test]
fn suggest_bookmarks_first_then_history() {
    let store = MemoryStore::default();
    let now = jiff::Timestamp::now();
    store
        .record_visit(
            &Url::parse("https://github.com/rust").unwrap(),
            "Rust GitHub",
            now,
        )
        .unwrap();
    store
        .toggle_bookmark(&Url::parse("https://gitlab.com/").unwrap(), "GitLab")
        .unwrap();

    let s = suggest(&store, "git", 8);
    assert!(!s.is_empty());
    assert_eq!(s[0].kind, SuggestionKind::Bookmark);
    // last is search
    assert_eq!(s.last().unwrap().kind, SuggestionKind::Search);
}

#[test]
#[ignore]
fn suggest_100k_under_20ms() {
    use std::time::Instant;
    let store = MemoryStore::default();
    let base = jiff::Timestamp::now();
    for i in 0..100_000 {
        let url = Url::parse(&format!("https://example.com/{}/git", i)).unwrap();
        store
            .record_visit(&url, &format!("git repo {}", i), base)
            .unwrap();
    }
    let mut times = Vec::new();
    for _ in 0..20 {
        let t = Instant::now();
        let _ = suggest(&store, "git", 8);
        times.push(t.elapsed());
    }
    times.sort();
    let median = times[times.len() / 2];
    assert!(median.as_millis() < 20, "median {:?} >= 20ms", median);
}
