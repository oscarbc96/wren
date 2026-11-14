use std::sync::Arc;

use engine::FakeEngine;
use engine::fake::FakeCall;
use gpui::TestAppContext;
use settings::Settings;
use store::MemoryStore;

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
    &mut gpui::VisualTestContext,
) {
    cx.update(themes::init);
    cx.add_window_view(|_window, cx| BrowserWindow::new(engine, store, settings, cx))
}

#[gpui::test]
fn devtools_toggle_shows_indicator(cx: &mut TestAppContext) {
    let store = Arc::new(MemoryStore::default());
    let engine = FakeEngine::new();
    let (view, cxw) = setup_window(cx, engine, store, test_settings());

    cxw.update(|_window, cx| {
        view.update(cx, |w, _cx| {
            w.toggle_devtools();
            assert!(
                w.engine
                    .calls()
                    .iter()
                    .any(|c| matches!(c, FakeCall::SetDevtools { port: Some(7000) })),
                "expected set_devtools(Some(7000)), got {:?}",
                w.engine.calls()
            );
            assert!(
                w.toolbar
                    .devtools_indicator
                    .as_deref()
                    .unwrap_or("")
                    .contains("7000"),
                "expected toolbar devtools indicator, got {:?}",
                w.toolbar.devtools_indicator
            );

            w.toggle_devtools();
            assert!(
                w.engine
                    .calls()
                    .iter()
                    .any(|c| matches!(c, FakeCall::SetDevtools { port: None })),
                "expected set_devtools(None), got {:?}",
                w.engine.calls()
            );
            assert!(
                w.toolbar.devtools_indicator.is_none(),
                "expected indicator cleared, got {:?}",
                w.toolbar.devtools_indicator
            );
        });
    });
}
