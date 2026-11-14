//! In-memory `Engine` implementation used by tests.

use crate::api::{
    Engine, EngineEvent, EngineInput, NavigationFilter, ProcessInfo, RequestFilter, ViewRect,
    WebViewId,
};
use url::Url;

/// One recorded call made to a `FakeEngine`.
#[derive(Debug, Clone, PartialEq)]
pub enum FakeCall {
    Create { url: Url },
    Close { id: WebViewId },
    Load { id: WebViewId, url: Url },
    Back { id: WebViewId },
    Forward { id: WebViewId },
    Reload { id: WebViewId },
    Show { id: WebViewId, rect: ViewRect },
    Hide { id: WebViewId },
    Input(WebViewId, EngineInput),
    RunScript { id: WebViewId, js: String },
    SetNavigationFilter,
    Spin,
    SetRequestFilter,
    Kill { id: WebViewId },
    Processes,
    SetDevtools { port: Option<u16> },
    DrainEvents,
}

/// Fake engine recording calls and replaying pushed events.
#[derive(Default)]
pub struct FakeEngine {
    next_id: u64,
    pending: Vec<EngineEvent>,
    calls: Vec<FakeCall>,
    navigation_filter: Option<NavigationFilter>,
    process_info: Vec<ProcessInfo>,
}

impl FakeEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, ev: EngineEvent) {
        self.pending.push(ev);
    }

    pub fn calls(&self) -> &[FakeCall] {
        &self.calls
    }

    pub fn set_processes(&mut self, info: Vec<ProcessInfo>) {
        self.process_info = info;
    }
}

impl Engine for FakeEngine {
    fn create(&mut self, url: &Url) -> WebViewId {
        let id = WebViewId(self.next_id);
        self.next_id += 1;
        self.calls.push(FakeCall::Create { url: url.clone() });
        id
    }

    fn close(&mut self, id: WebViewId) {
        self.calls.push(FakeCall::Close { id });
    }

    fn load(&mut self, id: WebViewId, url: &Url) {
        self.calls.push(FakeCall::Load {
            id,
            url: url.clone(),
        });
    }

    fn back(&mut self, id: WebViewId) {
        self.calls.push(FakeCall::Back { id });
    }

    fn forward(&mut self, id: WebViewId) {
        self.calls.push(FakeCall::Forward { id });
    }

    fn reload(&mut self, id: WebViewId) {
        self.calls.push(FakeCall::Reload { id });
    }

    fn show(&mut self, id: WebViewId, rect: ViewRect) {
        self.calls.push(FakeCall::Show { id, rect });
    }

    fn hide(&mut self, id: WebViewId) {
        self.calls.push(FakeCall::Hide { id });
    }

    fn input(&mut self, id: WebViewId, ev: EngineInput) {
        self.calls.push(FakeCall::Input(id, ev));
    }

    fn run_script(&mut self, id: WebViewId, js: &str) {
        self.calls.push(FakeCall::RunScript {
            id,
            js: js.to_string(),
        });
    }

    fn set_navigation_filter(&mut self, f: NavigationFilter) {
        self.navigation_filter = Some(f);
        self.calls.push(FakeCall::SetNavigationFilter);
    }

    fn spin(&mut self) {
        self.calls.push(FakeCall::Spin);
    }

    fn set_request_filter(&mut self, _f: RequestFilter) {
        self.calls.push(FakeCall::SetRequestFilter);
    }

    fn kill(&mut self, id: WebViewId) {
        self.calls.push(FakeCall::Kill { id });
    }

    fn processes(&mut self) -> Vec<ProcessInfo> {
        self.calls.push(FakeCall::Processes);
        self.process_info.clone()
    }

    fn set_devtools(&mut self, port: Option<u16>) -> anyhow::Result<()> {
        self.calls.push(FakeCall::SetDevtools { port });
        Ok(())
    }

    fn drain_events(&mut self) -> Vec<EngineEvent> {
        self.calls.push(FakeCall::DrainEvents);
        std::mem::take(&mut self.pending)
    }
}
