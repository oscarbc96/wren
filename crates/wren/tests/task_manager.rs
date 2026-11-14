use engine::{FakeEngine, ProcessInfo};
use tabs::TabStrip;
use url::Url;
use wren::task_manager;

fn url(s: &str) -> Url {
    Url::parse(s).unwrap()
}

#[test]
fn task_manager_lists_tabs() {
    let mut e = FakeEngine::default();
    let mut strip = TabStrip::new(25);
    let _a = strip.open(&mut e, &url("https://a/"), true);
    let _b = strip.open(&mut e, &url("https://b/"), false);
    let wv_a = strip.tabs()[0].webview;
    let wv_b = strip.tabs()[1].webview;
    strip.tabs_mut()[0].title = "Alpha".into();
    strip.tabs_mut()[1].title = "Beta".into();
    let processes = vec![
        ProcessInfo {
            id: None,
            pid: 1,
            rss_bytes: 100,
            cpu_percent: 1.0,
        },
        ProcessInfo {
            id: Some(wv_a),
            pid: 2,
            rss_bytes: 300,
            cpu_percent: 2.0,
        },
        ProcessInfo {
            id: Some(wv_b),
            pid: 3,
            rss_bytes: 200,
            cpu_percent: 3.0,
        },
    ];
    let rows = task_manager::rows(processes, &strip);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].title, "Alpha");
    assert_eq!(rows[0].rss_bytes, 300);
    assert_eq!(rows[1].title, "Beta");
    assert_eq!(rows[1].rss_bytes, 200);
    assert_eq!(rows[2].title, "Browser");
    assert_eq!(rows[2].id, None);
}
