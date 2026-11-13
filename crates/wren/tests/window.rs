use std::sync::Arc;

use engine::{EngineEvent, FakeEngine, LoadState};
use gpui::{TestAppContext, VisualTestContext};
use settings::Settings;
use store::{MemoryStore, SessionSnapshot, SessionTab, Store};
use url::Url;

use wren::window::BrowserWindow;

fn test_settings() -> Settings {
    Settings::default()
}

fn setup_window(
    cx: &mut TestAppContext,
    engine: FakeEngine,
    store: Arc<MemoryStore>,
    settings: Settings,
) -> (
    gpui::Entity<BrowserWindow<FakeEngine, MemoryStore>>,
    &mut VisualTestContext,
) {
    cx.update(themes::init);
    cx.add_window_view(|_window, cx| BrowserWindow::new(engine, store, settings, cx))
}

fn setup_window_with_urls(
    cx: &mut TestAppContext,
    engine: FakeEngine,
    store: Arc<MemoryStore>,
    settings: Settings,
    urls: Vec<Url>,
) -> (
    gpui::Entity<BrowserWindow<FakeEngine, MemoryStore>>,
    &mut VisualTestContext,
) {
    cx.update(themes::init);
    cx.add_window_view(|_window, cx| BrowserWindow::with_urls(engine, store, settings, urls, cx))
}

#[gpui::test]
fn new_tab_focuses_omnibox(cx: &mut TestAppContext) {
    let store = Arc::new(MemoryStore::default());
    let engine = FakeEngine::new();
    let (view, cxw) = setup_window(cx, engine, store, test_settings());
    cxw.update(|_window, cx| {
        view.update(cx, |w, _cx| {
            w.new_tab(true);
            assert!(w.toolbar.focus_omnibox);
        });
    });
}

#[gpui::test]
fn enter_in_omnibox_navigates_or_searches(cx: &mut TestAppContext) {
    let store = Arc::new(MemoryStore::default());
    let engine = FakeEngine::new();
    let (view, cxw) = setup_window(cx, engine, store, test_settings());
    cxw.update(|_window, cx| {
        view.update(cx, |w, _| {
            w.navigate_input("example.com");
        });
    });
    cxw.update(|_window, cx| {
        view.update(cx, |w, _| {
            let calls = w.engine.calls();
            let has_nav = calls.iter().any(|c| match c {
                engine::fake::FakeCall::Load { url, .. } => url.as_str().contains("example.com"),
                engine::fake::FakeCall::Create { url } => url.as_str().contains("example.com"),
                _ => false,
            });
            assert!(has_nav, "expected navigation to example.com, got {calls:?}");
        });
    });
    cxw.update(|_window, cx| {
        view.update(cx, |w, _| {
            w.navigate_input("what is rust");
        });
    });
    cxw.update(|_window, cx| {
        view.update(cx, |w, _| {
            let calls = w.engine.calls();
            let has_search = calls.iter().any(|c| match c {
                engine::fake::FakeCall::Load { url, .. } => {
                    url.as_str().contains("duckduckgo.com") && url.as_str().contains("what")
                }
                _ => false,
            });
            assert!(has_search, "expected search navigation, got {calls:?}");
        });
    });
}

#[gpui::test]
fn closing_last_tab_opens_new_tab_url(cx: &mut TestAppContext) {
    let store = Arc::new(MemoryStore::default());
    let engine = FakeEngine::new();
    let (view, cxw) = setup_window(cx, engine, store, test_settings());
    cxw.update(|_window, cx| {
        view.update(cx, |w, _| {
            let id = w.strip.active().expect("active");
            w.close_tab(id);
            assert_eq!(w.strip.tabs().len(), 1);
            assert_eq!(w.strip.tabs()[0].url.as_str(), "about:blank");
        });
    });
}

