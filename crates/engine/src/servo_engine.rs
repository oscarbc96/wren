//! Servo-backed `Engine` implementation.

use crate::api::{
    Engine, EngineEvent, EngineInput, LoadState, Modifiers, MouseButton, RequestFilter, RequestKind,
    ViewRect, WebViewId,
};
use anyhow::{Context, Result};
use dpi::PhysicalSize;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, Message, define_class, msg_send};
use objc2_app_kit::NSView;
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize};
use raw_window_handle::{
    AppKitDisplayHandle, AppKitWindowHandle, RawDisplayHandle, RawWindowHandle,
};
use servo::{
    CreateNewWebViewRequest, EventLoopWaker, InputEvent, LoadStatus, MouseButtonAction,
    MouseButtonEvent, MouseLeftViewportEvent, MouseMoveEvent, Opts, RenderingContext, Servo,
    ServoBuilder, WebView, WebViewBuilder, WebViewDelegate, WheelDelta, WheelEvent, WheelMode,
    WindowRenderingContext,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessesToUpdate, System};
use url::Url;

/// Returns the content-process token when args contain `--content-process <token>`.
pub fn content_process_token(args: &[String]) -> Option<String> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--content-process" {
            return iter.next().cloned();
        }
    }
    None
}

/// Runs as a Servo content process when launched with `--content-process <token>`.
///
/// Returns `Some(())` if this process was a content process and has completed,
/// or `None` if it should continue as the main browser process.
pub fn content_process_main() -> Option<()> {
    let args: Vec<String> = std::env::args().collect();
    let token = content_process_token(&args)?;
    // If the browser process dies without shutting Servo down (crash, SIGKILL),
    // this process is re-parented to launchd. Exit instead of lingering.
    let parent = std::os::unix::process::parent_id();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(500));
            if std::os::unix::process::parent_id() != parent {
                std::process::exit(0);
            }
        }
    });
    servo::run_content_process(token);
    Some(())
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "WrenContentView"]
    struct WrenContentView;

    unsafe impl NSObjectProtocol for WrenContentView {}

    impl WrenContentView {
        #[unsafe(method(hitTest:))]
        fn hit_test(&self, _point: NSPoint) -> *mut NSView {
            std::ptr::null_mut()
        }

        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            false
        }
    }
);

fn map_modifiers(m: &Modifiers) -> servo::Modifiers {
    let mut out = servo::Modifiers::empty();
    if m.shift {
        out |= servo::Modifiers::SHIFT;
    }
    if m.ctrl {
        out |= servo::Modifiers::CONTROL;
    }
    if m.alt {
        out |= servo::Modifiers::ALT;
    }
    if m.cmd {
        out |= servo::Modifiers::META;
    }
    out
}

fn map_button(button: MouseButton) -> servo::MouseButton {
    match button {
        MouseButton::Left => servo::MouseButton::Primary,
        MouseButton::Right => servo::MouseButton::Secondary,
        MouseButton::Middle => servo::MouseButton::Auxiliary,
    }
}

