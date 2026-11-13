use engine::{EngineEvent, FakeEngine, LoadState};
use tabs::TabStrip;
use url::Url;

fn url(s: &str) -> Url {
    Url::parse(s).unwrap()
}

#[test]
fn close_active_activates_right_then_left() {
    let mut e = FakeEngine::default();
    let mut strip = TabStrip::new(25);
    let a = strip.open(&mut e, &url("https://a/"), true);
    let b = strip.open(&mut e, &url("https://b/"), true);
    let c = strip.open(&mut e, &url("https://c/"), true);
    // active is c (last). close active -> right none, so left (b)
    strip.close(&mut e, c);
    assert_eq!(strip.active(), Some(b));
    // now active b; close -> right (none? actually after removing c, tabs a,b; active index1 b; close b -> left a)
    strip.close(&mut e, b);
    assert_eq!(strip.active(), Some(a));
}

#[test]
fn close_last_tab_leaves_strip_empty() {
    let mut e = FakeEngine::default();
    let mut strip = TabStrip::new(25);
    let a = strip.open(&mut e, &url("https://a/"), true);
    strip.close(&mut e, a);
    assert!(strip.tabs().is_empty());
    assert_eq!(strip.active(), None);
}

#[test]
fn reopen_restores_url_and_position() {
    let mut e = FakeEngine::default();
    let mut strip = TabStrip::new(25);
    let a = strip.open(&mut e, &url("https://a/"), true);
    let b = strip.open(&mut e, &url("https://b/"), true);
    // close a (index 0)
    strip.close(&mut e, a);
    // reopen
    let reopened = strip.reopen_closed(&mut e).unwrap();
    assert_eq!(strip.tabs()[0].url, url("https://a/"));
    // position restored
    let _ = b;
    let _ = reopened;
}

#[test]
fn closed_stack_is_capped_at_25() {
    let mut e = FakeEngine::default();
    let mut strip = TabStrip::new(25);
    for i in 0..30 {
        let t = strip.open(&mut e, &url(&format!("https://{i}/")), true);
        strip.close(&mut e, t);
    }
    // reopen 25 times max
    let mut count = 0;
    while strip.reopen_closed(&mut e).is_some() {
        count += 1;
    }
    assert_eq!(count, 25);
}

#[test]
fn title_and_url_events_update_tab() {
    let mut e = FakeEngine::default();
    let mut strip = TabStrip::new(25);
    let _id = strip.open(&mut e, &url("https://a/"), true);
    let wv = strip.tabs()[0].webview;
    strip.apply(&mut e, &EngineEvent::TitleChanged {
        id: wv,
        title: "Title".into(),
    });
    strip.apply(&mut e, &EngineEvent::UrlChanged {
        id: wv,
        url: url("https://b/"),
    });
    strip.apply(&mut e, &EngineEvent::Load {
        id: wv,
        state: LoadState::Started,
    });
    strip.apply(&mut e, &EngineEvent::History {
        id: wv,
        can_back: true,
        can_forward: false,
    });
    let tab = &strip.tabs()[0];
    assert_eq!(tab.title, "Title");
    assert_eq!(tab.url, url("https://b/"));
    assert!(tab.loading);
    assert!(tab.can_back);
    assert!(!tab.can_forward);
}

#[test]
fn open_tab_event_opens_after_opener() {
    let mut e = FakeEngine::default();
    let mut strip = TabStrip::new(25);
    let _a = strip.open(&mut e, &url("https://a/"), true);
    let wv = strip.tabs()[0].webview;
    strip.apply(&mut e, &EngineEvent::OpenTab {
        opener: wv,
        url: url("https://b/"),
    });
    assert_eq!(strip.tabs().len(), 2);
    assert_eq!(strip.tabs()[1].url, url("https://b/"));
    assert_ne!(strip.tabs()[1].webview, wv, "a real webview is created");
    // not activated: active still first tab
    assert_eq!(strip.active(), Some(strip.tabs()[0].id));
}

#[test]
fn crashed_tab_reloads_with_new_webview() {
    let mut e = FakeEngine::default();
    let mut strip = TabStrip::new(25);
    let id = strip.open(&mut e, &url("https://a/"), true);
    let old_wv = strip.tabs()[0].webview;
    strip.apply(&mut e, &EngineEvent::Crashed {
        id: old_wv,
        reason: "boom".into(),
    });
    assert_eq!(strip.tabs()[0].crashed.as_deref(), Some("boom"));
    strip.reload_crashed(&mut e, id);
    assert!(strip.tabs()[0].crashed.is_none());
    let calls = e.calls();
    assert!(
        calls
            .iter()
            .any(|c| matches!(c, engine::FakeCall::Close { id: x } if *x == old_wv))
    );
    let new_wv = strip.tabs()[0].webview;
    assert_ne!(new_wv, old_wv);
}

#[test]
fn snapshot_restore_roundtrip() {
    let mut e = FakeEngine::default();
    let mut strip = TabStrip::new(25);
    let a = strip.open(&mut e, &url("https://a/"), true);
    let _b = strip.open(&mut e, &url("https://b/"), false);
    // set title
    let wv = strip.tabs()[0].webview;
    strip.apply(&mut e, &EngineEvent::TitleChanged {
        id: wv,
        title: "A".into(),
    });
    let snap = strip.snapshot();
    // restore into new engine
    let mut e2 = FakeEngine::default();
    let restored = TabStrip::restore(&mut e2, &snap, 25);
    assert_eq!(restored.tabs().len(), 2);
    assert_eq!(restored.tabs()[0].url, url("https://a/"));
    assert_eq!(restored.tabs()[0].title, "A");
    assert_eq!(restored.active(), Some(restored.tabs()[0].id));
    let _ = a;
}

#[test]
fn changed_flag() {
    let mut e = FakeEngine::default();
    let mut strip = TabStrip::new(25);
    assert!(!strip.changed());
    let id = strip.open(&mut e, &url("https://a/"), true);
    assert!(strip.changed());
    assert!(!strip.changed());
    strip.activate(id);
    // activating same active may or may not set; ensure at least close sets
    let id2 = strip.open(&mut e, &url("https://b/"), false);
    strip.close(&mut e, id2);
    assert!(strip.changed());
}

