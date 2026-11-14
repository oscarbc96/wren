//! Tab strip model for Wren.

use engine::{Engine, EngineEvent, WebViewId};
use jiff::{SignedDuration, Timestamp};
use store::{SessionSnapshot, SessionTab};
use url::Url;

/// Unique id for a tab. Never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct TabId(pub u64);

/// Tab lifecycle state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabState {
    Active,
    Background,
    Slept,
}

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
    pub hung: bool,
    pub state: TabState,
    pub pinned: bool,
    pub audible: bool,
    pub last_active: Timestamp,
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

    pub fn tabs_mut(&mut self) -> &mut [Tab] {
        &mut self.tabs
    }

    pub fn active(&self) -> Option<TabId> {
        self.active.and_then(|i| self.tabs.get(i).map(|t| t.id))
    }

    pub fn open(&mut self, e: &mut dyn Engine, url: &Url, activate: bool) -> TabId {
        let webview = e.create(url);
        let id = self.next_tab_id();
        let now = Timestamp::now();
        let state = if activate {
            TabState::Active
        } else {
            TabState::Background
        };
        let tab = Tab {
            id,
            webview,
            url: url.clone(),
            title: String::new(),
            // Servo sends no Load Started for a web view's first load.
            loading: true,
            can_back: false,
            can_forward: false,
            crashed: None,
            hung: false,
            state,
            pinned: false,
            audible: false,
            last_active: now,
        };
        self.tabs.push(tab);
        let idx = self.tabs.len() - 1;
        if activate {
            if let Some(old) = self.active
                && let Some(t) = self.tabs.get_mut(old)
                && t.state == TabState::Active
            {
                t.state = TabState::Background;
                t.last_active = now;
            }
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
        if tab.state != TabState::Slept {
            e.close(tab.webview);
        }
        if self.closed_stack > 0 {
            self.closed.push(ClosedTab {
                url: tab.url.clone(),
                index: idx,
            });
            if self.closed.len() > self.closed_stack {
                let drop = self.closed.len() - self.closed_stack;
                self.closed.drain(0..drop);
            }
        }
        match self.active {
            Some(a) if a == idx => {
                if idx < self.tabs.len() {
                    self.active = Some(idx);
                    if let Some(t) = self.tabs.get_mut(idx)
                        && t.state == TabState::Background
                    {
                        t.state = TabState::Active;
                        t.last_active = Timestamp::now();
                    }
                } else if !self.tabs.is_empty() {
                    let i = self.tabs.len() - 1;
                    self.active = Some(i);
                    if let Some(t) = self.tabs.get_mut(i)
                        && t.state == TabState::Background
                    {
                        t.state = TabState::Active;
                        t.last_active = Timestamp::now();
                    }
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
        if let Some(idx) = self.tabs.iter().position(|t| t.id == id) {
            if self.active == Some(idx) {
                return;
            }
            let now = Timestamp::now();
            if let Some(old) = self.active
                && let Some(t) = self.tabs.get_mut(old)
                && t.state == TabState::Active
            {
                t.state = TabState::Background;
                t.last_active = now;
            }
            self.active = Some(idx);
            if let Some(t) = self.tabs.get_mut(idx)
                && t.state == TabState::Background
            {
                t.state = TabState::Active;
                t.last_active = now;
            }
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
        let now = Timestamp::now();
        if let Some(old) = self.active
            && let Some(t) = self.tabs.get_mut(old)
            && t.state == TabState::Active
        {
            t.state = TabState::Background;
            t.last_active = now;
        }
        let tab = Tab {
            id,
            webview,
            url: entry.url.clone(),
            title: String::new(),
            loading: false,
            can_back: false,
            can_forward: false,
            crashed: None,
            hung: false,
            state: TabState::Active,
            pinned: false,
            audible: false,
            last_active: now,
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
        if self.tabs[idx].state == TabState::Slept {
            // waking via reload is fine
            let new_wv = e.create(&self.tabs[idx].url);
            self.tabs[idx].webview = new_wv;
            self.tabs[idx].state = TabState::Background;
        } else {
            let old = self.tabs[idx].webview;
            e.close(old);
            let new_wv = e.create(&self.tabs[idx].url);
            self.tabs[idx].webview = new_wv;
        }
        self.tabs[idx].loading = true;
        self.changed_flag = true;
    }

    pub fn sleep(&mut self, e: &mut dyn Engine, id: TabId) {
        let Some(idx) = self.tabs.iter().position(|t| t.id == id) else {
            return;
        };
        if self.tabs[idx].state == TabState::Slept {
            return;
        }
        e.close(self.tabs[idx].webview);
        self.tabs[idx].state = TabState::Slept;
        self.tabs[idx].loading = false;
        self.tabs[idx].audible = false;
        if self.active == Some(idx) {
            // sleeping active is unusual, but keep active index; state becomes slept
        }
        self.changed_flag = true;
    }

    pub fn wake(&mut self, e: &mut dyn Engine, id: TabId) {
        let Some(idx) = self.tabs.iter().position(|t| t.id == id) else {
            return;
        };
        if self.tabs[idx].state != TabState::Slept {
            self.activate(id);
            return;
        }
        let webview = e.create(&self.tabs[idx].url);
        self.tabs[idx].webview = webview;
        self.tabs[idx].loading = true;
        let now = Timestamp::now();
        if let Some(old) = self.active
            && old != idx
            && let Some(t) = self.tabs.get_mut(old)
            && t.state == TabState::Active
        {
            t.state = TabState::Background;
            t.last_active = now;
        }
        self.active = Some(idx);
        self.tabs[idx].state = TabState::Active;
        self.tabs[idx].last_active = now;
        self.changed_flag = true;
    }

    pub fn sleep_candidates(&self, now: Timestamp, after: SignedDuration) -> Vec<TabId> {
        let mut candidates: Vec<(Timestamp, TabId)> = self
            .tabs
            .iter()
            .enumerate()
            .filter_map(|(i, t)| {
                if self.active == Some(i) {
                    return None;
                }
                if t.pinned || t.audible {
                    return None;
                }
                if t.state == TabState::Slept {
                    return None;
                }
                if now.duration_since(t.last_active) >= after {
                    Some((t.last_active, t.id))
                } else {
                    None
                }
            })
            .collect();
        candidates.sort_by_key(|(ts, _)| *ts);
        candidates.into_iter().map(|(_, id)| id).collect()
    }

    pub fn sleep_until_under_budget(
        &mut self,
        e: &mut dyn Engine,
        budget_bytes: u64,
        rss: &impl Fn(TabId) -> u64,
    ) {
        loop {
            let total: u64 = self
                .tabs
                .iter()
                .filter(|t| t.state != TabState::Slept)
                .map(|t| rss(t.id))
                .sum();
            if total <= budget_bytes {
                break;
            }
            // sleep candidates in order (oldest first). include all background non-pinned non-audible.
            let mut candidates: Vec<(Timestamp, TabId)> = self
                .tabs
                .iter()
                .enumerate()
                .filter_map(|(i, t)| {
                    if self.active == Some(i) {
                        return None;
                    }
                    if t.pinned || t.audible || t.state == TabState::Slept {
                        return None;
                    }
                    Some((t.last_active, t.id))
                })
                .collect();
            candidates.sort_by_key(|(ts, _)| *ts);
            let Some((_, id)) = candidates.first().copied() else {
                break;
            };
            self.sleep(e, id);
        }
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
                    tab.hung = false;
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
                    tab.hung = false;
                    tab.loading = false;
                    self.changed_flag = true;
                }
            }
            EngineEvent::OpenTab { opener, url } => {
                let pos = self
                    .tabs
                    .iter()
                    .position(|t| t.webview == *opener)
                    .map(|i| i + 1);
                let insert_at = pos.unwrap_or(self.tabs.len());
                let webview = e.create(url);
                let id = self.next_tab_id();
                let now = Timestamp::now();
                let tab = Tab {
                    id,
                    webview,
                    url: url.clone(),
                    title: String::new(),
                    loading: false,
                    can_back: false,
                    can_forward: false,
                    crashed: None,
                    hung: false,
                    state: TabState::Background,
                    pinned: false,
                    audible: false,
                    last_active: now,
                };
                if insert_at >= self.tabs.len() {
                    self.tabs.push(tab);
                } else {
                    self.tabs.insert(insert_at, tab);
                    if let Some(a) = self.active
                        && insert_at <= a
                    {
                        self.active = Some(a + 1);
                    }
                }
                self.changed_flag = true;
            }
            EngineEvent::Audible { id, playing } => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.webview == *id) {
                    tab.audible = *playing;
                    self.changed_flag = true;
                }
            }
            EngineEvent::Hung { id } => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.webview == *id) {
                    tab.hung = true;
                    self.changed_flag = true;
                }
            }
            EngineEvent::Responsive { id } => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.webview == *id) {
                    tab.hung = false;
                    self.changed_flag = true;
                }
            }
            EngineEvent::NavigationDenied { .. } | EngineEvent::Download { .. } => {}
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
        let now = Timestamp::now();
        for (i, t) in s.tabs.iter().enumerate() {
            let webview = e.create(&t.url);
            let id = strip.next_tab_id();
            let state = if s.active == Some(i) {
                TabState::Active
            } else {
                TabState::Background
            };
            strip.tabs.push(Tab {
                id,
                webview,
                url: t.url.clone(),
                title: t.title.clone(),
                loading: false,
                can_back: false,
                can_forward: false,
                crashed: None,
                hung: false,
                state,
                pinned: false,
                audible: false,
                last_active: now,
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
