//! Public engine API: ids, events, view geometry and the `Engine` trait.

use url::Url;

/// Unique id for a web view managed by an `Engine`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
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
}

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
    fn spin(&mut self);
    fn drain_events(&mut self) -> Vec<EngineEvent>;
    fn shutdown(&mut self) {}
}
