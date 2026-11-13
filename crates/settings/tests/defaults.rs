use settings::Settings;

#[test]
fn defaults_match_spec() {
    let s = Settings::default();
    assert_eq!(s.search.url, "https://duckduckgo.com/?q={query}");
    assert!(s.startup.restore_session);
    assert_eq!(s.tabs.new_tab_url, "about:blank");
    assert_eq!(s.tabs.closed_stack, 25);
}
