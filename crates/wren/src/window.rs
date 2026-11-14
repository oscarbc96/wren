use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::open_in_chrome::{self, Opener, SystemOpener};

use engine::{Engine, EngineEvent, EngineInput, LoadState, Modifiers, MouseButton, WebViewId};
use gpui::{
    App, ClickEvent, Context, FocusHandle, Focusable, KeyDownEvent, KeyUpEvent,
    MouseButton as GpuiMouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    Render, ScrollWheelEvent, Window, div, prelude::*, px,
};
use settings::Settings;
use store::Store;
use tabs::{TabId, TabState, TabStrip};
use url::Url;

use crate::toolbar::ToolbarState;
use themes::ActiveTheme;

pub struct BrowserWindow<E: Engine + 'static, S: Store + 'static> {
    pub engine: E,
    pub store: Arc<S>,
    pub cred_store: Arc<dyn creds::CredStore + Send + Sync>,
    pub opener: Arc<dyn Opener + Send + Sync>,
    pub settings: Settings,
    pub strip: TabStrip,
    pub toolbar: ToolbarState,
    pub focus_handle: FocusHandle,
    pub content_focus_handle: FocusHandle,
    pub omnibox_text: String,
    pub omnibox_focused: bool,
    pub omnibox_editing: String,
    pub suggestions: Vec<omnibox::Suggestion>,
    pub password_picker: Option<Vec<creds::Credential>>,
    pub password_status: Option<String>,
    pub site_notice: Option<String>,
    pub tab_menu: Option<TabId>,
    pub task_manager_open: bool,
    content_origin: Point<Pixels>,
    last_save: Option<Instant>,
    dirty_since: Option<Instant>,
    last_sleep_check: Option<Instant>,
    shutting_down: bool,
}

impl<E: Engine + 'static, S: Store + 'static> BrowserWindow<E, S> {
    pub fn new(engine: E, store: Arc<S>, settings: Settings, cx: &mut Context<Self>) -> Self {
        let cred_store: Arc<dyn creds::CredStore + Send + Sync> =
            Arc::new(creds::KeychainStore::new("wren.passwords"));
        Self::new_with_creds(engine, store, cred_store, settings, cx)
    }

    pub fn new_with_creds(
        engine: E,
        store: Arc<S>,
        cred_store: Arc<dyn creds::CredStore + Send + Sync>,
        settings: Settings,
        cx: &mut Context<Self>,
    ) -> Self {
        let opener: Arc<dyn Opener + Send + Sync> = Arc::new(SystemOpener);
        Self::new_with_creds_and_opener(engine, store, cred_store, opener, settings, cx)
    }

    pub fn new_with_creds_and_opener(
        mut engine: E,
        store: Arc<S>,
        cred_store: Arc<dyn creds::CredStore + Send + Sync>,
        opener: Arc<dyn Opener + Send + Sync>,
        settings: Settings,
        cx: &mut Context<Self>,
    ) -> Self {
        let new_tab_url = Url::parse(&settings.tabs.new_tab_url)
            .unwrap_or_else(|_| Url::parse("about:blank").unwrap());
        let mut strip = TabStrip::new(settings.tabs.closed_stack);
        if settings.startup.restore_session {
            if let Ok(Some(snap)) = store.load_session() {
                if !snap.tabs.is_empty() {
                    strip = TabStrip::restore(&mut engine, &snap, settings.tabs.closed_stack);
                } else {
                    strip.open(&mut engine, &new_tab_url, true);
                }
            } else {
                strip.open(&mut engine, &new_tab_url, true);
            }
        } else {
            strip.open(&mut engine, &new_tab_url, true);
        }
        Self::finish(engine, store, cred_store, opener, settings, strip, cx)
    }

    /// Opens one tab per URL, the first one active, without a leading blank tab.
    pub fn with_urls(
        engine: E,
        store: Arc<S>,
        settings: Settings,
        urls: Vec<Url>,
        cx: &mut Context<Self>,
    ) -> Self {
        let cred_store: Arc<dyn creds::CredStore + Send + Sync> =
            Arc::new(creds::KeychainStore::new("wren.passwords"));
        Self::with_urls_and_creds(engine, store, cred_store, settings, urls, cx)
    }

    pub fn with_urls_and_creds(
        engine: E,
        store: Arc<S>,
        cred_store: Arc<dyn creds::CredStore + Send + Sync>,
        settings: Settings,
        urls: Vec<Url>,
        cx: &mut Context<Self>,
    ) -> Self {
        let opener: Arc<dyn Opener + Send + Sync> = Arc::new(SystemOpener);
        Self::with_urls_and_opener(engine, store, cred_store, opener, settings, urls, cx)
    }

    pub fn with_urls_and_opener(
        mut engine: E,
        store: Arc<S>,
        cred_store: Arc<dyn creds::CredStore + Send + Sync>,
        opener: Arc<dyn Opener + Send + Sync>,
        settings: Settings,
        urls: Vec<Url>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut strip = TabStrip::new(settings.tabs.closed_stack);
        if urls.is_empty() {
            let url = Url::parse(&settings.tabs.new_tab_url)
                .unwrap_or_else(|_| Url::parse("about:blank").unwrap());
            strip.open(&mut engine, &url, true);
        } else {
            for (i, url) in urls.iter().enumerate() {
                strip.open(&mut engine, url, i == 0);
            }
        }
        Self::finish(engine, store, cred_store, opener, settings, strip, cx)
    }