/// Converts an `EngineInput` into a Servo input event.
///
/// Returns `None` if the input cannot be represented.
pub fn to_servo_input(ev: &EngineInput, scale: f32) -> Option<InputEvent> {
    match ev {
        EngineInput::MouseMove { x, y } => Some(InputEvent::MouseMove(MouseMoveEvent::new(
            servo::WebViewPoint::Device(servo::DevicePoint::new(x * scale, y * scale)),
        ))),
        EngineInput::MouseDown { button, x, y } => {
            Some(InputEvent::MouseButton(MouseButtonEvent::new(
                MouseButtonAction::Down,
                map_button(*button),
                servo::WebViewPoint::Device(servo::DevicePoint::new(x * scale, y * scale)),
            )))
        }
        EngineInput::MouseUp { button, x, y } => {
            Some(InputEvent::MouseButton(MouseButtonEvent::new(
                MouseButtonAction::Up,
                map_button(*button),
                servo::WebViewPoint::Device(servo::DevicePoint::new(x * scale, y * scale)),
            )))
        }
        EngineInput::Wheel { dx, dy, x, y } => Some(InputEvent::Wheel(WheelEvent::new(
            WheelDelta {
                x: (dx * scale) as f64,
                y: (dy * scale) as f64,
                z: 0.0,
                mode: WheelMode::DeltaPixel,
            },
            servo::WebViewPoint::Device(servo::DevicePoint::new(x * scale, y * scale)),
        ))),
        EngineInput::Key {
            down,
            key,
            code,
            modifiers,
        } => {
            let state = if *down {
                servo::KeyState::Down
            } else {
                servo::KeyState::Up
            };
            let key = servo::Key::from_str(key).ok()?;
            let code = servo::Code::from_str(code).ok()?;
            let modifiers = map_modifiers(modifiers);
            Some(InputEvent::Keyboard(
                servo::KeyboardEvent::new_without_event(
                    state,
                    key,
                    code,
                    servo::Location::Standard,
                    modifiers,
                    false,
                    false,
                ),
            ))
        }
        EngineInput::MouseLeft => Some(InputEvent::MouseLeftViewport(
            MouseLeftViewportEvent::default(),
        )),
    }
}

struct WakerImpl(Arc<dyn Fn() + Send + Sync>);

impl EventLoopWaker for WakerImpl {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(Self(self.0.clone()))
    }

    fn wake(&self) {
        (self.0)();
    }
}

#[derive(Clone, Default)]
struct EventQueue(Arc<Mutex<Vec<EngineEvent>>>);

impl EventQueue {
    fn push(&self, ev: EngineEvent) {
        self.0.lock().expect("event queue poisoned").push(ev);
    }

    fn drain(&self) -> Vec<EngineEvent> {
        std::mem::take(&mut *self.0.lock().expect("event queue poisoned"))
    }
}

struct Delegate {
    id: WebViewId,
    events: EventQueue,
    filter: std::rc::Rc<std::cell::RefCell<Option<RequestFilter>>>,
}

impl WebViewDelegate for Delegate {
    fn notify_url_changed(&self, _webview: WebView, url: Url) {
        self.events
            .push(EngineEvent::UrlChanged { id: self.id, url });
    }

    fn notify_page_title_changed(&self, _webview: WebView, title: Option<String>) {
        self.events.push(EngineEvent::TitleChanged {
            id: self.id,
            title: title.unwrap_or_default(),
        });
    }

    fn notify_load_status_changed(&self, _webview: WebView, status: LoadStatus) {
        let state = match status {
            LoadStatus::Started => LoadState::Started,
            LoadStatus::Complete => LoadState::Complete,
            LoadStatus::HeadParsed | LoadStatus::Stopped => return,
        };
        self.events.push(EngineEvent::Load { id: self.id, state });
    }

    fn notify_history_changed(&self, _webview: WebView, _entries: Vec<Url>, current: usize) {
        let total = _entries.len();
        self.events.push(EngineEvent::History {
            id: self.id,
            can_back: current > 0,
            can_forward: current + 1 < total,
        });
    }

    fn notify_crashed(&self, _webview: WebView, reason: String, _backtrace: Option<String>) {
        self.events.push(EngineEvent::Crashed {
            id: self.id,
            reason,
        });
    }

    fn request_create_new(&self, _parent_webview: WebView, request: CreateNewWebViewRequest) {
        // Servo 0.8 does not expose the target URL on this request; emit an
        // OpenTab request and deny Servo-side creation.
        self.events.push(EngineEvent::OpenTab {
            opener: self.id,
            url: Url::parse("about:blank").expect("valid url"),
        });
        drop(request);
    }

