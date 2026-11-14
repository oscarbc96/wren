//! In-memory `Engine` implementation used by tests.

use crate::api::{
    ConsoleLevel, Engine, EngineEvent, EngineInput, EvalReply, MemoryRow, NavigationFilter,
    ProcessInfo, RequestFilter, ScreenshotReply, ViewRect, WebViewId,
};
use image::{ImageBuffer, Rgba};
use std::collections::HashMap;
use std::io::Cursor;
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
    Evaluate { id: WebViewId, js: String },
    Screenshot { id: WebViewId },
    SetNavigationFilter,
    Spin,
    SetRequestFilter,
    Kill { id: WebViewId },
    Processes,
    MemoryReport,
    MinimizeMemory,
    SetDevtools { port: Option<u16> },
    DrainEvents,
}

/// Fake engine recording calls and replaying pushed events.
pub struct FakeEngine {
    next_id: u64,
    pending: Vec<EngineEvent>,
    calls: Vec<FakeCall>,
    navigation_filter: Option<NavigationFilter>,
    process_info: Vec<ProcessInfo>,
    memory_report: Vec<MemoryRow>,
    eval_results: HashMap<WebViewId, Result<serde_json::Value, String>>,
    pending_eval_replies: HashMap<WebViewId, Vec<EvalReply>>,
    pending_screenshot_replies: HashMap<WebViewId, Vec<ScreenshotReply>>,
}

impl Default for FakeEngine {
    fn default() -> Self {
        Self {
            next_id: 0,
            pending: Vec::new(),
            calls: Vec::new(),
            navigation_filter: None,
            process_info: Vec::new(),
            memory_report: vec![MemoryRow {
                process: None,
                path: "js-heap".to_string(),
                bytes: 1024,
            }],
            eval_results: HashMap::new(),
            pending_eval_replies: HashMap::new(),
            pending_screenshot_replies: HashMap::new(),
        }
    }
}

impl FakeEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, ev: EngineEvent) {
        if let EngineEvent::Crashed { id, .. } = ev {
            self.fail_pending(id);
        }
        self.pending.push(ev);
    }

    pub fn push_console(&mut self, id: WebViewId, level: ConsoleLevel, message: String) {
        self.pending
            .push(EngineEvent::Console { id, level, message });
    }

    pub fn set_eval_result(&mut self, id: WebViewId, result: Result<serde_json::Value, String>) {
        self.eval_results.insert(id, result);
    }

    pub fn calls(&self) -> &[FakeCall] {
        &self.calls
    }

    pub fn set_processes(&mut self, info: Vec<ProcessInfo>) {
        self.process_info = info;
    }

    pub fn set_memory_report(&mut self, rows: Vec<MemoryRow>) {
        self.memory_report = rows;
    }

    fn fail_pending(&mut self, id: WebViewId) {
        if let Some(replies) = self.pending_eval_replies.remove(&id) {
            for reply in replies {
                reply(Err("target crashed".to_string()));
            }
        }
        if let Some(replies) = self.pending_screenshot_replies.remove(&id) {
            for reply in replies {
                reply(Err("target crashed".to_string()));
            }
        }
    }

    fn one_by_one_png() -> Vec<u8> {
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(1, 1, Rgba([255, 255, 255, 255]));
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::Png)
            .expect("png encode");
        buf.into_inner()
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
        self.fail_pending(id);
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

    fn evaluate(&mut self, id: WebViewId, js: &str, reply: EvalReply) {
        self.calls.push(FakeCall::Evaluate {
            id,
            js: js.to_string(),
        });
        if let Some(result) = self.eval_results.remove(&id) {
            reply(result);
        } else {
            self.pending_eval_replies.entry(id).or_default().push(reply);
        }
    }

    fn screenshot(&mut self, id: WebViewId, reply: ScreenshotReply) {
        self.calls.push(FakeCall::Screenshot { id });
        reply(Ok(Self::one_by_one_png()));
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

    fn memory_report(&mut self) -> Vec<MemoryRow> {
        self.calls.push(FakeCall::MemoryReport);
        if self.memory_report.is_empty() {
            vec![MemoryRow {
                process: None,
                path: "js-heap".to_string(),
                bytes: 1024,
            }]
        } else {
            self.memory_report.clone()
        }
    }

    fn minimize_memory(&mut self) {
        self.calls.push(FakeCall::MinimizeMemory);
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
