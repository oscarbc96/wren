use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use gpui::{App, AppContext, WindowOptions};
use gpui_platform::application;
use raw_window_handle::HasWindowHandle;
use settings::{ConfigPaths, SettingsStore};
use store::SqliteStore;
use url::Url;

use crate::actions;
use crate::cli::Args;
use crate::window::BrowserWindow;

pub fn run() {
    let args = crate::cli::parse();
    run_with_args(args);
}

pub fn run_with_args(args: Args) {
    let (config_dir, data_dir) = resolve_dirs(&args);
    let paths = ConfigPaths::at(config_dir.clone());
    let _ = paths.ensure_exists();

    let store_path = data_dir.join("wren.sqlite");
    let store = Arc::new(SqliteStore::open(&store_path).expect("open store"));
    let settings_store = SettingsStore::load(paths.clone());
    let settings = settings_store.current().clone();

    let urls: Vec<Url> = args
        .urls
        .iter()
        .filter_map(|u| {
            Url::parse(u).ok().or_else(|| {
                let with_scheme = format!("https://{u}");
                Url::parse(&with_scheme).ok()
            })
        })
        .collect();

    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .ok();

    let app = application();
    app.run(move |cx: &mut App| {
        themes::init(cx);
        register_actions(cx);

        let store_for_win = store.clone();
        let settings_for_win = settings.clone();
        let urls_for_win = urls.clone();
        let handle = cx
            .open_window(WindowOptions::default(), move |window, cx| {
                let raw_wh = <gpui::Window as HasWindowHandle>::window_handle(window)
                    .expect("raw window handle");
                let waker: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
                let engine = engine::ServoEngine::new(raw_wh, waker).expect("servo engine");
                cx.new(|cx| {
                    let mut bw = BrowserWindow::new(
                        engine,
                        store_for_win.clone(),
                        settings_for_win.clone(),
                        cx,
                    );
                    if !urls_for_win.is_empty() {
                        if let Some(first) = urls_for_win.first() {
                            bw.open_url(first.clone());
                        }
                        for u in urls_for_win.iter().skip(1) {
                            bw.strip.open(&mut bw.engine, u, false);
                        }
                    }
                    bw
                })
            })
            .expect("open window");

        cx.spawn(async move |cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(8))
                    .await;
                let ok = handle
                    .update(cx, |view, window, cx| {
                        view.tick(window, cx);
                    })
                    .is_ok();
                if !ok {
                    break;
                }
            }
        })
        .detach();
    });
}

fn resolve_dirs(args: &Args) -> (PathBuf, PathBuf) {
    if let Some(p) = &args.profile_dir {
        let pb = PathBuf::from(p);
        (pb.clone(), pb)
    } else {
        let config = std::env::var("WREN_CONFIG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::config_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("wren")
            });
        let data = std::env::var("WREN_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::data_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("wren")
            });
        (config, data)
    }
}

fn register_actions(_cx: &mut App) {
    // Actions are registered via keymap bindings and listeners on views.
    // GPUI-ce binds actions through keymaps; ensure action types are linked.
    let _ = std::any::TypeId::of::<actions::New>();
}
