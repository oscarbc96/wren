use engine::{ConsoleLevel, Engine, EngineEvent, FakeEngine, WebViewId};
use std::cell::RefCell;
use std::rc::Rc;
use url::Url;

#[test]
#[allow(clippy::type_complexity)]
fn fake_evaluate_replies() {
    let mut e = FakeEngine::new();
    let id = e.create(&Url::parse("https://example.com/").unwrap());
    e.set_eval_result(id, Ok(serde_json::json!(2)));

    let got: Rc<RefCell<Option<Result<serde_json::Value, String>>>> = Rc::new(RefCell::new(None));
    let got2 = got.clone();
    e.evaluate(
        id,
        "1+1",
        Box::new(move |r| {
            *got2.borrow_mut() = Some(r);
        }),
    );
    assert_eq!(*got.borrow(), Some(Ok(serde_json::json!(2))));
}

#[test]
#[allow(clippy::type_complexity)]
fn fake_screenshot_is_png() {
    let mut e = FakeEngine::new();
    let id = e.create(&Url::parse("https://example.com/").unwrap());

    let got: Rc<RefCell<Option<Result<Vec<u8>, String>>>> = Rc::new(RefCell::new(None));
    let got2 = got.clone();
    e.screenshot(
        id,
        Box::new(move |r| {
            *got2.borrow_mut() = Some(r);
        }),
    );
    let bytes = got.borrow_mut().take().unwrap().expect("screenshot ok");
    assert!(bytes.starts_with(b"\x89PNG"), "expected png header");
}

#[test]
fn fake_console_event_drains() {
    let mut e = FakeEngine::new();
    let id = WebViewId(7);
    e.push_console(id, ConsoleLevel::Log, "hello".to_string());
    let evs = e.drain_events();
    assert_eq!(
        evs,
        vec![EngineEvent::Console {
            id,
            level: ConsoleLevel::Log,
            message: "hello".to_string(),
        }]
    );
}
