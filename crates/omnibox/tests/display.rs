use omnibox::display_host;
use url::Url;

#[test]
fn display_host_mixed_script_is_punycode() {
    // Cyrillic а (U+0430) in "аpple.com"
    // construct with unicode host? Url normalizes to punycode already
    let host = display_host(&Url::parse("https://аpple.com/").unwrap());
    assert_eq!(host, "xn--pple-43d.com");
}

#[test]
fn display_host_single_script_unicode() {
    let host = display_host(&Url::parse("https://münchen.de/").unwrap());
    assert_eq!(host, "münchen.de");
}
