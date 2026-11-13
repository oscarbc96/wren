use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;
use std::time::{Duration, Instant};

use clap::Parser;
use embedder_traits::EventLoopWaker;
use gpui::{Context, Window, WindowOptions, div, prelude::*, px, rgb};
use gpui_platform::application;
use objc2::MainThreadMarker;
use objc2::MainThreadOnly;
use objc2::rc::Retained;
use objc2_app_kit::{NSAutoresizingMaskOptions, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use raw_window_handle::{AppKitDisplayHandle, AppKitWindowHandle, HasWindowHandle};
use servo::{DeviceIntRect, DeviceIntSize};
use servo::{
    LoadStatus, RenderingContext, Servo, ServoBuilder, WebView, WebViewBuilder, WebViewDelegate,
    WindowRenderingContext,
};
use sysinfo::System;
use url::Url;
use winit::dpi::PhysicalSize;

#[derive(Parser, Debug, Clone)]
struct Args {
    #[arg(long, default_value = "https://example.org")]
    url: String,
    #[arg(long, default_value_t = 0)]
    exit_after: u64,
    #[arg(long, default_value_t = false)]
    two: bool,
}

#[derive(Clone)]
struct Waker;

impl EventLoopWaker for Waker {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(Self)
    }
    fn wake(&self) {
        // GPUI render loop continuously spins Servo and paints.
    }
}

struct AppState {
    servo: Servo,
    rendering_context: Rc<WindowRenderingContext>,
    webviews: RefCell<Vec<WebView>>,
    active: RefCell<usize>,
    started: Instant,
    exit_after: u64,
    done: RefCell<bool>,
    last_size: RefCell<(u32, u32)>,
    child_view: Retained<NSView>,
}

impl WebViewDelegate for AppState {
    fn notify_url_changed(&self, _webview: WebView, url: Url) {
        println!("URL_CHANGED {}", url);
    }
    fn notify_page_title_changed(&self, _webview: WebView, title: Option<String>) {
        println!("TITLE {}", title.unwrap_or_default());
    }
    fn notify_load_status_changed(&self, _webview: WebView, status: LoadStatus) {
        match status {
            LoadStatus::Started => println!("LOAD_STARTED"),
            LoadStatus::Complete => println!("LOAD_COMPLETE"),
            _ => {}
        }
    }
    fn notify_new_frame_ready(&self, _webview: WebView) {
        // GPUI render loop continuously repaints.
    }
    fn notify_crashed(&self, _webview: WebView, reason: String, _backtrace: Option<String>) {
        println!("CRASHED {}", reason);
    }
}

struct RootView {
    state: Rc<AppState>,
}

const HEADER_HEIGHT: f32 = 28.0;

impl RootView {
    /// One Servo step: spin the event loop, keep the child view sized to the
    /// window, paint the active webview and present. Called from the tick task.
    fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = self.state.clone();

        state.servo.spin_event_loop();

        let bounds = window.bounds();
        let scale = window.scale_factor();
        let total_w = f32::from(bounds.size.width);
        let total_h = f32::from(bounds.size.height);
        let content_h = (total_h - HEADER_HEIGHT).max(1.0);
        let pw = (total_w * scale).round() as u32;
        let ph = (content_h * scale).round() as u32;

