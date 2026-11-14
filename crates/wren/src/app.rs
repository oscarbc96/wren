use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use gpui::{App, AppContext, KeyBinding, WindowOptions};
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
    // Do not create template leftovers in the profile dir.
    let _ = std::fs::remove_dir_all(paths.root.join("plugins"));
    let _ = std::fs::remove_dir_all(paths.root.join("workspaces"));

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
                    if urls_for_win.is_empty() {
                        BrowserWindow::new(
                            engine,
                            store_for_win.clone(),
                            settings_for_win.clone(),
                            cx,
                        )
                    } else {
                        BrowserWindow::with_urls(
                            engine,
                            store_for_win.clone(),
                            settings_for_win.clone(),
                            urls_for_win.clone(),
                            cx,
                        )
                    }
                })
            })
            .expect("open window");

        let quit_handle = handle;
        cx.on_action(move |_: &actions::Quit, cx| {
            let _ = quit_handle.update(cx, |view, _window, _cx| {
                view.shutdown();
            });
            cx.quit();
        });

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

fn register_actions(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-t", actions::New, None),
        KeyBinding::new("cmd-w", actions::Close, None),
        KeyBinding::new("cmd-shift-t", actions::Reopen, None),
        KeyBinding::new("cmd-l", actions::Focus, None),
        KeyBinding::new("cmd-r", actions::Reload, None),
        KeyBinding::new("cmd-[", actions::Back, None),
        KeyBinding::new("cmd-]", actions::Forward, None),
        KeyBinding::new("cmd-d", actions::Toggle, None),
        KeyBinding::new("cmd-shift-o", actions::OpenInChrome, None),
        KeyBinding::new("cmd-\\", actions::Fill, None),
        KeyBinding::new("cmd-1", actions::Select1, None),
        KeyBinding::new("cmd-2", actions::Select2, None),
        KeyBinding::new("cmd-3", actions::Select3, None),
        KeyBinding::new("cmd-4", actions::Select4, None),
        KeyBinding::new("cmd-5", actions::Select5, None),
        KeyBinding::new("cmd-6", actions::Select6, None),
        KeyBinding::new("cmd-7", actions::Select7, None),
        KeyBinding::new("cmd-8", actions::Select8, None),
        KeyBinding::new("cmd-9", actions::Select9, None),
        KeyBinding::new("cmd-shift-[", actions::Prev, None),
        KeyBinding::new("cmd-shift-]", actions::Next, None),
        KeyBinding::new("cmd-q", actions::Quit, None),
    ]);
}
