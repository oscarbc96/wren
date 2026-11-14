use parity::topsites::{parse_harness_report, parse_must_work_toml, usable};

#[test]
fn harness_json_schema() {
    let json = r#"{
  "entries": [
    {
      "url": "https://example.com",
      "final_url": "https://example.com/",
      "title": "Example Domain",
      "load_ms": 123,
      "console_errors": [],
      "screenshot_png": "harness-0.png",
      "crashed": false
    }
  ]
}"#;
    let report = parse_harness_report(json).unwrap();
    assert_eq!(report.entries.len(), 1);
    let e = &report.entries[0];
    assert_eq!(e.url, "https://example.com");
    assert_eq!(e.final_url, "https://example.com/");
    assert_eq!(e.title, "Example Domain");
    assert_eq!(e.load_ms, 123);
    assert!(e.console_errors.is_empty());
    assert_eq!(e.screenshot_png, "harness-0.png");
    assert!(!e.crashed);
}

#[test]
fn usable_rule() {
    // usable = loaded and not crashed and (ssim >= 0.6 or probe passed)
    assert!(usable(true, false, Some(0.6), None));
    assert!(usable(true, false, Some(0.9), None));
    assert!(!usable(true, false, Some(0.59), None));
    assert!(usable(true, false, Some(0.1), Some(true)));
    assert!(!usable(true, false, Some(0.1), Some(false)));
    assert!(!usable(false, false, Some(1.0), Some(true)));
    assert!(!usable(true, true, Some(1.0), Some(true)));
    assert!(!usable(true, false, None, None));
}

#[test]
fn must_work_toml_parses() {
    let toml = r#"
[[site]]
name = "Google Calendar"
url = "https://calendar.google.com/"
probe = "document.querySelector('[role=\"main\"]')"

[[site]]
name = "Slack"
url = "https://app.slack.com/client"
probe = "document.querySelector('.p-workspace')"
"#;
    let sites = parse_must_work_toml(toml).unwrap();
    assert_eq!(sites.len(), 2);
    assert_eq!(sites[0].name, "Google Calendar");
    assert_eq!(sites[0].url, "https://calendar.google.com/");
    assert!(sites[0].probe.contains("querySelector"));
    assert_eq!(sites[1].name, "Slack");
}