#[gpui::test]
fn visits_are_recorded_on_load_complete(cx: &mut TestAppContext) {
    let store = Arc::new(MemoryStore::default());
    let engine = FakeEngine::new();
    let (view, cxw) = setup_window(cx, engine, store.clone(), test_settings());
    let webview = cxw.update(|_window, cx| view.update(cx, |w, _| w.strip.tabs()[0].webview));
    let url = Url::parse("https://example.org/").unwrap();
    cxw.update(|_window, cx| {
        view.update(cx, |w, _| {
            w.engine.push(EngineEvent::UrlChanged {
                id: webview,
                url: url.clone(),
            });
            w.engine.push(EngineEvent::TitleChanged {
                id: webview,
                title: "Example".into(),
            });
            w.engine.push(EngineEvent::Load {
                id: webview,
                state: LoadState::Complete,
            });
        });
    });
    cxw.update(|window, cx| {
        view.update(cx, |w, cx| {
            w.tick(window, cx);
        });
    });
    let hits = store.search_history("example", 10).unwrap();
    assert!(!hits.is_empty(), "expected recorded visit");
}

#[gpui::test]
fn star_toggles_bookmark(cx: &mut TestAppContext) {
    let store = Arc::new(MemoryStore::default());
    let engine = FakeEngine::new();
    let (view, cxw) = setup_window(cx, engine, store.clone(), test_settings());
    cxw.update(|_window, cx| {
        view.update(cx, |w, _| {
            let url = w.active_tab().unwrap().url.clone();
            assert!(!store.is_bookmarked(&url).unwrap());
            let added = store.toggle_bookmark(&url, "t").unwrap();
            assert!(added);
            let removed = store.toggle_bookmark(&url, "t").unwrap();
            assert!(!removed);
        });
    });
}

#[gpui::test]
fn crashed_tab_shows_reload(cx: &mut TestAppContext) {
    let store = Arc::new(MemoryStore::default());
    let engine = FakeEngine::new();
    let (view, cxw) = setup_window(cx, engine, store, test_settings());
    let (tab_id, webview) = cxw.update(|_window, cx| {
        view.update(cx, |w, _| {
            let t = w.strip.tabs()[0].clone();
            (t.id, t.webview)
        })
    });
    cxw.update(|_window, cx| {
        view.update(cx, |w, _| {
            w.engine.push(EngineEvent::Crashed {
                id: webview,
                reason: "boom".into(),
            });
        });
    });
    cxw.update(|window, cx| {
        view.update(cx, |w, cx| {
            w.tick(window, cx);
            assert_eq!(w.active_tab().unwrap().crashed.as_deref(), Some("boom"));
            w.strip.reload_crashed(&mut w.engine, tab_id);
            assert!(w.active_tab().unwrap().crashed.is_none());
        });
    });
}

#[gpui::test]
fn session_saved_when_changed(cx: &mut TestAppContext) {
    let store = Arc::new(MemoryStore::default());
    let engine = FakeEngine::new();
    let (view, cxw) = setup_window(cx, engine, store.clone(), test_settings());
    cxw.update(|window, cx| {
        view.update(cx, |w, cx| {
            w.new_tab(false);
            w.tick(window, cx);
        });
    });
    cxw.update(|_window, cx| {
        view.update(cx, |w, _| {
            w.save_session_now();
        });
    });
    let snap = store.load_session().unwrap().expect("session saved");
    assert!(snap.tabs.len() >= 2);
}

#[gpui::test]
fn startup_restores_session(cx: &mut TestAppContext) {
    let store = Arc::new(MemoryStore::default());
    let snap = SessionSnapshot {
        tabs: vec![
            SessionTab {
                url: Url::parse("https://a.test/").unwrap(),
                title: "A".into(),
            },
            SessionTab {
                url: Url::parse("https://b.test/").unwrap(),
                title: "B".into(),
            },
        ],
        active: Some(1),
    };
    store.save_session(&snap).unwrap();
    let engine = FakeEngine::new();
    let mut settings = test_settings();
    settings.startup.restore_session = true;
    let (view, cxw) = setup_window(cx, engine, store, settings);
    cxw.update(|_window, cx| {
        view.update(cx, |w, _| {
            assert_eq!(w.strip.tabs().len(), 2);
            assert_eq!(w.strip.active(), w.strip.tabs().get(1).map(|t| t.id));
        });
    });
}

