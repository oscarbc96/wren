use engine::api::Engine;
use engine::fake::{FakeCall, FakeEngine};

#[test]
fn devtools_toggle_records_calls() {
    let mut e = FakeEngine::new();
    e.set_devtools(Some(6000)).unwrap();
    e.set_devtools(None).unwrap();
    assert_eq!(
        e.calls(),
        &[
            FakeCall::SetDevtools { port: Some(6000) },
            FakeCall::SetDevtools { port: None },
        ]
    );
}
