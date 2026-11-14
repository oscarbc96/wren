use extensions::match_pattern::MatchPattern;
use url::Url;

#[test]
fn match_pattern_chrome_cases() {
    let u = |s: &str| Url::parse(s).unwrap();
    let p = |s: &str| MatchPattern::parse(s).unwrap();

    let all = p("<all_urls>");
    assert!(all.matches(&u("https://a.com/")));
    assert!(all.matches(&u("file:///x")));
    assert!(!all.matches(&u("wren://settings")));

    let gg = p("*://*.google.com/*");
    assert!(gg.matches(&u("https://mail.google.com/x")));
    assert!(!gg.matches(&u("https://google.com.evil.io/")));

    let path = p("https://a.com/foo*");
    assert!(path.matches(&u("https://a.com/foobar")));
    assert!(!path.matches(&u("https://a.com/bar")));
}