#[gpui::test]
fn cli_urls_open_tabs(cx: &mut TestAppContext) {
    let store = Arc::new(MemoryStore::default());
    let engine = FakeEngine::new();
    let urls = vec![
        Url::parse("https://first.test/").unwrap(),
        Url::parse("https://second.test/").unwrap(),
    ];
    let (view, cxw) = setup_window_with_urls(cx, engine, store, test_settings(), urls);
    cxw.update(|_window, cx| {
        view.update(cx, |w, _| {
            assert_eq!(w.strip.tabs().len(), 2);
            assert_eq!(w.strip.active(), w.strip.tabs().first().map(|t| t.id));
            let creates: Vec<_> = w
                .engine
                .calls()
                .iter()
                .filter_map(|c| match c {
                    engine::fake::FakeCall::Create { url } => Some(url.clone()),
                    _ => None,
                })
                .collect();
            assert_eq!(creates.len(), 2);
            assert_eq!(creates[0].as_str(), "https://first.test/");
            assert_eq!(creates[1].as_str(), "https://second.test/");
        });
    });
}

#[gpui::test]
fn blank_pages_not_recorded(cx: &mut TestAppContext) {
    let store = Arc::new(MemoryStore::default());
    let engine = FakeEngine::new();
    let (view, cxw) = setup_window(cx, engine, store.clone(), test_settings());
    let webview = cxw.update(|_window, cx| view.update(cx, |w, _| w.strip.tabs()[0].webview));
    let url = Url::parse("about:blank").unwrap();
    cxw.update(|_window, cx| {
        view.update(cx, |w, _| {
            w.engine.push(EngineEvent::UrlChanged {
                id: webview,
                url: url.clone(),
            });
            w.engine.push(EngineEvent::TitleChanged {
                id: webview,
                title: "Blank".into(),
            });
            w.engine.push(EngineEvent::Load {
                id: webview,
                state: LoadState::Complete,
            });
        });
    });
    cxw.update(|window, cx| {
        view.update(cx, |w, cx| {
            w.tick(window, cx);
        });
    });
    let hits = store.search_history("about", 10).unwrap();
    assert!(hits.is_empty(), "about:blank must not be recorded");
}

#[gpui::test]
fn actions_dispatch(cx: &mut TestAppContext) {
    let store = Arc::new(MemoryStore::default());
    let engine = FakeEngine::new();
    let (view, cxw) = setup_window(cx, engine, store, test_settings());
    cxw.update(|window, cx| {
        view.update(cx, |w, _| {
            w.new_tab(false);
        });
        window.dispatch_action(Box::new(wren::actions::Next), cx);
        window.dispatch_action(Box::new(wren::actions::Close), cx);
        window.dispatch_action(Box::new(wren::actions::New), cx);
        assert!(!view.read(cx).strip.tabs().is_empty());
    });
}

#[gpui::test]
fn content_forwards_input(cx: &mut TestAppContext) {
    let store = Arc::new(MemoryStore::default());
    let engine = FakeEngine::new();
    let (view, cxw) = setup_window(cx, engine, store, test_settings());
    cxw.update(|window, cx| {
        view.update(cx, |w, cx| {
            w.tick(window, cx);
        });
    });
    cxw.simulate_mouse_down(
        gpui::Point::new(gpui::px(50.), gpui::px(120.)),
        gpui::MouseButton::Left,
        gpui::Modifiers::default(),
    );
    cxw.simulate_mouse_up(
        gpui::Point::new(gpui::px(50.), gpui::px(120.)),
        gpui::MouseButton::Left,
        gpui::Modifiers::default(),
    );
    cxw.update(|_window, cx| {
        view.update(cx, |w, _| {
            let has_input = w.engine.calls().iter().any(|c| {
                matches!(
                    c,
                    engine::fake::FakeCall::Input(_, engine::EngineInput::MouseDown { .. })
                )
            });
            assert!(
                has_input,
                "expected mouse down forwarded, got {:?}",
                w.engine.calls()
            );
        });
    });
}