    fn load_web_resource(&self, _webview: WebView, load: servo::WebResourceLoad) {
        let req = load.request();
        let url = req.url.as_url().clone();
        let source = req
            .referrer_url
            .as_ref()
            .map(|u| u.as_url().clone())
            .unwrap_or_else(|| url.clone());
        let kind = match req.destination {
            content_security_policy::Destination::Document
            | content_security_policy::Destination::Frame
            | content_security_policy::Destination::IFrame => RequestKind::Document,
            content_security_policy::Destination::Script => RequestKind::Script,
            content_security_policy::Destination::Image => RequestKind::Image,
            content_security_policy::Destination::Style => RequestKind::Stylesheet,
            content_security_policy::Destination::Font => RequestKind::Font,
            content_security_policy::Destination::Audio
            | content_security_policy::Destination::Video
            | content_security_policy::Destination::Track => RequestKind::Media,
            _ => RequestKind::Other,
        };
        // XHR/fetch destinations often report as None/Json/Text; treat Json/Text as Xhr.
        let kind = if matches!(
            req.destination,
            content_security_policy::Destination::Json | content_security_policy::Destination::Text
        ) {
            RequestKind::Xhr
        } else {
            kind
        };

        let blocked = {
            let f = self.filter.borrow();
            f.as_ref().map(|f| f(&url, &source, kind)).unwrap_or(false)
        };
        if blocked && kind != RequestKind::Document {
            let intercepted = load.intercept(servo::WebResourceResponse::new(url.clone()));
            intercepted.finish();
        }
    }
}

struct WebViewEntry {
    webview: WebView,
}

/// Servo-backed engine.
///
/// Runs Servo in multiprocess mode: every web view is its own browsing
/// context group, so it gets its own content process (spec 7.2). Servo 0.8
/// does not report which process serves which web view, nor that a content
/// process died, so the engine watches its child processes itself: a new
/// child is attributed to the oldest web view still waiting for one, and a
/// child that disappears produces `EngineEvent::Crashed` for its web view.
/// Cross-site iframes may add processes and confuse the attribution; good
/// enough for M0, revisit with a Servo patch in M1.
pub struct ServoEngine {
    _servo: Option<Servo>,
    sys: System,
    pids: HashMap<u32, WebViewId>,
    awaiting_pid: VecDeque<WebViewId>,
    last_scan: Instant,
    /// A content process died under us. Servo 0.8 then never finishes its
    /// graceful shutdown, so `shutdown` must not wait for it.
    lost_process: bool,
    rendering_context: Rc<WindowRenderingContext>,
    parent_view: Retained<NSView>,
    child_view: Retained<NSView>,
    views: HashMap<WebViewId, WebViewEntry>,
    next_id: u64,
    events: EventQueue,
    shown: Option<WebViewId>,
    request_filter: std::rc::Rc<std::cell::RefCell<Option<RequestFilter>>>,
}

impl ServoEngine {
    /// Creates a new `ServoEngine` rendering into a child NSView of `parent`.
    pub fn new(
        parent: raw_window_handle::WindowHandle<'_>,
        waker: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Self> {
        let parent_ptr = match parent.as_raw() {
            RawWindowHandle::AppKit(h) => h.ns_view.as_ptr(),
            other => anyhow::bail!("unsupported parent window handle: {other:?}"),
        };
        let parent_view = unsafe { &*(parent_ptr as *const NSView) }.retain();

        let mtm =
            MainThreadMarker::new().context("ServoEngine must be created on the main thread")?;
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1.0, 1.0));
        let child: Retained<NSView> = unsafe {
            let obj: Retained<WrenContentView> =
                msg_send![mtm.alloc::<WrenContentView>(), initWithFrame: frame];
            Retained::into_super(obj)
        };
        child.setHidden(true);
        parent_view.addSubview(&child);

        let child_ptr = Retained::as_ptr(&child) as *mut std::ffi::c_void;
        let nonnull = std::ptr::NonNull::new(child_ptr).context("null child NSView")?;
        let appkit_child = AppKitWindowHandle::new(nonnull);
        let raw_child_wh = RawWindowHandle::AppKit(appkit_child);
        let raw_dh = RawDisplayHandle::AppKit(AppKitDisplayHandle::new());
        let safe_dh = unsafe { raw_window_handle::DisplayHandle::borrow_raw(raw_dh) };
        let safe_wh = unsafe { raw_window_handle::WindowHandle::borrow_raw(raw_child_wh) };
        let rendering_context = Rc::new(
            WindowRenderingContext::new(safe_dh, safe_wh, PhysicalSize::new(1, 1))
                .map_err(|e| anyhow::anyhow!("failed to create rendering context: {e:?}"))?,
        );
        let _ = rendering_context.make_current();