    fn finish(
        mut engine: E,
        store: Arc<S>,
        cred_store: Arc<dyn creds::CredStore + Send + Sync>,
        opener: Arc<dyn Opener + Send + Sync>,
        settings: Settings,
        strip: TabStrip,
        cx: &mut Context<Self>,
    ) -> Self {
        let rules = settings.sites.open_in_chrome.clone();
        engine.set_navigation_filter(Box::new(move |url| {
            !open_in_chrome::rule_matches(&rules, url)
        }));

        let focus_handle = cx.focus_handle();
        let content_focus_handle = cx.focus_handle();
        let text = strip
            .active()
            .and_then(|id| {
                strip
                    .tabs()
                    .iter()
                    .find(|t| t.id == id)
                    .map(|t| display_text(&t.url))
            })
            .unwrap_or_else(|| {
                display_text(
                    &Url::parse(&settings.tabs.new_tab_url)
                        .unwrap_or_else(|_| Url::parse("about:blank").unwrap()),
                )
            });
        Self {
            engine,
            store,
            cred_store,
            opener,
            settings,
            strip,
            toolbar: ToolbarState::new(),
            focus_handle,
            content_focus_handle,
            omnibox_text: text.clone(),
            omnibox_focused: false,
            omnibox_editing: text,
            suggestions: Vec::new(),
            password_picker: None,
            password_status: None,
            site_notice: None,
            tab_menu: None,
            task_manager_open: false,
            content_origin: Point {
                x: px(0.),
                y: px(0.),
            },
            last_save: None,
            dirty_since: None,
            last_sleep_check: Some(Instant::now()),
            shutting_down: false,
        }
    }

    fn apply_navigation_filter(&mut self) {
        let rules = self.settings.sites.open_in_chrome.clone();
        self.engine.set_navigation_filter(Box::new(move |url| {
            !open_in_chrome::rule_matches(&rules, url)
        }));
    }

