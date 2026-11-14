use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::Parser;
use engine::{Engine, EngineEvent, LoadState, ServoEngine, ViewRect, WebViewId};
use gpui::{Context, Window, WindowOptions, div, prelude::*, px, rgb};
use gpui_platform::application;
use raw_window_handle::HasWindowHandle;
use sysinfo::System;
use url::Url;

#[derive(Parser, Debug, Clone)]
struct Args {
    #[arg(long, default_value = "https://example.org")]
    url: String,
    #[arg(long, default_value_t = 0)]
    exit_after: u64,
    #[arg(long, default_value_t = false)]
    two: bool,
}

const HEADER_HEIGHT: f32 = 28.0;

struct RootView {
    engine: ServoEngine,
    webviews: Vec<WebViewId>,
    active: usize,
    started: Instant,
    exit_after: u64,
    done: bool,
}

impl RootView {
    fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Show active webview in content area below the header.
        let bounds = window.bounds();
        let total_w = f32::from(bounds.size.width).max(1.0);
        let total_h = f32::from(bounds.size.height).max(1.0);
        let content_h = (total_h - HEADER_HEIGHT).max(1.0);
        let rect = ViewRect {
            x: 0.0,
            y: HEADER_HEIGHT,
            width: total_w,
            height: content_h,
        };
        if let Some(&id) = self.webviews.get(self.active) {
            self.engine.show(id, rect);
        }

        self.engine.spin();
        for ev in self.engine.drain_events() {
            match ev {
                EngineEvent::UrlChanged { url, .. } => println!("URL_CHANGED {url}"),
                EngineEvent::TitleChanged { title, .. } => println!("TITLE {title}"),
                EngineEvent::Load {
                    state: LoadState::Started,
                    ..
                } => println!("LOAD_STARTED"),
                EngineEvent::Load {
                    state: LoadState::Complete,
                    ..
                } => println!("LOAD_COMPLETE"),
                EngineEvent::Load {
                    state: LoadState::Failed(reason),
                    ..
                } => println!("LOAD_FAILED {reason}"),
                EngineEvent::Crashed { reason, .. } => println!("CRASHED {reason}"),
                EngineEvent::History { .. }
                | EngineEvent::OpenTab { .. }
                | EngineEvent::Audible { .. } => {}
            }
        }

        if self.exit_after > 0
            && self.started.elapsed() >= Duration::from_secs(self.exit_after)
            && !self.done
        {
            self.done = true;
            if let Ok(mut sys) = System::new_all() {
                sys.refresh_all();
                if let Ok(pid) = sysinfo::get_current_pid()
                    && let Some(proc) = sys.process(pid)
                {
                    println!("RSS_BYTES={}", proc.memory());
                }
            }
            println!("CONTENT_PIDS {:?}", self.engine.content_pids());
            if let Ok(mut sys) = System::new_all() {
                sys.refresh_all();
                let me = sysinfo::Pid::from_u32(std::process::id());
                let tree: u64 = sys
                    .processes()
                    .iter()
                    .filter(|(pid, p)| **pid == me || p.parent() == Some(me))
                    .map(|(_, p)| p.memory())
                    .sum();
                println!("TREE_RSS_BYTES={tree}");
            }
            self.engine.shutdown();
            cx.quit();
        }
    }
}

impl gpui::Render for RootView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl gpui::IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(HEADER_HEIGHT))
                    .w_full()
                    .flex()
                    .items_center()
                    .px(px(8.))
                    .bg(rgb(0xf3f3f3))
                    .child("Wren Servo spike"),
            )
            .child(div().flex_1())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if engine::content_process_main().is_some() {
        return Ok(());
    }

    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install crypto provider");

    let args = Args::parse();
    let app = application();
    app.run(move |cx| {
        let args = args.clone();
        let url = Url::parse(&args.url).expect("valid url");

        let handle = cx
            .open_window(
                WindowOptions::default().window_bounds(gpui::WindowBounds::Windowed(
                    gpui::Bounds::centered(None, gpui::size(gpui::px(800.), gpui::px(600.)), cx),
                )),
                move |window, cx| {
                    let raw_wh = <Window as HasWindowHandle>::window_handle(window)
                        .expect("raw window handle");
                    // Wake is a no-op while the 8 ms tick remains; ServoEngine's
                    // waker would signal the foreground task once wake-driven
                    // scheduling replaces polling.
                    let waker: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
                    let mut engine = ServoEngine::new(raw_wh, waker).expect("servo engine");

                    let id1 = engine.create(&url);
                    let mut webviews = vec![id1];
                    if args.two {
                        let id2 =
                            engine.create(&Url::parse("https://servo.org").expect("servo url"));
                        engine.hide(id2);
                        webviews.push(id2);
                    }

                    cx.new(|_| RootView {
                        engine,
                        webviews,
                        active: 0,
                        started: Instant::now(),
                        exit_after: args.exit_after,
                        done: false,
                    })
                },
            )
            .expect("open window");

        cx.spawn(async move |cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(8))
                    .await;
                if handle
                    .update(cx, |view, window, cx| view.tick(window, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    });

    Ok(())
}
