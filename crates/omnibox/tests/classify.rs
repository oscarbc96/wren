use omnibox::{Input, classify, search_url};
use url::Url;

#[test]
fn classify_table() {
    let cases: &[(&str, Input)] = &[
        (
            "https://a.com/x",
            Input::Navigate(Url::parse("https://a.com/x").unwrap()),
        ),
        (
            "example.com",
            Input::Navigate(Url::parse("https://example.com/").unwrap()),
        ),
        (
            "localhost:3000",
            Input::Navigate(Url::parse("http://localhost:3000/").unwrap()),
        ),
        (
            "192.168.1.1",
            Input::Navigate(Url::parse("http://192.168.1.1/").unwrap()),
        ),
        (
            "[::1]:8080",
            Input::Navigate(Url::parse("http://[::1]:8080/").unwrap()),
        ),
        (
            "foo.local",
            Input::Navigate(Url::parse("http://foo.local/").unwrap()),
        ),
        (
            "about:blank",
            Input::Navigate(Url::parse("about:blank").unwrap()),
        ),
        (
            "file:///tmp/a.html",
            Input::Navigate(Url::parse("file:///tmp/a.html").unwrap()),
        ),
        ("what is rust", Input::Search("what is rust".to_string())),
        ("rust.lang", Input::Search("rust.lang".to_string())),
        (
            "  example.com  ",
            Input::Navigate(Url::parse("https://example.com/").unwrap()),
        ),
    ];
    for (input, expected) in cases {
        assert_eq!(classify(input), *expected, "input={input:?}");
    }
}

#[test]
fn search_url_encodes() {
    let u = search_url("https://duckduckgo.com/?q={query}", "rust lang");
    assert_eq!(u.as_str(), "https://duckduckgo.com/?q=rust%20lang");
}