    pub fn active_webview(&self) -> Option<WebViewId> {
        let id = self.strip.active()?;
        self.strip
            .tabs()
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.webview)
    }

    pub fn active_tab(&self) -> Option<&tabs::Tab> {
        let id = self.strip.active()?;
        self.strip.tabs().iter().find(|t| t.id == id)
    }

    pub fn open_url(&mut self, url: Url) {
        if let Some(tab) = self.active_tab().cloned() {
            self.engine.load(tab.webview, &url);
        } else {
            self.strip.open(&mut self.engine, &url, true);
        }
        self.omnibox_focused = false;
        self.suggestions.clear();
    }

    pub fn navigate_input(&mut self, text: &str) {
        let trimmed = text.trim();
        match omnibox::classify(trimmed) {
            omnibox::Input::Navigate(url) => self.open_url(url),
            omnibox::Input::Search(q) => {
                let url = omnibox::search_url(&self.settings.search.url, &q);
                self.open_url(url);
            }
        }
    }

    pub fn new_tab(&mut self, focus_omnibox: bool) {
        let url = Url::parse(&self.settings.tabs.new_tab_url)
            .unwrap_or_else(|_| Url::parse("about:blank").unwrap());
        self.strip.open(&mut self.engine, &url, true);
        if focus_omnibox {
            self.toolbar.focus_omnibox();
        }
    }

    pub fn close_tab(&mut self, id: TabId) {
        self.strip.close(&mut self.engine, id);
        if self.strip.tabs().is_empty() {
            let url = Url::parse(&self.settings.tabs.new_tab_url)
                .unwrap_or_else(|_| Url::parse("about:blank").unwrap());
            self.strip.open(&mut self.engine, &url, true);
        }
    }

    pub fn close_active(&mut self) {
        if let Some(id) = self.strip.active() {
            self.close_tab(id);
        }
    }

    pub fn activate_index(&mut self, index: usize) {
        if let Some(tab) = self.strip.tabs().get(index).cloned() {
            self.strip.activate(tab.id);
        }
    }

    pub fn reload_active(&mut self) {
        if let Some(tab) = self.active_tab().cloned() {
            self.engine.reload(tab.webview);
        }
    }

    pub fn back_active(&mut self) {
        if let Some(tab) = self.active_tab().cloned() {
            self.engine.back(tab.webview);
        }
    }

    pub fn forward_active(&mut self) {
        if let Some(tab) = self.active_tab().cloned() {
            self.engine.forward(tab.webview);
        }
    }

    pub fn toggle_bookmark(&mut self) {
        if let Some(tab) = self.active_tab().cloned() {
            let _ = self.store.toggle_bookmark(&tab.url, &tab.title);
            self.toolbar.is_bookmarked = self.store.is_bookmarked(&tab.url).unwrap_or(false);
        }
    }

    pub fn toggle_task_manager(&mut self) {
        self.task_manager_open = !self.task_manager_open;
    }

    pub fn end_process_for_tab(&mut self, webview: WebViewId) {
        self.engine.kill(webview);
    }

    pub fn wait_hung_tab(&mut self, id: TabId) {
        if let Some(tab) = self.strip.tabs_mut().iter_mut().find(|t| t.id == id) {
            tab.hung = false;
        }
    }

    pub fn reload_crashed_tab(&mut self, id: TabId) {
        self.strip.reload_crashed(&mut self.engine, id);
    }

    pub fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let bounds = window.bounds();
        let w = f32::from(bounds.size.width).max(1.0);
        let h = f32::from(bounds.size.height).max(1.0);
        let chrome_h = self.toolbar.chrome_height();
        self.content_origin = Point {
            x: bounds.origin.x,
            y: bounds.origin.y + px(chrome_h),
        };
        let rect = crate::layout::content_rect((w, h), 0.0, chrome_h);
        if let Some(active) = self.active_tab().cloned() {
            if active.crashed.is_none() {
                self.engine.show(active.webview, rect);
            } else {
                self.engine.hide(active.webview);
            }
            for t in self.strip.tabs() {
                if t.webview != active.webview {
                    self.engine.hide(t.webview);
                }
            }
        }

        self.engine.spin();
        let events = self.engine.drain_events();
        if std::env::var_os("WREN_DEBUG_EVENTS").is_some() {
            for e in &events {
                eprintln!("WREN_EVENT {e:?}");
            }
        }
        // Engine events change titles, URLs and load state: redraw the chrome.
        if !events.is_empty() {
            cx.notify();
        }
        for ev in events {
            self.handle_event(&ev);
        }

        if self.strip.changed() {
            self.dirty_since = Some(Instant::now());
        }
        if let Some(since) = self.dirty_since
            && since.elapsed() >= Duration::from_secs(1)
        {
            let _ = self.store.save_session(&self.strip.snapshot());
            self.dirty_since = None;
            self.last_save = Some(Instant::now());
        }

        if self
            .last_sleep_check
            .map(|t| t.elapsed() >= Duration::from_secs(60))
            .unwrap_or(true)
        {
            self.last_sleep_check = Some(Instant::now());
            let now = jiff::Timestamp::now();
            let after =
                jiff::SignedDuration::from_secs(self.settings.tabs.sleep_after_minutes as i64 * 60);
            for id in self.strip.sleep_candidates(now, after) {
                self.strip.sleep(&mut self.engine, id);
            }

            let budget_bytes = self
                .settings
                .tabs
                .memory_budget_mb
                .saturating_mul(1024 * 1024);
            if budget_bytes > 0
                && let Ok(mut sys) = sysinfo::System::new()
            {
                sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
                let me = sysinfo::Pid::from_u32(std::process::id());
                let mut total = 0u64;
                if let Some(p) = sys.process(me) {
                    total = p.memory();
                }
                // include direct children (content processes)
                for p in sys.processes().values() {
                    if p.parent() == Some(me) {
                        total = total.saturating_add(p.memory());
                    }
                }
                if total > budget_bytes {
                    // Sleep oldest eligible background tabs until under budget or none left.
                    loop {
                        if total <= budget_bytes {
                            break;
                        }
                        let candidate = self
                            .strip
                            .tabs()
                            .iter()
                            .filter(|t| {
                                self.strip.active() != Some(t.id)
                                    && !t.pinned
                                    && !t.audible
                                    && t.state != TabState::Slept
                            })
                            .min_by_key(|t| t.last_active)
                            .map(|t| t.id);
                        let Some(id) = candidate else {
                            break;
                        };
                        self.strip.sleep(&mut self.engine, id);
                        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
                        total = 0u64;
                        if let Some(p) = sys.process(me) {
                            total = p.memory();
                        }
                        for p in sys.processes().values() {
                            if p.parent() == Some(me) {
                                total = total.saturating_add(p.memory());
                            }
                        }
                    }
                }
            }
        }
    }

    fn handle_event(&mut self, ev: &EngineEvent) {
        if let EngineEvent::Load {
            id,
            state: LoadState::Complete,
        } = ev
            && let Some(tab) = self.strip.tabs().iter().find(|t| &t.webview == id)
        {
            let scheme = tab.url.scheme();
            if scheme == "http" || scheme == "https" {
                let _ = self
                    .store
                    .record_visit(&tab.url, &tab.title, jiff::Timestamp::now());
            }
        }
        if let EngineEvent::NavigationDenied { url, .. } = ev {
            self.open_external(url.clone());
        }
        self.strip.apply(&mut self.engine, ev);
        if let Some(tab) = self.active_tab().cloned() {
            self.toolbar.url_text = tab.url.to_string();
            self.toolbar.can_back = tab.can_back;
            self.toolbar.can_forward = tab.can_forward;
            self.toolbar.loading = tab.loading;
            self.toolbar.is_bookmarked = self.store.is_bookmarked(&tab.url).unwrap_or(false);
            self.toolbar.crashed = tab.crashed;
            self.toolbar.title = tab.title.clone();
        }
        if self.strip.changed() {
            self.sync_omnibox_text();
        }
    }

    fn open_external(&mut self, url: Url) {
        if let Err(msg) = self.opener.open(&url) {
            self.site_notice = Some(msg);
        }
    }

    pub fn open_active_in_chrome(&mut self) {
        if let Some(tab) = self.active_tab().cloned() {
            self.open_external(tab.url);
        }
    }

    pub fn always_open_active_in_chrome(&mut self) {
        if let Some(tab) = self.active_tab().cloned() {
            if let Some(host) = tab.url.host_str() {
                let host = host.to_ascii_lowercase();
                if !self
                    .settings
                    .sites
                    .open_in_chrome
                    .iter()
                    .any(|h| h == &host)
                {
                    self.settings.sites.open_in_chrome.push(host);
                    self.apply_navigation_filter();
                }
            }
            self.open_external(tab.url);
        }
    }

    fn sync_omnibox_text(&mut self) {
        if self.omnibox_focused {
            return;
        }
        let text = self
            .active_tab()
            .map(|t| display_text(&t.url))
            .unwrap_or_default();
        self.omnibox_text = text.clone();
        self.omnibox_editing = text;
    }

    pub fn save_session_now(&mut self) {
        let _ = self.store.save_session(&self.strip.snapshot());
        self.dirty_since = None;
    }

    pub fn shutdown(&mut self) {
        if self.shutting_down {
            return;
        }
        self.shutting_down = true;
        self.save_session_now();
        self.engine.shutdown();
    }

    pub fn focus_omnibox_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.omnibox_focused = true;
        self.toolbar.focus_omnibox();
        if let Some(tab) = self.active_tab() {
            self.omnibox_editing = tab.url.to_string();
        }
        self.refresh_suggestions();
        window.focus(&self.focus_handle, cx);
    }

    pub fn blur_omnibox(&mut self) {
        self.omnibox_focused = false;
        self.toolbar.focus_omnibox = false;
        self.suggestions.clear();
        self.sync_omnibox_text();
    }

    fn refresh_suggestions(&mut self) {
        let q = self.omnibox_editing.trim();
        if q.is_empty() {
            self.suggestions.clear();
            return;
        }
        self.suggestions = omnibox::suggest(self.store.as_ref(), q, 6);
    }

    fn omnibox_type_char(&mut self, ch: char) {
        self.omnibox_editing.push(ch);
        self.refresh_suggestions();
    }

    fn omnibox_backspace(&mut self) {
        self.omnibox_editing.pop();
        self.refresh_suggestions();
    }

    fn omnibox_enter(&mut self) {
        let text = self.omnibox_editing.clone();
        self.navigate_input(&text);
        self.omnibox_focused = false;
        self.toolbar.focus_omnibox = false;
        self.suggestions.clear();
    }

    fn omnibox_escape(&mut self) {
        self.blur_omnibox();
    }

    fn tab_index(&self) -> Option<usize> {
        self.strip
            .active()
            .and_then(|id| self.strip.tabs().iter().position(|t| t.id == id))
    }

    fn content_point(&self, position: Point<Pixels>) -> (f32, f32) {
        let x = f32::from(position.x - self.content_origin.x).max(0.0);
        let y = f32::from(position.y - self.content_origin.y).max(0.0);
        (x, y)
    }

    fn map_mouse_button(button: GpuiMouseButton) -> Option<MouseButton> {
        match button {
            GpuiMouseButton::Left => Some(MouseButton::Left),
            GpuiMouseButton::Right => Some(MouseButton::Right),
            GpuiMouseButton::Middle => Some(MouseButton::Middle),
            _ => None,
        }
    }

    fn map_modifiers(m: &gpui::Modifiers) -> Modifiers {
        Modifiers {
            shift: m.shift,
            ctrl: m.control,
            alt: m.alt,
            cmd: m.platform,
        }
    }

    fn map_key(keystroke: &gpui::Keystroke) -> Option<(String, String)> {
        let k = keystroke.key.as_str();
        let (key, code) = match k {
            "enter" => ("Enter".into(), "Enter".into()),
            "escape" => ("Escape".into(), "Escape".into()),
            "backspace" => ("Backspace".into(), "Backspace".into()),
            "tab" => ("Tab".into(), "Tab".into()),
            "space" => (" ".into(), "Space".into()),
            "left" => ("ArrowLeft".into(), "ArrowLeft".into()),
            "right" => ("ArrowRight".into(), "ArrowRight".into()),
            "up" => ("ArrowUp".into(), "ArrowUp".into()),
            "down" => ("ArrowDown".into(), "ArrowDown".into()),
            "home" => ("Home".into(), "Home".into()),
            "end" => ("End".into(), "End".into()),
            "pageup" => ("PageUp".into(), "PageUp".into()),
            "pagedown" => ("PageDown".into(), "PageDown".into()),
            "delete" => ("Delete".into(), "Delete".into()),
            s if s.len() == 1 => {
                let ch = s.chars().next().unwrap();
                if ch.is_ascii_alphabetic() {
                    let lower = ch.to_ascii_lowercase().to_string();
                    let upper = ch.to_ascii_uppercase().to_string();
                    (lower, format!("Key{upper}"))
                } else if ch.is_ascii_digit() {
                    (ch.to_string(), format!("Digit{ch}"))
                } else {
                    (ch.to_string(), String::new())
                }
            }
            _ => return None,
        };
        Some((key, code))
    }

    fn has_cmd_binding(mods: &gpui::Modifiers) -> bool {
        mods.platform
    }

    fn render_chrome(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<ActiveTheme>().clone();
        let chrome_h = self.toolbar.chrome_height();
        let mut col = div()
            .relative()
            .flex()
            .flex_col()
            .w_full()
            .h(px(chrome_h))
            .bg(theme.background)
            .child(self.render_tab_strip(cx));
        if let Some(menu_id) = self.tab_menu {
            let pinned = self
                .strip
                .tabs()
                .iter()
                .find(|t| t.id == menu_id)
                .map(|t| t.pinned)
                .unwrap_or(false);
            col = col.child(
                div()
                    .absolute()
                    .top(px(34.0))
                    .left(px(80.0))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(8.0))
                    .py(px(4.0))
                    .rounded_md()
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.border)
                    .shadow_lg()
                    .child(
                        div()
                            .id("tab-menu-pin")
                            .cursor_pointer()
                            .on_click(cx.listener(
                                move |this: &mut Self, _: &ClickEvent, _window, _cx| {
                                    if let Some(t) =
                                        this.strip.tabs_mut().iter_mut().find(|t| t.id == menu_id)
                                    {
                                        t.pinned = !t.pinned;
                                    }
                                    this.tab_menu = None;
                                },
                            ))
                            .child(if pinned { "Unpin" } else { "Pin" }),
                    )
                    .child(
                        div()
                            .id("tab-menu-sleep")
                            .cursor_pointer()
                            .on_click(cx.listener(
                                move |this: &mut Self, _: &ClickEvent, _window, _cx| {
                                    this.strip.sleep(&mut this.engine, menu_id);
                                    this.tab_menu = None;
                                },
                            ))
                            .child("Sleep"),
                    )
                    .child(
                        div()
                            .id("tab-menu-close")
                            .cursor_pointer()
                            .on_click(cx.listener(|this: &mut Self, _: &ClickEvent, _w, _cx| {
                                this.tab_menu = None;
                            }))
                            .child("Dismiss"),
                    ),
            );
        }
        col.child(self.render_toolbar(cx))
    }

    fn render_tab_strip(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<ActiveTheme>().clone();
        let tabs = self.strip.tabs().to_vec();
        let active_id = self.strip.active();
        let mut children: Vec<gpui::AnyElement> = Vec::new();
        for (i, tab) in tabs.iter().enumerate() {
            let tab = tab.clone();
            let selected = active_id == Some(tab.id);
            let id = tab.id;
            let label = if tab.crashed.is_some() {
                "! crashed".to_string()
            } else if tab.title.is_empty() {
                display_text(&tab.url)
            } else {
                truncate_text(&tab.title, 24)
            };
            let host_letter = tab
                .url
                .host_str()
                .and_then(|h| h.chars().next())
                .map(|c| c.to_ascii_uppercase().to_string())
                .unwrap_or_else(|| "?".to_string());
            let is_slept = tab.state == TabState::Slept;
            children.push(
                div()
                    .id(("tab", i))
                    .group("tab")
                    .h(px(30.0))
                    .min_w(px(120.0))
                    .max_w(px(220.0))
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(8.0))
                    .rounded_md()
                    .cursor_pointer()
                    .when(selected, |style| style.bg(theme.surface))
                    .when(!selected, |style| style.bg(gpui::transparent_black()))
                    .hover(|style| style.bg(theme.surface))
                    .when(is_slept, |style| style.opacity(0.5))
                    .on_click(
                        cx.listener(move |this: &mut Self, _: &ClickEvent, _window, _cx| {
                            let slept = this
                                .strip
                                .tabs()
                                .iter()
                                .find(|t| t.id == id)
                                .map(|t| t.state == TabState::Slept)
                                .unwrap_or(false);
                            if slept {
                                this.strip.wake(&mut this.engine, id);
                            } else {
                                this.strip.activate(id);
                            }
                        }),
                    )
                    .on_mouse_down(
                        gpui::MouseButton::Right,
                        cx.listener(move |this: &mut Self, _: &MouseDownEvent, _window, _cx| {
                            this.tab_menu = Some(id);
                        }),
                    )
                    .child(
                        div()
                            .w(px(16.0))
                            .h(px(16.0))
                            .rounded_sm()
                            .bg(theme.accent)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(10.0))
                            .text_color(theme.accent_text)
                            .child(host_letter),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(13.0))
                            .text_color(theme.text)
                            .child(label),
                    )
                    .child(
                        div()
                            .id(("tab-close", i))
                            .w(px(16.0))
                            .h(px(16.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_sm()
                            .cursor_pointer()
                            .opacity(if selected { 1.0 } else { 0.0 })
                            .group_hover("tab", |style| style.opacity(1.0))
                            .on_click(cx.listener(
                                move |this: &mut Self, _: &ClickEvent, _window, _cx| {
                                    this.close_tab(id);
                                },
                            ))
                            .child("×"),
                    )
                    .into_any_element(),
            );
        }
        div()
            .flex()
            .items_center()
            .h(px(38.0))
            .w_full()
            .pl(px(86.0))
            .gap(px(6.0))
            .bg(theme.background)
            .children(children)
            .child(
                div()
                    .id("new-tab")
                    .h(px(28.0))
                    .w(px(28.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_md()
                    .cursor_pointer()
                    .text_size(px(16.0))
                    .text_color(theme.muted_text)
                    .hover(|style| style.bg(theme.surface))
                    .on_click(
                        cx.listener(|this: &mut Self, _: &ClickEvent, _window, _cx| {
                            this.new_tab(false);
                        }),
                    )
                    .child("+"),
            )
    }

    fn render_toolbar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<ActiveTheme>().clone();
        let can_back = self.toolbar.can_back;
        let can_forward = self.toolbar.can_forward;
        let loading = self.toolbar.loading;
        let bookmarked = self.toolbar.is_bookmarked;
        let url_display = if self.omnibox_focused {
            self.omnibox_editing.clone()
        } else if self.omnibox_text.is_empty() {
            self.toolbar.url_text.clone()
        } else {
            self.omnibox_text.clone()
        };
        let mut suggestions = Vec::new();
        if self.omnibox_focused && !self.suggestions.is_empty() {
            for (i, s) in self.suggestions.iter().enumerate() {
                let url = s.url.clone();
                suggestions.push(
                    div()
                        .id(("suggest", i))
                        .px(px(8.0))
                        .py(px(4.0))
                        .cursor_pointer()
                        .on_click(cx.listener(
                            move |this: &mut Self, _: &ClickEvent, _window, _cx| {
                                this.open_url(url.clone());
                                this.omnibox_focused = false;
                                this.suggestions.clear();
                            },
                        ))
                        .child(format!("{} - {}", s.title, s.url)),
                );
            }
        }

        let icon_button = |id: &'static str, label: &'static str, enabled: bool| {
            div()
                .id(id)
                .w(px(28.0))
                .h(px(28.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_md()
                .cursor_pointer()
                .opacity(if enabled { 1.0 } else { 0.4 })
                .child(label)
        };

        div()
            .flex()
            .flex_col()
            .w_full()
            .bg(theme.background)
            .child(
                div()
                    .h(px(40.0))
                    .w_full()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(8.0))
                    .child(
                        icon_button("back", "\u{2190}", can_back).on_click(cx.listener(
                            |this: &mut Self, _: &ClickEvent, _window, _cx| {
                                this.back_active();
                            },
                        )),
                    )
                    .child(
                        icon_button("forward", "\u{2192}", can_forward).on_click(cx.listener(
                            |this: &mut Self, _: &ClickEvent, _window, _cx| {
                                this.forward_active();
                            },
                        )),
                    )
                    .child(
                        div()
                            .id("reload")
                            .w(px(28.0))
                            .h(px(28.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_md()
                            .cursor_pointer()
                            .on_click(cx.listener(
                                |this: &mut Self, _: &ClickEvent, _window, _cx| {
                                    this.reload_active();
                                },
                            ))
                            .child(if loading { "\u{25A0}" } else { "\u{21BB}" }),
                    )
                    .child(
                        div()
                            .id("url-display")
                            .flex_1()
                            .h(px(30.0))
                            .px(px(12.0))
                            .flex()
                            .items_center()
                            .rounded_md()
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.surface)
                            .cursor_text()
                            .on_click(cx.listener(|this: &mut Self, _: &ClickEvent, window, cx| {
                                this.focus_omnibox_window(window, cx);
                            }))
                            .child(if url_display.is_empty() {
                                "Search or enter address".to_string()
                            } else {
                                url_display
                            }),
                    )
                    .child(
                        div()
                            .id("star")
                            .cursor_pointer()
                            .on_click(cx.listener(
                                |this: &mut Self, _: &ClickEvent, _window, _cx| {
                                    this.toggle_bookmark();
                                },
                            ))
                            .child(if bookmarked { "*" } else { "\u{2606}" }),
                    ),
            )
            .when(!suggestions.is_empty(), |d| {
                d.child(
                    div()
                        .flex()
                        .flex_col()
                        .w_full()
                        .px(px(8.0))
                        .bg(theme.surface)
                        .children(suggestions),
                )
            })
    }

    pub fn fill_passwords(&mut self) {
        self.password_status = None;
        let Some(tab) = self.active_tab().cloned() else {
            return;
        };
        let origin = tab.url.origin().ascii_serialization();
        match self.cred_store.find(&origin) {
            Ok(creds) if creds.is_empty() => {
                self.password_status = Some("No saved passwords for this site".to_string());
            }
            Ok(creds) if creds.len() == 1 => {
                let c = &creds[0];
                let js = creds::fill_script(&c.username, &c.password);
                self.engine.run_script(tab.webview, &js);
                self.password_picker = None;
            }
            Ok(creds) => {
                self.password_picker = Some(creds);
            }
            Err(e) => {
                self.password_status = Some(format!("Password lookup failed: {e}"));
            }
        }
    }

    pub fn choose_password(&mut self, idx: usize) {
        let Some(tab) = self.active_tab().cloned() else {
            return;
        };
        if let Some(list) = &self.password_picker
            && let Some(c) = list.get(idx)
        {
            let js = creds::fill_script(&c.username, &c.password);
            self.engine.run_script(tab.webview, &js);
        }
        self.password_picker = None;
    }

    pub fn import_passwords_from_chrome(&mut self) {
        let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        let profile = home
            .join("Library")
            .join("Application Support")
            .join("Google")
            .join("Chrome")
            .join("Default");
        match creds::chrome::import(&profile, self.cred_store.as_ref()) {
            Ok(n) => self.password_status = Some(format!("Imported {n} passwords from Chrome")),
            Err(e) => self.password_status = Some(format!("Chrome import failed: {e}")),
        }
    }

    pub fn import_passwords_from_csv(&mut self, path: &std::path::Path) {
        match creds::csv::import(path, self.cred_store.as_ref()) {
            Ok(n) => self.password_status = Some(format!("Imported {n} passwords from CSV")),
            Err(e) => self.password_status = Some(format!("CSV import failed: {e}")),
        }
    }

    fn render_content(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.task_manager_open {
            let processes = self.engine.processes();
            let rows = crate::task_manager::rows(processes, &self.strip);
            let mut items = Vec::new();
            for (i, row) in rows.iter().enumerate() {
                let label = format!(
                    "{} | pid {} | RSS {} | CPU {:.1}%",
                    row.title, row.pid, row.rss_bytes, row.cpu_percent
                );
                let mut item = div()
                    .id(("tm-row", i))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .px(px(8.))
                    .py(px(4.))
                    .child(label);
                if let Some(wv) = row.id {
                    item = item.child(
                        div()
                            .id(("tm-end", i))
                            .cursor_pointer()
                            .on_click(cx.listener(
                                move |this: &mut Self, _: &ClickEvent, _w, _cx| {
                                    this.end_process_for_tab(wv);
                                },
                            ))
                            .child("End process"),
                    );
                }
                items.push(item.into_any_element());
            }
            return div()
                .size_full()
                .flex()
                .flex_col()
                .gap(px(4.))
                .p(px(8.))
                .child("Task manager")
                .children(items)
                .child(
                    div()
                        .id("tm-close")
                        .cursor_pointer()
                        .on_click(cx.listener(|this: &mut Self, _: &ClickEvent, _w, _cx| {
                            this.task_manager_open = false;
                        }))
                        .child("Close"),
                )
                .into_any_element();
        }
        if let Some(list) = self.password_picker.clone() {
            let mut items = Vec::new();
            for (i, c) in list.iter().enumerate() {
                let label = if c.username.is_empty() {
                    "(no username)".to_string()
                } else {
                    c.username.clone()
                };
                items.push(
                    div()
                        .id(("pw-pick", i))
                        .px(px(8.))
                        .py(px(4.))
                        .cursor_pointer()
                        .on_click(cx.listener(
                            move |this: &mut Self, _: &ClickEvent, _window, _cx| {
                                this.choose_password(i);
                            },
                        ))
                        .child(label)
                        .into_any_element(),
                );
            }
            return div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .child("Choose an account to fill")
                .children(items)
                .child(
                    div()
                        .id("pw-cancel")
                        .cursor_pointer()
                        .on_click(cx.listener(|this: &mut Self, _: &ClickEvent, _w, _cx| {
                            this.password_picker = None;
                        }))
                        .child("Cancel"),
                )
                .into_any_element();
        }
        if let Some(tab) = self.active_tab().cloned()
            && let Some(reason) = tab.crashed.clone()
        {
            let tab_id = tab.id;
            return div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .child("This tab's process stopped")
                .child(reason)
                .child(
                    div()
                        .id("crashed-reload")
                        .cursor_pointer()
                        .on_click(cx.listener(
                            move |this: &mut Self, _: &ClickEvent, _window, _cx| {
                                this.reload_crashed_tab(tab_id);
                            },
                        ))
                        .child("Reload"),
                )
                .into_any_element();
        }
        let hung_bar = if let Some(tab) = self.active_tab().cloned() {
            if tab.hung {
                let tab_id = tab.id;
                let wv = tab.webview;
                Some(
                    div()
                        .w_full()
                        .px(px(8.))
                        .py(px(4.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child("This page is not responding")
                        .child(
                            div()
                                .id("hung-wait")
                                .cursor_pointer()
                                .on_click(cx.listener(
                                    move |this: &mut Self, _: &ClickEvent, _w, _cx| {
                                        this.wait_hung_tab(tab_id);
                                    },
                                ))
                                .child("Wait"),
                        )
                        .child(
                            div()
                                .id("hung-end")
                                .cursor_pointer()
                                .on_click(cx.listener(
                                    move |this: &mut Self, _: &ClickEvent, _w, _cx| {
                                        this.end_process_for_tab(wv);
                                    },
                                ))
                                .child("End process"),
                        )
                        .into_any_element(),
                )
            } else {
                None
            }
        } else {
            None
        };
        let content_focus = self.content_focus_handle.clone();
        let mut root = div().flex().flex_col().size_full();
        if let Some(bar) = hung_bar {
            root = root.child(bar);
        }
        root.child(
            div()
                .id("content-area")
                .track_focus(&content_focus)
                .flex_1()
                .w_full()
                .on_mouse_move(cx.listener(
                    |this: &mut Self, event: &MouseMoveEvent, _window, _cx| {
                        if let Some(active) = this.active_webview() {
                            let (x, y) = this.content_point(event.position);
                            this.engine.input(active, EngineInput::MouseMove { x, y });
                        }
                    },
                ))
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(|this: &mut Self, event: &MouseDownEvent, window, cx| {
                        window.focus(&this.content_focus_handle, cx);
                        if let Some(active) = this.active_webview() {
                            let (x, y) = this.content_point(event.position);
                            if let Some(button) = Self::map_mouse_button(event.button) {
                                this.engine
                                    .input(active, EngineInput::MouseDown { button, x, y });
                            }
                        }
                    }),
                )
                .on_mouse_down(
                    gpui::MouseButton::Right,
                    cx.listener(|this: &mut Self, event: &MouseDownEvent, window, cx| {
                        window.focus(&this.content_focus_handle, cx);
                        if let Some(active) = this.active_webview() {
                            let (x, y) = this.content_point(event.position);
                            if let Some(button) = Self::map_mouse_button(event.button) {
                                this.engine
                                    .input(active, EngineInput::MouseDown { button, x, y });
                            }
                        }
                    }),
                )
                .on_mouse_down(
                    gpui::MouseButton::Middle,
                    cx.listener(|this: &mut Self, event: &MouseDownEvent, window, cx| {
                        window.focus(&this.content_focus_handle, cx);
                        if let Some(active) = this.active_webview() {
                            let (x, y) = this.content_point(event.position);
                            if let Some(button) = Self::map_mouse_button(event.button) {
                                this.engine
                                    .input(active, EngineInput::MouseDown { button, x, y });
                            }
                        }
                    }),
                )
                .on_mouse_up(
                    gpui::MouseButton::Left,
                    cx.listener(|this: &mut Self, event: &MouseUpEvent, _window, _cx| {
                        if let Some(active) = this.active_webview() {
                            let (x, y) = this.content_point(event.position);
                            if let Some(button) = Self::map_mouse_button(event.button) {
                                this.engine
                                    .input(active, EngineInput::MouseUp { button, x, y });
                            }
                        }
                    }),
                )
                .on_mouse_up(
                    gpui::MouseButton::Right,
                    cx.listener(|this: &mut Self, event: &MouseUpEvent, _window, _cx| {
                        if let Some(active) = this.active_webview() {
                            let (x, y) = this.content_point(event.position);
                            if let Some(button) = Self::map_mouse_button(event.button) {
                                this.engine
                                    .input(active, EngineInput::MouseUp { button, x, y });
                            }
                        }
                    }),
                )
                .on_mouse_up(
                    gpui::MouseButton::Middle,
                    cx.listener(|this: &mut Self, event: &MouseUpEvent, _window, _cx| {
                        if let Some(active) = this.active_webview() {
                            let (x, y) = this.content_point(event.position);
                            if let Some(button) = Self::map_mouse_button(event.button) {
                                this.engine
                                    .input(active, EngineInput::MouseUp { button, x, y });
                            }
                        }
                    }),
                )
                .on_scroll_wheel(cx.listener(
                    |this: &mut Self, event: &ScrollWheelEvent, _window, _cx| {
                        if let Some(active) = this.active_webview() {
                            let (x, y) = this.content_point(event.position);
                            let (dx, dy) = match event.delta {
                                gpui::ScrollDelta::Pixels(p) => (-f32::from(p.x), -f32::from(p.y)),
                                gpui::ScrollDelta::Lines(l) => (-l.x * 16.0, -l.y * 16.0),
                            };
                            this.engine
                                .input(active, EngineInput::Wheel { dx, dy, x, y });
                        }
                    },
                ))
                .on_key_down(
                    cx.listener(|this: &mut Self, event: &KeyDownEvent, _window, _cx| {
                        if Self::has_cmd_binding(&event.keystroke.modifiers) {
                            return;
                        }
                        if let Some(active) = this.active_webview()
                            && let Some((key, code)) = Self::map_key(&event.keystroke)
                        {
                            let modifiers = Self::map_modifiers(&event.keystroke.modifiers);
                            this.engine.input(
                                active,
                                EngineInput::Key {
                                    down: true,
                                    key,
                                    code,
                                    modifiers,
                                },
                            );
                        }
                    }),
                )
                .on_key_up(
                    cx.listener(|this: &mut Self, event: &KeyUpEvent, _window, _cx| {
                        if Self::has_cmd_binding(&event.keystroke.modifiers) {
                            return;
                        }
                        if let Some(active) = this.active_webview()
                            && let Some((key, code)) = Self::map_key(&event.keystroke)
                        {
                            let modifiers = Self::map_modifiers(&event.keystroke.modifiers);
                            this.engine.input(
                                active,
                                EngineInput::Key {
                                    down: false,
                                    key,
                                    code,
                                    modifiers,
                                },
                            );
                        }
                    }),
                ),
        )
        .into_any_element()
    }
}

impl<E: Engine + 'static, S: Store + 'static> Focusable for BrowserWindow<E, S> {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl<E: Engine + 'static, S: Store + 'static> Render for BrowserWindow<E, S> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let chrome_h = self.toolbar.chrome_height();
        div()
            .id("browser-window")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .on_key_down(
                cx.listener(|this: &mut Self, event: &KeyDownEvent, window, cx| {
                    if this.omnibox_focused {
                        if event.keystroke.modifiers.platform {
                            return;
                        }
                        match event.keystroke.key.as_str() {
                            "enter" => {
                                this.omnibox_enter();
                                cx.notify();
                            }
                            "escape" => {
                                this.omnibox_escape();
                                cx.notify();
                            }
                            "backspace" => {
                                this.omnibox_backspace();
                                cx.notify();
                            }
                            "space" => {
                                this.omnibox_type_char(' ');
                                cx.notify();
                            }
                            k if k.len() == 1 => {
                                if let Some(ch) = k.chars().next() {
                                    this.omnibox_type_char(ch);
                                    cx.notify();
                                }
                            }
                            _ => {}
                        }
                        return;
                    }
                    if !this.content_focus_handle.is_focused(window)
                        && event.keystroke.modifiers.platform
                    {}
                }),
            )
            .child(
                div()
                    .h(px(chrome_h))
                    .w_full()
                    .child(self.render_chrome(window, cx)),
            )
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .child(self.render_content(window, cx)),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &crate::actions::New, _window, _cx| {
                    this.new_tab(true);
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &crate::actions::Close, _window, _cx| {
                    this.close_active();
                }),
            )
            .on_action(cx.listener(
                |this: &mut Self, _: &crate::actions::Reopen, _window, _cx| {
                    this.strip.reopen_closed(&mut this.engine);
                },
            ))
            .on_action(
                cx.listener(|this: &mut Self, _: &crate::actions::Next, _window, _cx| {
                    if let Some(i) = this.tab_index() {
                        this.activate_index(i + 1);
                    }
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &crate::actions::Prev, _window, _cx| {
                    if let Some(i) = this.tab_index()
                        && i > 0
                    {
                        this.activate_index(i - 1);
                    }
                }),
            )
            .on_action(cx.listener(
                |this: &mut Self, _: &crate::actions::Select1, _window, _cx| {
                    this.activate_index(0);
                },
            ))
            .on_action(cx.listener(
                |this: &mut Self, _: &crate::actions::Select2, _window, _cx| {
                    this.activate_index(1);
                },
            ))
            .on_action(cx.listener(
                |this: &mut Self, _: &crate::actions::Select3, _window, _cx| {
                    this.activate_index(2);
                },
            ))
            .on_action(cx.listener(
                |this: &mut Self, _: &crate::actions::Select4, _window, _cx| {
                    this.activate_index(3);
                },
            ))
            .on_action(cx.listener(
                |this: &mut Self, _: &crate::actions::Select5, _window, _cx| {
                    this.activate_index(4);
                },
            ))
            .on_action(cx.listener(
                |this: &mut Self, _: &crate::actions::Select6, _window, _cx| {
                    this.activate_index(5);
                },
            ))
            .on_action(cx.listener(
                |this: &mut Self, _: &crate::actions::Select7, _window, _cx| {
                    this.activate_index(6);
                },
            ))
            .on_action(cx.listener(
                |this: &mut Self, _: &crate::actions::Select8, _window, _cx| {
                    this.activate_index(7);
                },
            ))
            .on_action(cx.listener(
                |this: &mut Self, _: &crate::actions::Select9, _window, _cx| {
                    if let Some(tab) = this.strip.tabs().last().cloned() {
                        this.strip.activate(tab.id);
                    }
                },
            ))
            .on_action(
                cx.listener(|this: &mut Self, _: &crate::actions::Focus, window, cx| {
                    this.focus_omnibox_window(window, cx);
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &crate::actions::Back, _window, _cx| {
                    this.back_active();
                }),
            )
            .on_action(cx.listener(
                |this: &mut Self, _: &crate::actions::Forward, _window, _cx| {
                    this.forward_active();
                },
            ))
            .on_action(cx.listener(
                |this: &mut Self, _: &crate::actions::Reload, _window, _cx| {
                    this.reload_active();
                },
            ))
            .on_action(cx.listener(
                |this: &mut Self, _: &crate::actions::Toggle, _window, _cx| {
                    this.toggle_bookmark();
                },
            ))
            .on_action(
                cx.listener(|this: &mut Self, _: &crate::actions::Fill, _window, _cx| {
                    this.fill_passwords();
                }),
            )
            .on_action(cx.listener(
                |this: &mut Self, _: &crate::actions::ImportChrome, _window, _cx| {
                    this.import_passwords_from_chrome();
                },
            ))
            .on_action(cx.listener(
                |this: &mut Self, _: &crate::actions::ImportCsv, _window, _cx| {
                    this.password_status = Some(
                        "CSV import: provide a path via settings/import action (not wired in UI)"
                            .to_string(),
                    );
                },
            ))
            .on_action(cx.listener(
                |this: &mut Self, _: &crate::actions::OpenInChrome, _window, _cx| {
                    this.open_active_in_chrome();
                },
            ))
            .on_action(cx.listener(
                |this: &mut Self, _: &crate::actions::AlwaysOpenInChrome, _window, _cx| {
                    this.always_open_active_in_chrome();
                },
            ))
            .on_action(cx.listener(
                |this: &mut Self, _: &crate::task_manager::Toggle, _window, _cx| {
                    this.toggle_task_manager();
                },
            ))
    }
}

fn display_text(url: &Url) -> String {
    let host = omnibox::display_host(url);
    if host.is_empty() {
        url.as_str().to_string()
    } else {
        host
    }
}

fn truncate_text(text: &str, max: usize) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        if out.chars().count() >= max {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}

pub use crate::layout::content_rect;
