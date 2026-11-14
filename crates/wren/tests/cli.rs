use clap::Parser;
use wren::cli::Args;

#[test]
fn parses_screenshot_flags() {
    let args = Args::parse_from([
        "wren",
        "--screenshot",
        "out.png",
        "--screenshot-delay-ms",
        "1500",
        "https://example.com",
    ]);
    assert_eq!(args.screenshot.as_deref(), Some("out.png"));
    assert_eq!(args.screenshot_delay_ms, 1500);
    assert_eq!(args.urls, vec!["https://example.com".to_string()]);
}

#[test]
fn screenshot_defaults_delay() {
    let args = Args::parse_from(["wren", "--screenshot", "out.png"]);
    assert_eq!(args.screenshot_delay_ms, 3000);
}
