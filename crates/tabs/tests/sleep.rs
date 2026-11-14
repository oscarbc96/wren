use engine::FakeEngine;
use jiff::Timestamp;
use tabs::{TabId, TabState, TabStrip};
use url::Url;

fn url(s: &str) -> Url {
    Url::parse(s).unwrap()
}

fn ts(secs: i64) -> Timestamp {
    Timestamp::from_second(secs).unwrap()
}

#[test]
fn sleep_closes_webview_keeps_url() {
    let mut e = FakeEngine::default();
    let mut strip = TabStrip::new(25);
    let id = strip.open(&mut e, &url("https://a/"), true);
    let before = strip.tabs()[0].clone();
    strip.sleep(&mut e, id);
    let tab = strip.tabs().iter().find(|t| t.id == id).unwrap();
    assert_eq!(tab.state, TabState::Slept);
    assert_eq!(tab.url, before.url);
    assert_eq!(tab.title, before.title);
    // webview closed
    assert!(
        e.calls()
            .iter()
            .any(|c| matches!(c, engine::FakeCall::Close { id } if *id == before.webview))
    );
}

#[test]
fn woken_tab_restores_url() {
    let mut e = FakeEngine::default();
    let mut strip = TabStrip::new(25);
    let id = strip.open(&mut e, &url("https://a/"), true);
    let old_url = strip.tabs()[0].url.clone();
    strip.sleep(&mut e, id);
    let create_count = e
        .calls()
        .iter()
        .filter(|c| matches!(c, engine::FakeCall::Create { .. }))
        .count();
    strip.wake(&mut e, id);
    let tab = strip.tabs().iter().find(|t| t.id == id).unwrap();
    assert_eq!(tab.state, TabState::Active);
    assert_eq!(tab.url, old_url);
    let create_count_after = e
        .calls()
        .iter()
        .filter(|c| matches!(c, engine::FakeCall::Create { .. }))
        .count();
    assert_eq!(create_count_after, create_count + 1);
}

#[test]
fn sleep_skips_media_and_pinned() {
    let mut e = FakeEngine::default();
    let mut strip = TabStrip::new(25);
    let a = strip.open(&mut e, &url("https://a/"), true);
    let b = strip.open(&mut e, &url("https://b/"), false);
    let c = strip.open(&mut e, &url("https://c/"), false);
    let d = strip.open(&mut e, &url("https://d/"), false);
    // make b pinned, c audible, a is active
    {
        let tab = strip.tabs_mut().iter_mut().find(|t| t.id == b).unwrap();
        tab.pinned = true;
    }
    {
        let tab = strip.tabs_mut().iter_mut().find(|t| t.id == c).unwrap();
        tab.audible = true;
    }
    // set last_active old for all
    let old = ts(0);
    for t in strip.tabs_mut().iter_mut() {
        t.last_active = old;
    }
    let now = ts(3600);
    let after = jiff::SignedDuration::from_secs(60 * 30);
    let candidates = strip.sleep_candidates(now, after);
    assert_eq!(candidates, vec![d]);
    let _ = a;
}

#[test]
fn candidates_oldest_first() {
    let mut e = FakeEngine::default();
    let mut strip = TabStrip::new(25);
    let active = strip.open(&mut e, &url("https://active/"), true);
    let b = strip.open(&mut e, &url("https://b/"), false);
    let c = strip.open(&mut e, &url("https://c/"), false);
    let d = strip.open(&mut e, &url("https://d/"), false);
    for t in strip.tabs_mut().iter_mut() {
        if t.id == active {
            t.last_active = ts(3000);
        } else if t.id == b {
            t.last_active = ts(100);
        } else if t.id == c {
            t.last_active = ts(50);
        } else if t.id == d {
            t.last_active = ts(200);
        }
    }
    let now = ts(3600);
    let after = jiff::SignedDuration::from_secs(60);
    let candidates = strip.sleep_candidates(now, after);
    assert_eq!(candidates, vec![c, b, d]);
}

#[test]
fn budget_sleeps_until_under() {
    let mut e = FakeEngine::default();
    let mut strip = TabStrip::new(25);
    let active = strip.open(&mut e, &url("https://active/"), true);
    let b = strip.open(&mut e, &url("https://b/"), false);
    let c = strip.open(&mut e, &url("https://c/"), false);
    let d = strip.open(&mut e, &url("https://d/"), false);
    for t in strip.tabs_mut().iter_mut() {
        if t.id == active {
            t.last_active = ts(3000);
        } else if t.id == b {
            t.last_active = ts(100);
        } else if t.id == c {
            t.last_active = ts(50);
        } else if t.id == d {
            t.last_active = ts(200);
        }
    }
    let rss = |id: TabId| -> u64 {
        if id == active {
            300
        } else if id == b {
            200
        } else if id == c {
            150
        } else if id == d {
            250
        } else {
            0
        }
    };
    strip.sleep_until_under_budget(&mut e, 500, &rss);
    let states: Vec<_> = strip
        .tabs()
        .iter()
        .map(|t| (t.id, t.state.clone()))
        .collect();
    assert!(
        states
            .iter()
            .any(|(id, s)| *id == c && *s == TabState::Slept)
    );
    assert!(
        states
            .iter()
            .any(|(id, s)| *id == b && *s == TabState::Slept)
    );
    assert!(
        states
            .iter()
            .any(|(id, s)| *id == d && *s != TabState::Slept
                || *id == active && *s != TabState::Slept)
    );
    let total: u64 = strip
        .tabs()
        .iter()
        .filter(|t| t.state != TabState::Slept)
        .map(|t| rss(t.id))
        .sum();
    assert!(total <= 500);
}
