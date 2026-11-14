use std::sync::{Arc, Mutex};

use engine::{EngineEvent, FakeEngine};
use gpui::TestAppContext;
use settings::Settings;
use store::MemoryStore;
use url::Url;

use wren::open_in_chrome::{Opener, RecordingOpener, rule_matches};
use wren::window::BrowserWindow;

fn test_settings() -> Settings {
    Settings::default()
}

fn setup_window_with_opener(
    cx: &mut TestAppContext,
    engine: FakeEngine,
    store: Arc<MemoryStore>,
    settings: Settings,
    opener: Arc<dyn Opener + Send + Sync>,
) -> (
    gpui::Entity<BrowserWindow<FakeEngine, MemoryStore>>,
    &mut gpui::VisualTestContext,
) {
    cx.update(themes::init);
    cx.add_window_view(|_window, cx| {
        BrowserWindow::with_urls_and_opener(
            engine,
            store,
            Arc::new(creds::MemoryCredStore::default()),
            opener,
            settings,
            Vec::new(),
            cx,
        )
    })
}

#[test]
fn rule_matches_subdomains() {
    let rules = vec!["example.com".to_string(), "*.meet.test".to_string()];
    assert!(rule_matches(
        &rules,
        &Url::parse("https://example.com/").unwrap()
    ));
    assert!(rule_matches(
        &rules,
        &Url::parse("https://sub.example.com/path").unwrap()
    ));
    assert!(rule_matches(
        &rules,
        &Url::parse("https://a.meet.test/room").unwrap()
    ));
    assert!(!rule_matches(
        &rules,
        &Url::parse("https://other.com/").unwrap()
    ));
    assert!(!rule_matches(
        &rules,
        &Url::parse("https://notexample.com/").unwrap()
    ));
}

#[gpui::test]
fn redirect_into_rule_host_opens_chrome(cx: &mut TestAppContext) {
    let store = Arc::new(MemoryStore::default());
    let engine = FakeEngine::new();
    let mut settings = test_settings();
    settings.sites.open_in_chrome = vec!["meet.google.com".to_string()];
    let opened: Arc<Mutex<Vec<Url>>> = Arc::new(Mutex::new(Vec::new()));
    let opener = Arc::new(RecordingOpener::new(opened.clone()));
    let (view, cxw) = setup_window_with_opener(cx, engine, store, settings, opener);

    let (webview, initial_url) = cxw.update(|_window, cx| {
        view.update(cx, |w, _| {
            let tab = w.active_tab().cloned().expect("active tab");
            (tab.webview, tab.url.clone())
        })
    });

    let redirect_url = Url::parse("https://meet.google.com/abc-defg-hij").unwrap();
    cxw.update(|_window, cx| {
        view.update(cx, |w, _| {
            w.engine.push(EngineEvent::NavigationDenied {
                id: webview,
                url: redirect_url.clone(),
            });
        });
    });
    cxw.update(|window, cx| {
        view.update(cx, |w, cx| {
            w.tick(window, cx);
        });
    });

    let calls = opened.lock().unwrap();
    assert_eq!(
        calls.as_slice(),
        &[redirect_url],
        "expected opener called with redirect URL"
    );
    drop(calls);

    cxw.update(|_window, cx| {
        view.update(cx, |w, _| {
            let tab = w.active_tab().expect("active tab");
            assert_eq!(tab.url, initial_url, "tab must stay on previous page");
        });
    });
}

#[gpui::test]
fn open_in_chrome_action_uses_active_url(cx: &mut TestAppContext) {
    let store = Arc::new(MemoryStore::default());
    let engine = FakeEngine::new();
    let settings = test_settings();
    let opened: Arc<Mutex<Vec<Url>>> = Arc::new(Mutex::new(Vec::new()));
    let opener = Arc::new(RecordingOpener::new(opened.clone()));
    let (view, cxw) = setup_window_with_opener(cx, engine, store, settings, opener);

    let url = Url::parse("https://example.org/page").unwrap();
    let webview = cxw.update(|_window, cx| view.update(cx, |w, _| w.active_webview().unwrap()));
    cxw.update(|_window, cx| {
        view.update(cx, |w, _| {
            w.engine.push(EngineEvent::UrlChanged {
                id: webview,
                url: url.clone(),
            });
        });
    });
    cxw.update(|window, cx| {
        view.update(cx, |w, cx| {
            w.tick(window, cx);
        });
    });
    cxw.update(|_window, cx| {
        view.update(cx, |w, _cx| {
            w.open_active_in_chrome();
        });
    });

    let calls = opened.lock().unwrap();
    assert_eq!(calls.as_slice(), &[url]);
}