        let opts = Opts {
            multiprocess: true,
            ..Opts::default()
        };
        let servo = ServoBuilder::default()
            .opts(opts)
            .event_loop_waker(Box::new(WakerImpl(waker)))
            .build();
        servo.setup_logging();

        Ok(Self {
            _servo: Some(servo),
            sys: System::new().context("process table")?,
            pids: HashMap::new(),
            awaiting_pid: VecDeque::new(),
            last_scan: Instant::now(),
            lost_process: false,
            rendering_context,
            parent_view,
            child_view: child,
            views: HashMap::new(),
            next_id: 0,
            events: EventQueue::default(),
            shown: None,
            request_filter: std::rc::Rc::new(std::cell::RefCell::new(None)),
        })
    }

    /// Content process id of each web view, as attributed by the child
    /// process watcher (see the type docs).
    pub fn content_pids(&self) -> Vec<(WebViewId, u32)> {
        let mut v: Vec<_> = self.pids.iter().map(|(pid, id)| (*id, *pid)).collect();
        v.sort();
        v
    }

    /// Shuts Servo down (content processes exit), then kills any child that
    /// is still alive. Call before quitting; `Drop` calls it too.
    pub fn shutdown(&mut self) {
        self.views.clear();
        self.shown = None;
        let Some(servo) = self._servo.take() else {
            return;
        };
        if self.lost_process {
            // Servo would spin forever waiting for the dead pipeline: kill the
            // remaining content processes and leak Servo; the app is exiting.
            self.kill_children();
            std::mem::forget(servo);
        } else {
            drop(servo);
            self.kill_children();
        }
        self.pids.clear();
    }

    fn kill_children(&mut self) {
        for pid in self.child_pids() {
            if let Some(p) = self.sys.process(Pid::from_u32(pid)) {
                p.kill();
            }
        }
    }

    fn servo(&self) -> &Servo {
        self._servo
            .as_ref()
            .expect("ServoEngine used after shutdown")
    }

    fn child_pids(&mut self) -> HashSet<u32> {
        self.sys.refresh_processes(ProcessesToUpdate::All, true);
        let me = Pid::from_u32(std::process::id());
        self.sys
            .processes()
            .iter()
            .filter(|(_, p)| p.parent() == Some(me))
            .map(|(pid, _)| pid.as_u32())
            .collect()
    }

    /// Attributes new child processes to web views and reports dead ones.
    fn watch_children(&mut self) {
        if self.last_scan.elapsed() < Duration::from_millis(250) {
            return;
        }
        self.last_scan = Instant::now();
        let alive = self.child_pids();
        let gone: Vec<u32> = self
            .pids
            .keys()
            .filter(|p| !alive.contains(p))
            .copied()
            .collect();
        for pid in gone {
            self.lost_process = true;
            if let Some(id) = self.pids.remove(&pid)
                && self.views.contains_key(&id)
            {
                self.events.push(EngineEvent::Crashed {
                    id,
                    reason: format!("content process {pid} exited"),
                });
            }
        }
        let mut new: Vec<u32> = alive
            .into_iter()
            .filter(|p| !self.pids.contains_key(p))
            .collect();
        new.sort();
        for pid in new {
            let Some(id) = self.awaiting_pid.pop_front() else {
                break;
            };
            if self.views.contains_key(&id) {
                self.pids.insert(pid, id);
            }
        }
    }

    fn alloc_id(&mut self) -> WebViewId {
        let id = WebViewId(self.next_id);
        self.next_id += 1;
        id
    }
}

impl Engine for ServoEngine {
    fn create(&mut self, url: &Url) -> WebViewId {
        let id = self.alloc_id();
        let delegate = Rc::new(Delegate {
            id,
            events: self.events.clone(),
            filter: self.request_filter.clone(),
        });
        let webview = WebViewBuilder::new(self.servo(), self.rendering_context.clone())
            .url(url.clone())
            .delegate(delegate)
            .build();
        // Hide unless it becomes the shown view.
        webview.hide();
        self.views.insert(id, WebViewEntry { webview });
        self.awaiting_pid.push_back(id);
        id
    }

