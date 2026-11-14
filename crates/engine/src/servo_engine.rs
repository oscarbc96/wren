//! Servo-backed `Engine` implementation.

use crate::api::{
    Engine, EngineEvent, EngineInput, EngineOptions, LoadState, MemoryRow, Modifiers, MouseButton,
    NavigationFilter, ProcessInfo, RequestFilter, RequestKind, ViewRect, WebViewId,
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
    MouseButtonEvent, MouseLeftViewportEvent, MouseMoveEvent, NavigationRequest, Opts,
    RenderingContext, Servo, ServoBuilder, WebView, WebViewBuilder, WebViewDelegate, WheelDelta,
    WheelEvent, WheelMode, WindowRenderingContext,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessesToUpdate, System};
use url::Url;

fn is_download_url(url: &Url) -> bool {
    let path = url.path().to_ascii_lowercase();
    const EXTS: &[&str] = &[
        ".zip", ".dmg", ".pkg", ".pdf", ".csv", ".xlsx", ".docx", ".mp4", ".tar.gz",
    ];
    EXTS.iter().any(|ext| path.ends_with(ext))
}

fn suggested_name_from_url(url: &Url) -> Option<String> {
    url.path_segments()
        .and_then(|mut s| s.rfind(|x| !x.is_empty()))
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

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
    navigation_filter: Rc<std::cell::RefCell<Option<NavigationFilter>>>,
    filter: std::rc::Rc<std::cell::RefCell<Option<RequestFilter>>>,
    frame_ready: Rc<std::cell::Cell<bool>>,
    framed: Rc<std::cell::RefCell<HashSet<WebViewId>>>,
}

impl WebViewDelegate for Delegate {
    fn notify_new_frame_ready(&self, _webview: WebView) {
        self.frame_ready.set(true);
        self.framed.borrow_mut().insert(self.id);
    }

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

