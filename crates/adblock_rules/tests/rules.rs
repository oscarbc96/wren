use adblock_rules::{Blocker, RequestKind};
use url::Url;

#[test]
fn blocks_known_tracker() {
    let blocker = Blocker::from_lists(&["||doubleclick.net^"]);
    let url = Url::parse("https://doubleclick.net/tracker.js").unwrap();
    let source = Url::parse("https://example.org/").unwrap();
    assert!(blocker.should_block(&url, &source, RequestKind::Script));
}

#[test]
fn allows_exception_rule() {
    let blocker = Blocker::from_lists(&["||doubleclick.net^", "@@||doubleclick.net^"]);
    let url = Url::parse("https://doubleclick.net/tracker.js").unwrap();
    let source = Url::parse("https://example.org/").unwrap();
    assert!(!blocker.should_block(&url, &source, RequestKind::Script));
}

#[test]
fn document_requests_never_blocked() {
    let blocker = Blocker::from_lists(&["||example.org^"]);
    let url = Url::parse("https://example.org/page").unwrap();
    let source = Url::parse("https://example.org/").unwrap();
    assert!(!blocker.should_block(&url, &source, RequestKind::Document));
}

#[test]
fn third_party_option_respected() {
    let blocker = Blocker::from_lists(&["||tracker.example^$third-party"]);
    let url = Url::parse("https://tracker.example/x.js").unwrap();
    let third = Url::parse("https://example.org/").unwrap();
    let first = Url::parse("https://tracker.example/").unwrap();
    assert!(blocker.should_block(&url, &third, RequestKind::Script));
    assert!(!blocker.should_block(&url, &first, RequestKind::Script));
}
