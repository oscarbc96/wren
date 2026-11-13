//! Tab strip model for Wren.

use engine::{Engine, EngineEvent, WebViewId};
use store::{SessionSnapshot, SessionTab};
use url::Url;

/// Unique id for a tab. Never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct TabId(pub u64);

/// One tab in the strip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tab {
    pub id: TabId,
    pub webview: WebViewId,
    pub url: Url,
    pub title: String,
    pub loading: bool,
    pub can_back: bool,
    pub can_forward: bool,
    pub crashed: Option<String>,
}

#[derive(Debug, Clone)]
struct ClosedTab {
    url: Url,
    index: usize,
}

/// Tab strip model.
pub struct TabStrip {
    tabs: Vec<Tab>,
    active: Option<usize>,
    next_tab_id: u64,
    closed_stack: usize,
    closed: Vec<ClosedTab>,
    changed_flag: bool,
}

impl TabStrip {
    pub fn new(closed_stack: usize) -> Self {
        Self {
            tabs: Vec::new(),
            active: None,
            next_tab_id: 1,
            closed_stack,
            closed: Vec::new(),
            changed_flag: false,
        }
    }

    fn next_tab_id(&mut self) -> TabId {
        let id = TabId(self.next_tab_id);
        self.next_tab_id += 1;
        id
    }

    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    pub fn active(&self) -> Option<TabId> {
        self.active.and_then(|i| self.tabs.get(i).map(|t| t.id))
    }

    pub fn open(&mut self, e: &mut dyn Engine, url: &Url, activate: bool) -> TabId {
        let webview = e.create(url);
        let id = self.next_tab_id();
        let tab = Tab {
            id,
            webview,
            url: url.clone(),
            title: String::new(),
            loading: false,
            can_back: false,
            can_forward: false,
            crashed: None,
        };
        self.tabs.push(tab);
        let idx = self.tabs.len() - 1;
        if activate {
            self.active = Some(idx);
        }
        self.changed_flag = true;
        id
    }

    pub fn close(&mut self, e: &mut dyn Engine, id: TabId) {
        let idx = match self.tabs.iter().position(|t| t.id == id) {
            Some(i) => i,
            None => return,
        };
        let tab = self.tabs.remove(idx);
        e.close(tab.webview);
        // record closed
        if self.closed_stack > 0 {
            self.closed.push(ClosedTab {
                url: tab.url.clone(),
                index: idx,
            });
            if self.closed.len() > self.closed_stack {
                // keep most recent N
                let drop = self.closed.len() - self.closed_stack;
                self.closed.drain(0..drop);
            }
        }
        // adjust active
        match self.active {
            Some(a) if a == idx => {
                if idx < self.tabs.len() {
                    self.active = Some(idx);
                } else if !self.tabs.is_empty() {
                    self.active = Some(self.tabs.len() - 1);
                } else {
                    self.active = None;
                }
            }
            Some(a) if a > idx => {
                self.active = Some(a - 1);
            }
            other => {
                self.active = other;
            }
        }
        self.changed_flag = true;
    }

    pub fn activate(&mut self, id: TabId) {
        if let Some(idx) = self.tabs.iter().position(|t| t.id == id)
            && self.active != Some(idx)
        {
            self.active = Some(idx);
            self.changed_flag = true;
        }
    }

