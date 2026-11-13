use engine::{Engine, EngineEvent, FakeCall, FakeEngine, LoadState, WebViewId};
use url::Url;

#[test]
fn create_returns_unique_ids() {
    let mut e = FakeEngine::default();
    let a = e.create(&Url::parse("https://a/").unwrap());
    let b = e.create(&Url::parse("https://b/").unwrap());
    let c = e.create(&Url::parse("https://c/").unwrap());
    assert_ne!(a, b);
    assert_ne!(a, c);
    assert_ne!(b, c);
}

#[test]
fn events_drain_in_order() {
    let mut e = FakeEngine::default();
    let id = WebViewId(1);
    let ev1 = EngineEvent::Load {
        id,
        state: LoadState::Started,
    };
    let ev2 = EngineEvent::Load {
        id,
        state: LoadState::Complete,
    };
    e.push(ev1.clone());
    e.push(ev2.clone());
    let drained = e.drain_events();
    assert_eq!(drained, vec![ev1, ev2]);
    assert!(e.drain_events().is_empty());
}

#[test]
fn calls_are_recorded() {
    let mut e = FakeEngine::default();
    let id = e.create(&Url::parse("https://x/").unwrap());
    e.load(id, &Url::parse("https://y/").unwrap());
    e.back(id);
    e.reload(id);
    let calls = e.calls();
    assert!(calls.iter().any(|c| matches!(c, FakeCall::Load { .. })));
    assert!(calls.iter().any(|c| matches!(c, FakeCall::Back { .. })));
    assert!(calls.iter().any(|c| matches!(c, FakeCall::Reload { .. })));
}
