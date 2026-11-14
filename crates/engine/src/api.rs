//! Public engine API: ids, events, view geometry and the `Engine` trait.

use url::Url;

pub use adblock_rules::RequestKind;

/// Closure used by [`Engine::set_request_filter`].
pub type RequestFilter = Box<dyn Fn(&Url, &Url, RequestKind) -> bool>;

/// Reply for [`Engine::evaluate`].
pub type EvalReply = Box<dyn FnOnce(Result<serde_json::Value, String>)>;

/// Reply for [`Engine::screenshot`], returning PNG bytes.
pub type ScreenshotReply = Box<dyn FnOnce(Result<Vec<u8>, String>)>;

/// Console message severity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsoleLevel {
    Log,
    Info,
    Warn,
    Error,
    Debug,
}

/// Unique id for a web view managed by an `Engine`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub struct WebViewId(pub u64);

/// State of a page load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadState {
    Started,
    Complete,
    Failed(String),
}

/// Events emitted by an engine and drained by the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineEvent {
    UrlChanged {
        id: WebViewId,
        url: Url,
    },
    TitleChanged {
        id: WebViewId,
        title: String,
    },
    Load {
        id: WebViewId,
        state: LoadState,
    },
    History {
        id: WebViewId,
        can_back: bool,
        can_forward: bool,
    },
    Crashed {
        id: WebViewId,
        reason: String,
    },
    OpenTab {
        opener: WebViewId,
        url: Url,
    },
    Audible {
        id: WebViewId,
        playing: bool,
    },
    NavigationDenied {
        id: WebViewId,
        url: Url,
    },
    Download {
        id: WebViewId,
        url: Url,
        suggested_name: Option<String>,
    },
    Hung {
        id: WebViewId,
    },
    Responsive {
        id: WebViewId,
    },
    Console {
        id: WebViewId,
        level: ConsoleLevel,
        message: String,
    },
    Request {
        id: WebViewId,
        request_id: u64,
        url: Url,
        method: String,
        is_document: bool,
    },
}

/// Information about a browser or content process.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessInfo {
    pub id: Option<WebViewId>,
    pub pid: u32,
    pub rss_bytes: u64,
    pub cpu_percent: f32,
}

/// One row of a Servo memory report.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MemoryRow {
    pub process: Option<WebViewId>,
    pub path: String,
    pub bytes: u64,
}

/// Options passed to [`ServoEngine::with_options`](crate::servo_engine::ServoEngine::with_options).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineOptions {
    pub devtools_port: Option<u16>,
    pub js_gc_profile: String,
    pub js_jit: bool,
}

impl Default for EngineOptions {
    fn default() -> Self {
        Self {
            devtools_port: None,
            js_gc_profile: "low".to_string(),
            js_jit: true,
        }
    }
}

/// Filter for navigations: return `false` to deny.
pub type NavigationFilter = Box<dyn Fn(&Url) -> bool>;

/// Rectangle in points for showing a web view.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Mouse button used for page input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Keyboard modifiers for page input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub cmd: bool,
}

/// Input forwarded from the UI to the engine.
///
/// Coordinates are in points relative to the shown view's top-left.
/// `dx`/`dy` are in points. `key` is the DOM key value, `code` the DOM code.
#[derive(Debug, Clone, PartialEq)]
pub enum EngineInput {
    MouseMove {
        x: f32,
        y: f32,
    },
    MouseDown {
        button: MouseButton,
        x: f32,
        y: f32,
    },
    MouseUp {
        button: MouseButton,
        x: f32,
        y: f32,
    },
    Wheel {
        dx: f32,
        dy: f32,
        x: f32,
        y: f32,
    },
    Key {
        down: bool,
        key: String,
        code: String,
        modifiers: Modifiers,
    },
    MouseLeft,
}

/// Abstract browser engine.
pub trait Engine {
    fn create(&mut self, url: &Url) -> WebViewId;
    fn close(&mut self, id: WebViewId);
    fn load(&mut self, id: WebViewId, url: &Url);
    fn back(&mut self, id: WebViewId);
    fn forward(&mut self, id: WebViewId);
    fn reload(&mut self, id: WebViewId);
    fn show(&mut self, id: WebViewId, rect: ViewRect);
    fn hide(&mut self, id: WebViewId);
    fn input(&mut self, _id: WebViewId, _ev: EngineInput) {}
    fn run_script(&mut self, _id: WebViewId, _js: &str) {}
    fn evaluate(&mut self, _id: WebViewId, _js: &str, reply: EvalReply) {
        reply(Err("unsupported".to_string()));
    }
    fn screenshot(&mut self, _id: WebViewId, reply: ScreenshotReply) {
        reply(Err("unsupported".to_string()));
    }
    fn set_navigation_filter(&mut self, _f: NavigationFilter) {}
    fn spin(&mut self);
    fn drain_events(&mut self) -> Vec<EngineEvent>;
    fn set_request_filter(&mut self, _f: RequestFilter) {}
    fn kill(&mut self, _id: WebViewId) {}
    fn processes(&mut self) -> Vec<ProcessInfo> {
        Vec::new()
    }
    fn memory_report(&mut self) -> Vec<MemoryRow> {
        Vec::new()
    }
    /// Requests memory minimization as far as the engine allows (GC, cache
    /// purges). Default no-op.
    fn minimize_memory(&mut self) {}
    fn set_devtools(&mut self, _port: Option<u16>) -> anyhow::Result<()> {
        Ok(())
    }
    fn shutdown(&mut self) {}
}