        if pw > 0 && ph > 0 {
            let (lw, lh) = *state.last_size.borrow();
            if pw != lw || ph != lh {
                *state.last_size.borrow_mut() = (pw, ph);
                let size = PhysicalSize::new(pw, ph);
                state.rendering_context.resize(size);
                for v in state.webviews.borrow().iter() {
                    v.resize(size);
                }
            }
            let frame = NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(total_w as f64, content_h as f64),
            );
            state.child_view.setFrame(frame);
        }

        let idx = *state.active.borrow();
        let views = state.webviews.borrow();
        if let Some(v) = views.get(idx) {
            v.paint();
        }
        drop(views);
        let exiting = state.exit_after > 0
            && state.started.elapsed() >= Duration::from_secs(state.exit_after)
            && !*state.done.borrow();
        if exiting {
            // Read back the painted frame to prove Servo drew into the child view.
            let (w, h) = *state.last_size.borrow();
            let rect = DeviceIntRect::from_size(DeviceIntSize::new(w as i32, h as i32));
            if let Some(img) = state.rendering_context.read_to_image(rect) {
                let total = (img.width() * img.height()) as usize;
                let dark = img.pixels().filter(|p| p.0[0] < 128 && p.0[3] > 0).count();
                println!(
                    "FRAME {}x{} dark_pixels={dark} of {total}",
                    img.width(),
                    img.height()
                );
            } else {
                println!("FRAME none");
            }
        }
        state.rendering_context.present();

        if state.exit_after > 0 && state.started.elapsed() >= Duration::from_secs(state.exit_after)
        {
            if !*state.done.borrow() {
                *state.done.borrow_mut() = true;
                let mut sys = System::new_all().unwrap();
                sys.refresh_all();
                if let Ok(pid) = sysinfo::get_current_pid()
                    && let Some(proc) = sys.process(pid)
                {
                    println!("RSS_BYTES={}", proc.memory());
                }
            }
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

fn main() -> Result<(), Box<dyn Error>> {
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
                    let parent_ptr = match raw_wh.as_raw() {
                        raw_window_handle::RawWindowHandle::AppKit(h) => h.ns_view.as_ptr(),
                        _ => panic!("expected appkit window handle"),
                    };
                    let parent_view = unsafe { &*(parent_ptr as *const NSView) };

                    let bounds = window.bounds();
                    let scale = window.scale_factor();
                    let total_w = f32::from(bounds.size.width).max(1.0);
                    let total_h = f32::from(bounds.size.height).max(1.0);
                    let content_h = (total_h - HEADER_HEIGHT).max(1.0);
                    let pw = (total_w * scale).round() as u32;
                    let ph = (content_h * scale).round() as u32;
                    let frame = NSRect::new(
                        NSPoint::new(0.0, 0.0),
                        NSSize::new(total_w as f64, content_h as f64),
                    );
                    let mtm = MainThreadMarker::new().expect("main thread");
                    let child: Retained<NSView> = NSView::initWithFrame(NSView::alloc(mtm), frame);
                    let mask = NSAutoresizingMaskOptions::ViewWidthSizable
                        | NSAutoresizingMaskOptions::ViewHeightSizable;
                    child.setAutoresizingMask(mask);
                    parent_view.addSubview(&child);

                    let waker = Waker;

                    let child_ptr = Retained::as_ptr(&child) as *mut std::ffi::c_void;
                    let nonnull = std::ptr::NonNull::new(child_ptr).expect("child view ptr");
                    let appkit_child = AppKitWindowHandle::new(nonnull);
                    let raw_child_wh = raw_window_handle::RawWindowHandle::AppKit(appkit_child);
                    let raw_dh =
                        raw_window_handle::RawDisplayHandle::AppKit(AppKitDisplayHandle::new());

                    let size = PhysicalSize::new(pw.max(1), ph.max(1));
                    let safe_dh = unsafe { raw_window_handle::DisplayHandle::borrow_raw(raw_dh) };
                    let safe_wh =
                        unsafe { raw_window_handle::WindowHandle::borrow_raw(raw_child_wh) };
                    let rendering_context = Rc::new(
                        WindowRenderingContext::new(safe_dh, safe_wh, size)
                            .expect("rendering context"),
                    );
                    let _ = rendering_context.make_current();

                    let servo = ServoBuilder::default()
                        .event_loop_waker(Box::new(waker))
                        .build();
                    servo.setup_logging();

                    let state = Rc::new(AppState {
                        servo,
                        rendering_context: rendering_context.clone(),
                        webviews: RefCell::new(Vec::new()),
                        active: RefCell::new(0),
                        started: Instant::now(),
                        exit_after: args.exit_after,
                        done: RefCell::new(false),
                        last_size: RefCell::new((size.width, size.height)),
                        child_view: child,
                    });

                    let wv1 = WebViewBuilder::new(&state.servo, state.rendering_context.clone())
                        .url(url.clone())
                        .delegate(state.clone())
                        .build();
                    state.webviews.borrow_mut().push(wv1);

                    if args.two {
                        let wv2 =
                            WebViewBuilder::new(&state.servo, state.rendering_context.clone())
                                .url(Url::parse("https://servo.org").expect("servo url"))
                                .delegate(state.clone())
                                .build();
                        wv2.hide();
                        state.webviews.borrow_mut().push(wv2);
                    }
                    cx.new(|_| RootView { state })
                },
            )
            .expect("open window");
        // Drive Servo from a foreground tick: refreshing the window re-runs
        // `render`, which spins Servo and paints. GPUI ignores `refresh` calls
        // made during a draw, so the tick has to come from outside `render`.
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