    fn notify_media_session_event(&self, _webview: WebView, event: servo::MediaSessionEvent) {
        // Only playback state changes say whether the tab is audible.
        if let servo::MediaSessionEvent::PlaybackStateChange(state) = event {
            self.events.push(EngineEvent::Audible {
                id: self.id,
                playing: matches!(state, servo::MediaSessionPlaybackState::Playing),
            });
        }
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

    fn request_navigation(&self, _webview: WebView, request: NavigationRequest) {
        let url = request.url.clone();
        let allowed = self
            .navigation_filter
            .borrow()
            .as_ref()
            .map(|f| f(&url))
            .unwrap_or(true);
        if !allowed {
            request.deny();
            self.events
                .push(EngineEvent::NavigationDenied { id: self.id, url });
            return;
        }
        if is_download_url(&url) {
            request.deny();
            let suggested_name = suggested_name_from_url(&url);
            self.events.push(EngineEvent::Download {
                id: self.id,
                url,
                suggested_name,
            });
            return;
        }
        request.allow();
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

#[derive(Clone)]
struct PingState {
    last_reply: Instant,
    outstanding: bool,
    last_sent: Instant,
    hung_emitted: bool,
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
    navigation_filter: Rc<std::cell::RefCell<Option<NavigationFilter>>>,
    request_filter: std::rc::Rc<std::cell::RefCell<Option<RequestFilter>>>,
    hang_timeout: Duration,
    ping_state: HashMap<WebViewId, PingState>,
    surface_size: Option<PhysicalSize<u32>>,
    view_sizes: HashMap<WebViewId, PhysicalSize<u32>>,
    frame_ready: Rc<std::cell::Cell<bool>>,
    /// Web views that have produced at least one frame.
    framed: Rc<std::cell::RefCell<HashSet<WebViewId>>>,
    /// Web views to hide once they have produced their first frame.
    pending_hide: std::cell::RefCell<HashSet<WebViewId>>,
    /// Port of the remote debugging server started with Servo, if any.
    devtools_port: Option<u16>,
}

impl ServoEngine {
    /// Creates a new `ServoEngine` rendering into a child NSView of `parent`.
    pub fn new(
        parent: raw_window_handle::WindowHandle<'_>,
        waker: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Self> {
        Self::with_options(parent, waker, EngineOptions::default())
    }

    /// Like `new`, with Servo's remote debugging server listening on
    /// `127.0.0.1:<port>` when `devtools_port` is set. Servo 0.8 starts that
    /// server only at startup, so it cannot be toggled later.
    pub fn with_devtools(
        parent: raw_window_handle::WindowHandle<'_>,
        waker: Arc<dyn Fn() + Send + Sync>,
        devtools_port: Option<u16>,
    ) -> Result<Self> {
        let opts = EngineOptions {
            devtools_port,
            ..EngineOptions::default()
        };
        Self::with_options(parent, waker, opts)
    }

    /// Creates a `ServoEngine` with the given options (devtools port, JS GC
    /// profile, JIT toggle).
    pub fn with_options(
        parent: raw_window_handle::WindowHandle<'_>,
        waker: Arc<dyn Fn() + Send + Sync>,
        options: EngineOptions,
    ) -> Result<Self> {
        let devtools_port = options.devtools_port;
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
        // Start at the parent's real pixel size: WebRender sizes some
        // resources from the initial surface, and a 1x1 start paints blank.
        let initial_size = {
            let b = parent_view.bounds();
            let scale = parent_view
                .window()
                .map(|w| w.backingScaleFactor())
                .unwrap_or(2.0);
            PhysicalSize::new(
                ((b.size.width * scale).round() as u32).max(1),
                ((b.size.height * scale).round() as u32).max(1),
            )
        };
        let rendering_context = Rc::new(
            WindowRenderingContext::new(safe_dh, safe_wh, initial_size)
                .map_err(|e| anyhow::anyhow!("failed to create rendering context: {e:?}"))?,
        );
        let _ = rendering_context.make_current();

        let opts = Opts {
            multiprocess: true,
            ..Opts::default()
        };
        let mut prefs = servo::Preferences::default();
        if let Some(port) = devtools_port {
            prefs.devtools_server_enabled = true;
            prefs.devtools_server_listen_address = format!("127.0.0.1:{port}");
        }
        if options.js_gc_profile == "low" {
            prefs.js_mem_gc_compacting_enabled = true;
            prefs.js_mem_gc_empty_chunk_count_min = 0;
            prefs.js_mem_gc_high_frequency_high_limit_mb = 200;
            prefs.js_mem_gc_high_frequency_low_limit_mb = 50;
            prefs.js_mem_gc_high_frequency_heap_growth_max = 200;
            prefs.js_mem_gc_high_frequency_heap_growth_min = 120;
            prefs.js_mem_gc_low_frequency_heap_growth = 120;
        }
        if !options.js_jit {
            prefs.js_ion_enabled = false;
            prefs.js_baseline_jit_enabled = false;
        }
        let servo = ServoBuilder::default()
            .opts(opts)
            .preferences(prefs)
            .event_loop_waker(Box::new(WakerImpl(waker)))
            .build();
        servo.setup_logging();

        let now = Instant::now();
        Ok(Self {
            _servo: Some(servo),
            sys: System::new().context("process table")?,
            pids: HashMap::new(),
            awaiting_pid: VecDeque::new(),
            last_scan: now,
            lost_process: false,
            rendering_context,
            parent_view,
            child_view: child,
            views: HashMap::new(),
            next_id: 0,
            events: EventQueue::default(),
            shown: None,
            navigation_filter: Rc::new(std::cell::RefCell::new(None)),
            surface_size: None,
            devtools_port,
            view_sizes: HashMap::new(),
            frame_ready: Rc::new(std::cell::Cell::new(true)),
            framed: Rc::new(std::cell::RefCell::new(HashSet::new())),
            pending_hide: std::cell::RefCell::new(HashSet::new()),
            request_filter: std::rc::Rc::new(std::cell::RefCell::new(None)),
            hang_timeout: Duration::from_secs(5),
            ping_state: HashMap::new(),
        })
    }

    /// Content process id of each web view, as attributed by the child
    /// process watcher (see the type docs).
    pub fn content_pids(&self) -> Vec<(WebViewId, u32)> {
        let mut v: Vec<_> = self.pids.iter().map(|(pid, id)| (*id, *pid)).collect();
        v.sort();
        v
    }

    pub fn set_hang_timeout(&mut self, secs: u64) {
        self.hang_timeout = Duration::from_secs(secs.max(1));
    }

    fn pid_for_view(&self, id: WebViewId) -> Option<u32> {
        self.pids
            .iter()
            .find_map(|(pid, vid)| if *vid == id { Some(*pid) } else { None })
    }

    fn check_hangs(&mut self) {
        let now = Instant::now();
        let ids: Vec<WebViewId> = self.views.keys().copied().collect();
        for id in ids {
            let state = self.ping_state.entry(id).or_insert_with(|| PingState {
                last_reply: now,
                outstanding: false,
                last_sent: now - Duration::from_secs(10),
                hung_emitted: false,
            });
            if state.outstanding {
                if !state.hung_emitted && now.duration_since(state.last_reply) > self.hang_timeout {
                    self.events.push(EngineEvent::Hung { id });
                    state.hung_emitted = true;
                }
            } else if now.duration_since(state.last_sent) >= Duration::from_secs(1)
                && let Some(entry) = self.views.get(&id)
            {
                let events = self.events.clone();
                entry.webview.evaluate_javascript("0", move |_| {
                    events.push(EngineEvent::Responsive { id });
                });
                let st = self.ping_state.entry(id).or_insert_with(|| PingState {
                    last_reply: now,
                    outstanding: false,
                    last_sent: now,
                    hung_emitted: false,
                });
                st.outstanding = true;
                st.last_sent = now;
            }
        }
    }

    /// Shuts Servo down (content processes exit), then kills any child that
    /// is still alive. Call before quitting; `Drop` calls it too.
    pub fn shutdown(&mut self) {
        self.views.clear();
        self.shown = None;
        self.ping_state.clear();
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

impl ServoEngine {
    /// Hides a web view, or defers the hide until its first frame (Servo
    /// 0.8 never paints a view hidden before that).
    fn hide_view(&self, id: WebViewId) {
        if !self.framed.borrow().contains(&id) {
            self.pending_hide.borrow_mut().insert(id);
            return;
        }
        if let Some(entry) = self.views.get(&id) {
            entry.webview.hide();
        }
    }

    /// Applies hides deferred by `hide_view` for views that now have a frame.
    fn apply_pending_hides(&self) {
        let ready: Vec<WebViewId> = {
            let framed = self.framed.borrow();
            self.pending_hide
                .borrow()
                .iter()
                .copied()
                .filter(|id| framed.contains(id))
                .collect()
        };
        for id in ready {
            self.pending_hide.borrow_mut().remove(&id);
            if self.shown != Some(id)
                && let Some(entry) = self.views.get(&id)
            {
                entry.webview.hide();
            }
        }
    }
}

impl Engine for ServoEngine {
    fn create(&mut self, url: &Url) -> WebViewId {
        let id = self.alloc_id();
        let delegate = Rc::new(Delegate {
            id,
            events: self.events.clone(),
            navigation_filter: self.navigation_filter.clone(),
            filter: self.request_filter.clone(),
            frame_ready: self.frame_ready.clone(),
            framed: self.framed.clone(),
        });
        let webview = WebViewBuilder::new(self.servo(), self.rendering_context.clone())
            .url(url.clone())
            .delegate(delegate)
            .build();
        // Hide unless it becomes the shown view.
        // Servo 0.8 never paints a web view that was hidden before its first
        // frame, so the hide waits for that frame (see `spin`).
        self.pending_hide.borrow_mut().insert(id);
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
        self.ping_state.remove(&id);
        self.framed.borrow_mut().remove(&id);
        self.pending_hide.borrow_mut().remove(&id);
        self.view_sizes.remove(&id);
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
            && self.views.contains_key(&current)
        {
            self.hide_view(current);
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
        // Resizing recreates the surfaces, so only do it when the size
        // changes; doing it every tick presents blank surfaces.
        let resized = self.surface_size != Some(size);
        if resized {
            self.rendering_context.resize(size);
            self.surface_size = Some(size);
            self.frame_ready.set(true);
        }

        if let Some(entry) = self.views.get(&id) {
            if resized || self.shown != Some(id) || self.view_sizes.get(&id) != Some(&size) {
                entry
                    .webview
                    .set_hidpi_scale_factor(euclid::Scale::new(scale));
                entry.webview.resize(size);
                self.view_sizes.insert(id, size);
            }
            self.pending_hide.borrow_mut().remove(&id);
            if self.shown != Some(id) {
                entry.webview.show();
                self.frame_ready.set(true);
            }
            self.shown = Some(id);
        }
    }

    fn hide(&mut self, id: WebViewId) {
        self.hide_view(id);
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

    fn set_navigation_filter(&mut self, f: NavigationFilter) {
        *self.navigation_filter.borrow_mut() = Some(f);
    }

    fn spin(&mut self) {
        if self._servo.is_none() {
            return;
        }
        self.servo().spin_event_loop();
        self.watch_children();
        self.apply_pending_hides();
        self.check_hangs();
        // Paint and present only when Servo has a new frame (or after a
        // resize or tab switch); presenting without painting swaps in a
        // stale or blank buffer.
        if self.frame_ready.replace(false)
            && let Some(id) = self.shown
            && let Some(entry) = self.views.get(&id)
        {
            entry.webview.paint();
            self.rendering_context.present();
        }
    }

    fn set_request_filter(&mut self, f: RequestFilter) {
        *self.request_filter.borrow_mut() = Some(f);
    }

    fn kill(&mut self, id: WebViewId) {
        if let Some(pid) = self.pid_for_view(id) {
            let _ = std::process::Command::new("kill")
                .arg("-9")
                .arg(pid.to_string())
                .status();
        }
    }

    fn processes(&mut self) -> Vec<ProcessInfo> {
        self.sys.refresh_processes(ProcessesToUpdate::All, true);
        let mut out = Vec::new();
        let me = Pid::from_u32(std::process::id());
        if let Some(p) = self.sys.process(me) {
            out.push(ProcessInfo {
                id: None,
                pid: std::process::id(),
                rss_bytes: p.memory(),
                cpu_percent: p.cpu_usage(),
            });
        } else {
            out.push(ProcessInfo {
                id: None,
                pid: std::process::id(),
                rss_bytes: 0,
                cpu_percent: 0.0,
            });
        }
        let pairs = self.content_pids();
        for (id, pid) in pairs {
            if let Some(p) = self.sys.process(Pid::from_u32(pid)) {
                out.push(ProcessInfo {
                    id: Some(id),
                    pid,
                    rss_bytes: p.memory(),
                    cpu_percent: p.cpu_usage(),
                });
            }
        }
        out
    }

    fn set_devtools(&mut self, port: Option<u16>) -> anyhow::Result<()> {
        if port == self.devtools_port {
            Ok(())
        } else {
            anyhow::bail!("Servo starts or stops DevTools only at launch")
        }
    }

    fn memory_report(&mut self) -> Vec<MemoryRow> {
        use servo::profile_traits::mem::MemoryReportResult;
        use servo_base::generic_channel::GenericCallback;
        let report: std::sync::Arc<std::sync::Mutex<Option<MemoryReportResult>>> =
            std::sync::Arc::default();
        let cb_report = report.clone();
        let callback = match GenericCallback::new(move |result| {
            if let Ok(r) = result {
                *cb_report.lock().expect("memory report lock") = Some(r);
            }
        }) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        self.servo().create_memory_report(callback);

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while report.lock().expect("memory report lock").is_none()
            && std::time::Instant::now() < deadline
        {
            self.spin();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let result = report.lock().expect("memory report lock").take();
        let mut rows = Vec::new();
        if let Some(r) = result {
            for proc in r.results {
                let process = if proc.pid == std::process::id() {
                    None
                } else {
                    self.pids
                        .iter()
                        .find_map(|(pid, id)| if *pid == proc.pid { Some(*id) } else { None })
                };
                for rep in proc.reports {
                    rows.push(MemoryRow {
                        process,
                        path: rep.path.join("/"),
                        bytes: rep.size as u64,
                    });
                }
            }
        }
        rows
    }

    fn drain_events(&mut self) -> Vec<EngineEvent> {
        let evs = self.events.drain();
        let now = Instant::now();
        // Ping replies arrive as `Responsive`; only a reply after `Hung` is
        // reported, the rest just reset the watchdog.
        let mut out = Vec::with_capacity(evs.len());
        for ev in evs {
            if let EngineEvent::Responsive { id } = ev {
                let was_hung = match self.ping_state.get_mut(&id) {
                    Some(st) => {
                        let was = st.hung_emitted;
                        st.outstanding = false;
                        st.last_reply = now;
                        st.hung_emitted = false;
                        was
                    }
                    None => false,
                };
                if !was_hung {
                    continue;
                }
            }
            out.push(ev);
        }
        out
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
