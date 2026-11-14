use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use gpui::{App, AppContext, KeyBinding, TitlebarOptions, WindowOptions, point, px};
use gpui_platform::application;
use raw_window_handle::HasWindowHandle;
use settings::{ConfigPaths, SettingsStore};
use store::SqliteStore;
use url::Url;

use engine::Engine;

use crate::actions;
use crate::cdp_bridge::CdpBridge;
use crate::cli::Args;
use crate::screenshot;
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
    let devtools_port = match args.devtools {
        Some(0) => Some(settings.devtools.port),
        Some(port) => Some(port),
        None => settings.devtools.enabled.then_some(settings.devtools.port),
    };
    let cdp_port = args
        .remote_debugging_port
        .or_else(|| settings.cdp.enabled.then_some(settings.cdp.port));
    let engine_options = engine::EngineOptions {
        devtools_port,
        js_gc_profile: settings.memory.js_gc_profile.clone(),
        js_jit: settings.memory.js_jit,
    };
    let settings_file = paths.settings.clone();

    let adblock_dir = data_dir.join("adblock");
    let mut cached_lists: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&adblock_dir) {
        for entry in entries.flatten() {
            if entry.path().extension().and_then(|e| e.to_str()) == Some("txt")
                && let Ok(text) = std::fs::read_to_string(entry.path())
            {
                cached_lists.push(text);
            }
        }
    }
    let blocker = if settings.adblock.enabled {
        let refs: Vec<&str> = cached_lists.iter().map(|s| s.as_str()).collect();
        Some(std::rc::Rc::new(adblock_rules::Blocker::from_lists(&refs)))
    } else {
        None
    };
    if settings.adblock.enabled {
        let dir = adblock_dir.clone();
        std::thread::spawn(move || {
            let _ = adblock_rules::update_lists(&dir);
        });
    }

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

    let screenshot_out = args.screenshot.clone().map(PathBuf::from);
    let harness_out = args.harness.clone().map(PathBuf::from);
    let screenshot_delay_ms = args.screenshot_delay_ms;
    let memory_report_out = args.memory_report.clone().map(PathBuf::from);
    let screenshot_has_urls = !urls.is_empty();
    let screenshot_view_ptr: Arc<std::sync::Mutex<Option<usize>>> =
        Arc::new(std::sync::Mutex::new(None));
    let screenshot_view_ptr_for_run = screenshot_view_ptr.clone();

    let app = application();
    app.run(move |cx: &mut App| {
        themes::init(cx);
        register_actions(cx);

        let store_for_win = store.clone();
        let settings_for_win = settings.clone();
        let urls_for_win = urls.clone();
        let harness_mode = harness_out.is_some();
        let window_options = WindowOptions {
            titlebar: Some(
                TitlebarOptions::default()
                    .appears_transparent(true)
                    .traffic_light_position(point(px(14.0), px(13.0))),
            ),
            ..Default::default()
        };
        let handle = cx
            .open_window(window_options, {
                let screenshot_view_ptr = screenshot_view_ptr_for_run.clone();
                move |window, cx| {
                    let raw_wh = <gpui::Window as HasWindowHandle>::window_handle(window)
                        .expect("raw window handle");
                    let rwh = raw_wh.as_raw();
                    if let raw_window_handle::RawWindowHandle::AppKit(h) = rwh {
                        *screenshot_view_ptr.lock().unwrap() = Some(h.ns_view.as_ptr() as usize);
                    }
                    let waker: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
                    let mut engine =
                        engine::ServoEngine::with_options(raw_wh, waker, engine_options.clone())
                            .expect("servo engine");
                    engine.set_hang_timeout(settings_for_win.hang.timeout_seconds);
                    if let Some(b) = &blocker {
                        let b = b.clone();
                        engine.set_request_filter(Box::new(move |url, source, kind| {
                            b.should_block(url, source, kind)
                        }));
                    }
                    let settings_file = settings_file.clone();
                    cx.new(|cx| {
                        let mut view = if harness_mode {
                            // Start empty; harness loop opens URLs one at a time.
                            BrowserWindow::with_urls(
                                engine,
                                store_for_win.clone(),
                                settings_for_win.clone(),
                                Vec::new(),
                                cx,
                            )
                        } else if urls_for_win.is_empty() {
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
                        };
                        view.set_devtools_state(devtools_port, Some(settings_file));
                        if let Some(port) = cdp_port {
                            match CdpBridge::start(port) {
                                Ok(bridge) => view.set_cdp_bridge(Some(bridge)),
                                Err(e) => eprintln!("failed to start cdp server: {e}"),
                            }
                        }
                        if settings_for_win.memory.pressure_response
                            && let Some(src) = crate::memory_pressure::DispatchPressureSource::new()
                        {
                            view.set_pressure_source(Box::new(src));
                        }
                        view
                    })
                }
            })
            .expect("open window");

        let quit_handle = handle;
        cx.on_action(move |_: &actions::Quit, cx| {
            let _ = quit_handle.update(cx, |view, _window, _cx| {
                view.shutdown();
            });
            cx.quit();
        });

        let screenshot_out_for_task = screenshot_out.clone();
        let harness_out_for_task = harness_out.clone();
        let screenshot_view_ptr_for_task = screenshot_view_ptr.clone();
        let memory_report_out_for_task = memory_report_out.clone();
        let harness_urls = urls.clone();
        cx.spawn(async move |cx| {
            if let Some(out_json) = harness_out_for_task {
                run_harness_loop(
                    cx,
                    handle,
                    screenshot_view_ptr_for_task,
                    out_json,
                    harness_urls,
                )
                .await;
                return;
            }
            let mut seen_loading = false;
            let start = std::time::Instant::now();
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
                // cdp quit is detected in tick; also check via bridge state
                let quit = handle
                    .update(cx, |view, _window, _cx| {
                        view.cdp_bridge
                            .as_ref()
                            .map(|b| b.take_quit_requested())
                            .unwrap_or(false)
                    })
                    .unwrap_or(false);
                if quit {
                    let _ = handle.update(cx, |view, _window, _cx| view.shutdown());
                    cx.update(|app| app.quit());
                    break;
                }
                if let Some(out) = memory_report_out_for_task.clone() {
                    let elapsed = start.elapsed();
                    if elapsed >= Duration::from_secs(40) {
                        let rows = handle
                            .update(cx, |view, _window, _cx| view.engine.memory_report())
                            .unwrap_or_default();
                        if let Ok(text) = serde_json::to_string_pretty(&rows) {
                            let _ = std::fs::write(&out, text);
                        }
                        std::process::exit(0);
                    }
                }
                if let Some(out) = screenshot_out_for_task.clone() {
                    let loading = handle
                        .update(cx, |view, _window, _cx| view.toolbar.loading)
                        .unwrap_or(false);
                    if loading {
                        seen_loading = true;
                    }
                    let elapsed = start.elapsed();
                    let delay = Duration::from_millis(screenshot_delay_ms);
                    let max = delay + Duration::from_secs(10);
                    // If a URL was given, wait until loading finished at least once.
                    let ready = if screenshot_has_urls {
                        elapsed >= delay && (!loading && seen_loading)
                    } else {
                        elapsed >= delay
                    };
                    if ready || elapsed >= max {
                        let ptr = *screenshot_view_ptr_for_task.lock().unwrap();
                        let mut success = false;
                        if let Some(p) = ptr {
                            if let Some(wn) = screenshot::window_number_from_view(p as *mut _) {
                                if screenshot::capture_window_png(wn, &out).is_ok() {
                                    success = true;
                                } else {
                                    eprintln!("screenshot capture failed");
                                }
                            } else {
                                eprintln!("could not resolve window number");
                            }
                        } else {
                            eprintln!("window not ready");
                        }
                        if !success {
                            std::process::exit(1);
                        }
                        break;
                    }
                }
            }
            if screenshot_out_for_task.is_some() {
                std::process::exit(0);
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
        KeyBinding::new("cmd-alt-i", actions::devtools::Toggle, None),
        KeyBinding::new("cmd-alt-escape", crate::task_manager::Toggle, None),
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

async fn run_harness_loop(
    cx: &mut gpui::AsyncApp,
    handle: gpui::WindowHandle<BrowserWindow<engine::ServoEngine, SqliteStore>>,
    screenshot_view_ptr: Arc<std::sync::Mutex<Option<usize>>>,
    out_json: PathBuf,
    urls: Vec<Url>,
) {
    use std::time::Instant;
    let mut entries: Vec<serde_json::Value> = Vec::new();
    let out_dir = out_json
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let _ = std::fs::create_dir_all(&out_dir);

    for (i, url) in urls.into_iter().enumerate() {
        let url_str = url.to_string();
        let open_ok = handle
            .update(cx, |view, _window, _cx| {
                // close existing tabs to keep one at a time
                let ids: Vec<_> = view.strip.tabs().iter().map(|t| t.id).collect();
                for id in ids {
                    view.strip.close(&mut view.engine, id);
                }
                view.strip.open(&mut view.engine, &url, true);
            })
            .is_ok();
        if !open_ok {
            entries.push(serde_json::json!({
                "url": url_str.clone(),
                "final_url": url_str,
                "title": "",
                "load_ms": 20000u64,
                "console_errors": [],
                "screenshot_png": "",
                "crashed": true
            }));
            continue;
        }

        let start = Instant::now();
        let mut saw_loading = false;
        let mut done = false;
        let mut crashed = false;
        let mut final_url = url_str.clone();
        let mut title = String::new();
        let mut load_ms: u64 = 20_000;
        while !done {
            cx.background_executor()
                .timer(Duration::from_millis(8))
                .await;
            let tick_ok = handle
                .update(cx, |view, window, cx| {
                    view.tick(window, cx);
                })
                .is_ok();
            if !tick_ok {
                crashed = true;
                break;
            }
            let state = handle
                .update(cx, |view, _window, _cx| {
                    view.active_tab().map(|t| {
                        (
                            t.loading,
                            t.url.to_string(),
                            t.title.clone(),
                            t.crashed.is_some(),
                        )
                    })
                })
                .ok()
                .flatten();
            if let Some((loading, fu, ti, cr)) = state {
                final_url = fu;
                title = ti;
                if loading {
                    saw_loading = true;
                }
                if cr {
                    crashed = true;
                    load_ms = start.elapsed().as_millis() as u64;
                    done = true;
                } else if saw_loading && !loading {
                    load_ms = start.elapsed().as_millis() as u64;
                    done = true;
                }
            }
            if !done && start.elapsed() >= Duration::from_secs(20) {
                load_ms = start.elapsed().as_millis().min(20_000_u128) as u64;
                done = true;
            }
        }

        let shot_name = format!("harness-{i}.png");
        let shot_path = out_dir.join(&shot_name);
        // give a brief moment for paint after load before capture
        for _ in 0..30 {
            cx.background_executor()
                .timer(Duration::from_millis(16))
                .await;
            let _ = handle.update(cx, |view, window, cx| view.tick(window, cx));
        }
        let ptr = *screenshot_view_ptr.lock().unwrap();
        if let Some(p) = ptr
            && let Some(wn) = screenshot::window_number_from_view(p as *mut _)
        {
            let _ = screenshot::capture_window_png(wn, &shot_path);
        }
        let screenshot_png = if shot_path.exists() {
            shot_path.to_string_lossy().into_owned()
        } else {
            String::new()
        };

        entries.push(serde_json::json!({
            "url": url_str,
            "final_url": final_url,
            "title": title,
            "load_ms": load_ms,
            "console_errors": [],
            "screenshot_png": screenshot_png,
            "crashed": crashed
        }));

        let _ = handle.update(cx, |view, _window, _cx| {
            let ids: Vec<_> = view.strip.tabs().iter().map(|t| t.id).collect();
            for id in ids {
                view.strip.close(&mut view.engine, id);
            }
        });
    }

    let report = serde_json::json!({ "entries": entries });
    if let Ok(json) = serde_json::to_string_pretty(&report) {
        let _ = std::fs::write(&out_json, json);
    }
    // Exit without Servo's graceful shutdown: it can hang after content
    // processes were closed, and content processes exit when we do.
    std::process::exit(0);
}