    fn close(&mut self, id: WebViewId) {
        if self.shown == Some(id) {
            self.shown = None;
            self.child_view.setHidden(true);
        }
        self.views.remove(&id);
        self.awaiting_pid.retain(|w| *w != id);
        self.pids.retain(|_, w| *w != id);
    }

    fn load(&mut self, id: WebViewId, url: &Url) {
        if let Some(entry) = self.views.get(&id) {
            entry.webview.load(url.clone());
        }
    }

    fn back(&mut self, id: WebViewId) {
        if let Some(entry) = self.views.get(&id) {
            let _ = entry.webview.go_back(1);
        }
    }

    fn forward(&mut self, id: WebViewId) {
        if let Some(entry) = self.views.get(&id) {
            let _ = entry.webview.go_forward(1);
        }
    }

    fn reload(&mut self, id: WebViewId) {
        if let Some(entry) = self.views.get(&id) {
            entry.webview.reload();
        }
    }

    fn show(&mut self, id: WebViewId, rect: ViewRect) {
        // Hide currently shown webview.
        if let Some(current) = self.shown
            && current != id
            && let Some(entry) = self.views.get(&current)
        {
            entry.webview.hide();
        }

        let parent_bounds = self.parent_view.bounds();
        let flipped = self.parent_view.isFlipped();
        let parent_h = parent_bounds.size.height as f32;
        let y = if flipped {
            rect.y
        } else {
            (parent_h - rect.y - rect.height).max(0.0)
        };
        let frame = NSRect::new(
            NSPoint::new(rect.x as f64, y as f64),
            NSSize::new(rect.width.max(1.0) as f64, rect.height.max(1.0) as f64),
        );
        self.child_view.setFrame(frame);
        self.child_view.setHidden(false);

        let scale = self
            .child_view
            .window()
            .map(|w| w.backingScaleFactor() as f32)
            .unwrap_or(1.0);
        let pw = (rect.width.max(1.0) * scale).round().max(1.0) as u32;
        let ph = (rect.height.max(1.0) * scale).round().max(1.0) as u32;
        let size = PhysicalSize::new(pw, ph);
        self.rendering_context.resize(size);

        if let Some(entry) = self.views.get(&id) {
            entry.webview.resize(size);
            entry.webview.show();
            self.shown = Some(id);
        }
    }

    fn hide(&mut self, id: WebViewId) {
        if let Some(entry) = self.views.get(&id) {
            entry.webview.hide();
        }
        if self.shown == Some(id) {
            self.shown = None;
            self.child_view.setHidden(true);
        }
    }

    fn input(&mut self, id: WebViewId, ev: EngineInput) {
        let Some(entry) = self.views.get(&id) else {
            return;
        };
        let scale = self
            .child_view
            .window()
            .map(|w| w.backingScaleFactor() as f32)
            .unwrap_or(1.0);
        if let Some(servo_ev) = to_servo_input(&ev, scale) {
            entry.webview.notify_input_event(servo_ev);
        }
    }

    fn run_script(&mut self, id: WebViewId, js: &str) {
        if let Some(entry) = self.views.get(&id) {
            entry.webview.evaluate_javascript(js, |_| {});
        }
    }

    fn spin(&mut self) {
        if self._servo.is_none() {
            return;
        }
        self.servo().spin_event_loop();
        self.watch_children();
        if let Some(id) = self.shown
            && let Some(entry) = self.views.get(&id)
        {
            entry.webview.paint();
        }
        self.rendering_context.present();
    }

    fn drain_events(&mut self) -> Vec<EngineEvent> {
        self.events.drain()
    }

    fn set_request_filter(&mut self, f: RequestFilter) {
        *self.request_filter.borrow_mut() = Some(f);
    }

    fn shutdown(&mut self) {
        ServoEngine::shutdown(self);
    }
}

impl Drop for ServoEngine {
    fn drop(&mut self) {
        self.shutdown();
    }
}