    pub fn move_to(&mut self, id: TabId, index: usize) {
        let from = match self.tabs.iter().position(|t| t.id == id) {
            Some(i) => i,
            None => return,
        };
        if self.tabs.is_empty() {
            return;
        }
        let mut to = index;
        if to >= self.tabs.len() {
            to = self.tabs.len() - 1;
        }
        if from == to {
            return;
        }
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);
        // update active
        if let Some(a) = self.active {
            let new_active = if a == from {
                to
            } else if from < a && to >= a {
                a - 1
            } else if from > a && to <= a {
                a + 1
            } else {
                a
            };
            self.active = Some(new_active);
        }
        self.changed_flag = true;
    }

    pub fn reopen_closed(&mut self, e: &mut dyn Engine) -> Option<TabId> {
        let entry = self.closed.pop()?;
        let webview = e.create(&entry.url);
        let id = self.next_tab_id();
        let tab = Tab {
            id,
            webview,
            url: entry.url.clone(),
            title: String::new(),
            loading: false,
            can_back: false,
            can_forward: false,
            crashed: None,
        };
        let mut idx = entry.index;
        if idx > self.tabs.len() {
            idx = self.tabs.len();
        }
        self.tabs.insert(idx, tab);
        self.active = Some(idx);
        self.changed_flag = true;
        Some(id)
    }

    pub fn reload_crashed(&mut self, e: &mut dyn Engine, id: TabId) {
        let idx = match self.tabs.iter().position(|t| t.id == id) {
            Some(i) => i,
            None => return,
        };
        if self.tabs[idx].crashed.is_none() {
            return;
        }
        self.tabs[idx].crashed = None;
        let old = self.tabs[idx].webview;
        e.close(old);
        let new_wv = e.create(&self.tabs[idx].url);
        self.tabs[idx].webview = new_wv;
        self.tabs[idx].loading = true;
        self.changed_flag = true;
    }

    /// Applies an engine event. `OpenTab` creates a real webview through `e`.
    pub fn apply(&mut self, e: &mut dyn Engine, ev: &EngineEvent) {
        match ev {
            EngineEvent::UrlChanged { id, url } => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.webview == *id) {
                    tab.url = url.clone();
                    self.changed_flag = true;
                }
            }
            EngineEvent::TitleChanged { id, title } => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.webview == *id) {
                    tab.title = title.clone();
                    self.changed_flag = true;
                }
            }
            EngineEvent::Load { id, state } => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.webview == *id) {
                    match state {
                        engine::LoadState::Started => tab.loading = true,
                        engine::LoadState::Complete | engine::LoadState::Failed(_) => {
                            tab.loading = false
                        }
                    }
                    self.changed_flag = true;
                }
            }
            EngineEvent::History {
                id,
                can_back,
                can_forward,
            } => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.webview == *id) {
                    tab.can_back = *can_back;
                    tab.can_forward = *can_forward;
                    self.changed_flag = true;
                }
            }
            EngineEvent::Crashed { id, reason } => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.webview == *id) {
                    tab.crashed = Some(reason.clone());
                    tab.loading = false;
                    self.changed_flag = true;
                }
            }
            EngineEvent::OpenTab { opener, url } => {
                // find opener tab
                let pos = self
                    .tabs
                    .iter()
                    .position(|t| t.webview == *opener)
                    .map(|i| i + 1);
                let insert_at = pos.unwrap_or(self.tabs.len());
                let webview = e.create(url);
                let id = self.next_tab_id();
                let tab = Tab {
                    id,
                    webview,
                    url: url.clone(),
                    title: String::new(),
                    loading: false,
                    can_back: false,
                    can_forward: false,
                    crashed: None,
                };
                if insert_at >= self.tabs.len() {
                    self.tabs.push(tab);
                } else {
                    self.tabs.insert(insert_at, tab);
                }
                self.changed_flag = true;
            }
        }
    }

    pub fn snapshot(&self) -> SessionSnapshot {
        SessionSnapshot {
            tabs: self
                .tabs
                .iter()
                .map(|t| SessionTab {
                    url: t.url.clone(),
                    title: t.title.clone(),
                })
                .collect(),
            active: self.active,
        }
    }

    pub fn restore(e: &mut dyn Engine, s: &SessionSnapshot, closed_stack: usize) -> TabStrip {
        let mut strip = TabStrip::new(closed_stack);
        for t in &s.tabs {
            let webview = e.create(&t.url);
            let id = strip.next_tab_id();
            strip.tabs.push(Tab {
                id,
                webview,
                url: t.url.clone(),
                title: t.title.clone(),
                loading: false,
                can_back: false,
                can_forward: false,
                crashed: None,
            });
        }
        if let Some(i) = s.active
            && i < strip.tabs.len()
        {
            strip.active = Some(i);
        }
        strip.changed_flag = false;
        strip
    }

    pub fn changed(&mut self) -> bool {
        let v = self.changed_flag;
        self.changed_flag = false;
        v
    }
}
