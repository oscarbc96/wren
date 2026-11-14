use engine::{Engine, EngineEvent, FakeEngine, LoadState};
use tabs::TabStrip;
use url::Url;

fn url(s: &str) -> Url {
    Url::parse(s).unwrap()
}

#[test]
fn hang_after_timeout() {
    let mut e = FakeEngine::default();
    let mut strip = TabStrip::new(25);
    let _id = strip.open(&mut e, &url("https://a/"), true);
    let wv = strip.tabs()[0].webview;
    strip.apply(&mut e, &EngineEvent::Hung { id: wv });
    assert!(strip.tabs()[0].hung);
}

#[test]
fn responsive_clears_hung() {
    let mut e = FakeEngine::default();
    let mut strip = TabStrip::new(25);
    let _id = strip.open(&mut e, &url("https://a/"), true);
    let wv = strip.tabs()[0].webview;
    strip.apply(&mut e, &EngineEvent::Hung { id: wv });
    assert!(strip.tabs()[0].hung);
    strip.apply(&mut e, &EngineEvent::Responsive { id: wv });
    assert!(!strip.tabs()[0].hung);
}

#[test]
fn end_process_marks_only_that_tab() {
    let mut e = FakeEngine::default();
    let mut strip = TabStrip::new(25);
    let _a = strip.open(&mut e, &url("https://a/"), true);
    let _b = strip.open(&mut e, &url("https://b/"), false);
    let wv_a = strip.tabs()[0].webview;
    let wv_b = strip.tabs()[1].webview;
    e.kill(wv_a);
    strip.apply(
        &mut e,
        &EngineEvent::Crashed {
            id: wv_a,
            reason: "killed".into(),
        },
    );
    assert!(strip.tabs()[0].crashed.is_some());
    assert!(strip.tabs()[1].crashed.is_none());
    assert!(
        e.calls()
            .iter()
            .any(|c| matches!(c, engine::FakeCall::Kill { id } if *id == wv_a))
    );
    assert_eq!(wv_b, strip.tabs()[1].webview);
    // load clears hung, so also check hung unaffected
    strip.apply(
        &mut e,
        &EngineEvent::Load {
            id: wv_b,
            state: LoadState::Complete,
        },
    );
    assert!(strip.tabs()[1].crashed.is_none());
}
