use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;
use std::time::{Duration, Instant};

use clap::Parser;
use embedder_traits::EventLoopWaker;
use servo::{
    LoadStatus, RenderingContext, Servo, ServoBuilder, WebView, WebViewBuilder, WebViewDelegate,
    WindowRenderingContext,
};
use sysinfo::System;
use tracing::warn;
use url::Url;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::EventLoop;
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::{Window, WindowAttributes};

#[derive(Parser, Debug, Clone)]
struct Args {
    #[arg(long, default_value = "https://example.org")]
    url: String,
    #[arg(long, default_value_t = 0)]
    exit_after: u64,
    #[arg(long, default_value_t = false)]
    two: bool,
}

#[derive(Debug)]
struct WakerEvent;

#[derive(Clone)]
struct Waker(winit::event_loop::EventLoopProxy<WakerEvent>);

impl Waker {
    fn new(event_loop: &EventLoop<WakerEvent>) -> Self {
        Self(event_loop.create_proxy())
    }
}

impl EventLoopWaker for Waker {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(Self(self.0.clone()))
    }
    fn wake(&self) {
        if let Err(e) = self.0.send_event(WakerEvent) {
            warn!(?e, "failed to wake event loop");
        }
    }
}

struct AppState {
    window: Window,
    servo: Servo,
    rendering_context: Rc<WindowRenderingContext>,
    webviews: RefCell<Vec<WebView>>,
    active: RefCell<usize>,
    two: bool,
    started: Instant,
    exit_after: u64,
    done: RefCell<bool>,
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
            LoadStatus::Started => println!("LOAD_START"),
            LoadStatus::Complete => println!("LOAD_COMPLETE"),
            _ => {}
        }
    }
    fn notify_new_frame_ready(&self, _webview: WebView) {
        self.window.request_redraw();
    }
    fn notify_crashed(&self, _webview: WebView, reason: String, _backtrace: Option<String>) {
        println!("CRASHED {}", reason);
    }
}

enum App {
    Initial(Waker, Args),
    Running(Rc<AppState>),
}

impl ApplicationHandler<WakerEvent> for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Self::Initial(waker, args) = self {
            let url = Url::parse(&args.url).expect("valid url");
            let display_handle = event_loop.display_handle().expect("display handle");
            let window = event_loop
                .create_window(WindowAttributes::default().with_title("Wren spike"))
                .expect("window");
            let window_handle = window.window_handle().expect("window handle");
            let size = window.inner_size();

            let rendering_context = Rc::new(
                WindowRenderingContext::new(display_handle, window_handle, size)
                    .expect("rendering context"),
            );
            let _ = rendering_context.make_current();

            let servo = ServoBuilder::default()
                .event_loop_waker(Box::new(waker.clone()))
                .build();
            servo.setup_logging();

            let state = Rc::new(AppState {
                window,
                servo,
                rendering_context: rendering_context.clone(),
                webviews: RefCell::new(Vec::new()),
                active: RefCell::new(0),
                two: args.two,
                started: Instant::now(),
                exit_after: args.exit_after,
                done: RefCell::new(false),
            });

            let wv1 = WebViewBuilder::new(&state.servo, state.rendering_context.clone())
                .url(url.clone())
                .delegate(state.clone())
                .build();
            state.webviews.borrow_mut().push(wv1);

            if args.two {
                let wv2 = WebViewBuilder::new(&state.servo, state.rendering_context.clone())
                    .url(Url::parse("https://example.org").unwrap())
                    .delegate(state.clone())
                    .build();
                wv2.hide();
                state.webviews.borrow_mut().push(wv2);
            }

            *self = Self::Running(state);
        }
    }

    fn user_event(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, _event: WakerEvent) {
        if let Self::Running(state) = self {
            state.servo.spin_event_loop();
            check_exit(state, event_loop);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        if let Self::Running(state) = self {
            state.servo.spin_event_loop();
            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::Resized(new_size) => {
                    let size: PhysicalSize<u32> = new_size;
                    for v in state.webviews.borrow().iter() {
                        v.resize(size);
                    }
                }
                WindowEvent::RedrawRequested => {
                    let idx = *state.active.borrow();
                    let views = state.webviews.borrow();
                    if let Some(v) = views.get(idx) {
                        v.paint();
                    }
                    state.rendering_context.present();
                }
                _ => {}
            }
            check_exit(state, event_loop);
        }
    }

    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Self::Running(state) = self {
            if state.two {
                let elapsed = state.started.elapsed().as_secs();
                let idx = (elapsed / 2) as usize % 2;
                if idx != *state.active.borrow() {
                    let views = state.webviews.borrow();
                    if let Some(v) = views.get(*state.active.borrow()) {
                        v.hide();
                    }
                    if let Some(v) = views.get(idx) {
                        v.show();
                    }
                    *state.active.borrow_mut() = idx;
                }
            }
            check_exit(state, event_loop);
        }
    }
}

fn check_exit(state: &AppState, event_loop: &winit::event_loop::ActiveEventLoop) {
    if state.exit_after > 0 && state.started.elapsed() >= Duration::from_secs(state.exit_after) {
        if !*state.done.borrow() {
            *state.done.borrow_mut() = true;
            let mut sys = System::new_all().unwrap();
            sys.refresh_all();
            let pid = sysinfo::get_current_pid().unwrap();
            let proc = sys.process(pid).unwrap();
            println!("RSS_BYTES={}", proc.memory());
        }
        event_loop.exit();
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install crypto provider");

    let args = Args::parse();

    let event_loop = EventLoop::with_user_event().build().expect("event loop");
    let waker = Waker::new(&event_loop);

    let mut app = App::Initial(waker, args);

    event_loop.run_app(&mut app)?;
    Ok(())
}
