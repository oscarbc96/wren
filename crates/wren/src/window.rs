use std::sync::Arc;
use std::time::{Duration, Instant};

use engine::{Engine, EngineEvent, LoadState, ViewRect, WebViewId};
use gpui::{
    App, ClickEvent, Context, Entity, FocusHandle, Focusable, Global, Render, Window, div, prelude::*,
    px,
};
use gpui_ce_elements::editable_text::{
    actions::{default_bindings, DEFAULT_INPUT_CONTEXT}, EditableTextState, StringStorage,
};
use settings::Settings;
use store::Store;
use tabs::{TabId, TabStrip};
use url::Url;

use crate::toolbar::ToolbarState;
use themes::ActiveTheme;

pub struct BrowserWindow<E: Engine + 'static, S: Store + 'static> {
    pub engine: E,
    pub store: Arc<S>,
    pub settings: Settings,
    pub strip: TabStrip,
    pub toolbar: ToolbarState,
    pub focus_handle: FocusHandle,
    pub omnibox: Entity<EditableTextState>,
    pub omnibox_text: String,
    pub omnibox_focused: bool,
    last_save: Option<Instant>,
    dirty_since: Option<Instant>,
    shutting_down: bool,
}

impl<E: Engine + 'static, S: Store + 'static> BrowserWindow<E, S> {
    pub fn new(mut engine: E, store: Arc<S>, settings: Settings, cx: &mut Context<Self>) -> Self {
        let new_tab_url = Url::parse(&settings.tabs.new_tab_url)
            .unwrap_or_else(|_| Url::parse("about:blank").unwrap());
        let mut strip = TabStrip::new(settings.tabs.closed_stack);
        if settings.startup.restore_session
            && let Ok(Some(snap)) = store.load_session()
            && !snap.tabs.is_empty()
        {
            strip = TabStrip::restore(&mut engine, &snap, settings.tabs.closed_stack);
        } else {
            strip.open(&mut engine, &new_tab_url, true);
        }
        Self::finish(engine, store, settings, strip, cx)
    }

    /// Opens one tab per URL, the first one active, without a leading blank tab.
    pub fn with_urls(
        mut engine: E,
        store: Arc<S>,
        settings: Settings,
        urls: Vec<Url>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut strip = TabStrip::new(settings.tabs.closed_stack);
        for (i, url) in urls.iter().enumerate() {
            strip.open(&mut engine, url, i == 0);
        }
        Self::finish(engine, store, settings, strip, cx)
    }

    fn finish(
        mut engine: E,
        store: Arc<S>,
        settings: Settings,
        strip: TabStrip,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        let omnibox = cx.new(|cx| EditableTextState::new(StringStorage::default(), cx));
        let text = strip
            .active()
            .and_then(|id| {
                strip
                    .tabs()
                    .iter()
                    .find(|t| t.id == id)
                    .map(|t| display_text(&t.url))
            })
            .unwrap_or_else(|| display_text(&Url::parse(&settings.tabs.new_tab_url).unwrap_or_else(|_| Url::parse("about:blank").unwrap())));
        Self {
            engine,
            store,
            settings,
            strip,
            toolbar: ToolbarState::new(),
            focus_handle,
            omnibox,
            omnibox_text: text,
            omnibox_focused: false,
            last_save: None,
            dirty_since: None,
            shutting_down: false,
        }
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

    fn display_text(&self, url: &Url) -> String {
        let host = omnibox::display_host(url);
        if host.is_empty() {
            url.as_str().to_string()
        } else {
            host
        }
    }

    pub fn open_url(&mut self, url: Url) {
        if let Some(tab) = self.active_tab() {
            self.engine.load(tab.webview, &url);
        } else {
            self.strip.open(&mut self.engine, &url, true);
        }
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
        if let Some(tab) = self.strip.tabs().get(index) {
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

    pub fn reload_crashed_tab(&mut self, id: TabId) {
        self.strip.reload_crashed(&mut self.engine, id);
    }

    pub fn tick(&mut self, window: &mut Window, _cx: &mut Context<Self>) {
        // layout engine view
        let bounds = window.bounds();
        let w = f32::from(bounds.size.width).max(1.0);
        let h = f32::from(bounds.size.height).max(1.0);
        let chrome_h = self.toolbar.chrome_height();
        let content_h = (h - chrome_h).max(1.0);
        let rect = ViewRect {
            x: 0.0,
            y: chrome_h,
            width: w,
            height: content_h,
        };
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
    }

    fn handle_event(&mut self, ev: &EngineEvent) {
        if let EngineEvent::Load {
            id,
            state: LoadState::Complete,
        } = ev
            && let Some(tab) = self.strip.tabs().iter().find(|t| &t.webview == id)
        {
            let http = tab.url.scheme() == "http" || tab.url.scheme() == "https";
            if http {
                let _ = self
                    .store
                    .record_visit(&tab.url, &tab.title, jiff::Timestamp::now());
            }
        }
        self.strip.apply(&mut self.engine, ev);
        // sync toolbar from active tab
        if let Some(tab) = self.active_tab().cloned() {
            self.toolbar.url_text = tab.url.to_string();
            self.toolbar.can_back = tab.can_back;
            self.toolbar.can_forward = tab.can_forward;
            self.toolbar.loading = tab.loading;
            self.toolbar.is_bookmarked = self.store.is_bookmarked(&tab.url).unwrap_or(false);
            self.toolbar.crashed = tab.crashed;
            self.toolbar.title = tab.title;
        }
        if self.strip.changed() {
            self.sync_omnibox_text();
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
        self.omnibox_text = text;
        let omnibox = self.omnibox.clone();
        let text = self.omnibox_text.clone();
        let _ = omnibox.update(self, |_state, cx| {
            let _ = cx;
            let _ = text;
            let _ = &mut ();
        });
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
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(chrome_h))
                    .w_full()
                    .child(self.render_chrome(window, cx)),
            )
            .child(div().flex_1().w_full().child(self.render_content(window, cx)))
            .on_action(cx.listener(|this: &mut Self, _: &crate::actions::New, _window, cx| {
                this.new_tab(true);
            }))
            .on_action(cx.listener(|this: &mut Self, _: &crate::actions::Close, _window, cx| {
                this.close_active();
            }))
            .on_action(
                cx.listener(|this: &mut Self, _: &crate::actions::Reopen, _window, cx| {
                    this.strip.reopen_closed(&mut this.engine);
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &crate::actions::Next, _window, cx| {
                    if let Some(i) = self.tab_index() {
                        self.activate_index(i + 1);
                    }
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &crate::actions::Prev, _window, cx| {
                    if let Some(i) = self.tab_index() {
                        if i > 0 {
                            self.activate_index(i - 1);
                        }
                    }
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &crate::actions::Select1, _window, cx| {
                    this.activate_index(0);
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &crate::actions::Select2, _window, cx| {
                    this.activate_index(1);
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &crate::actions::Select3, _window, cx| {
                    this.activate_index(2);
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &crate::actions::Select4, _window, cx| {
                    this.activate_index(3);
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &crate::actions::Select5, _window, cx| {
                    this.activate_index(4);
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &crate::actions::Select6, _window, cx| {
                    this.activate_index(5);
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &crate::actions::Select7, _window, cx| {
                    this.activate_index(6);
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &crate::actions::Select8, _window, cx| {
                    this.activate_index(7);
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &crate::actions::Select9, _window, cx| {
                    if let Some(tab) = this.strip.tabs().last() {
                        this.strip.activate(tab.id);
                    }
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &crate::actions::Focus, window, cx| {
                    this.focus_omnibox_window(window, cx);
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &crate::actions::Back, _window, cx| {
                    this.back_active();
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &crate::actions::Forward, _window, cx| {
                    this.forward_active();
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &crate::actions::Reload, _window, cx| {
                    this.reload_active();
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &crate::actions::Toggle, _window, cx| {
                    this.toggle_bookmark();
                }),
            );
    }

    fn tab_index(&self) -> Option<usize> {
        self.strip
            .active()
            .and_then(|id| self.strip.tabs().iter().position(|t| t.id == id))
    }

    fn render_chrome(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .w_full()
            .h(px(self.toolbar.chrome_height()))
            .child(self.render_tab_strip(cx))
            .child(self.render_toolbar(cx))
    }

    fn render_tab_strip(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let tabs = self.strip.tabs().to_vec();
        let mut children: Vec<gpui::Div> = Vec::new();
        for (i, tab) in tabs.iter().enumerate() {
            let tab = tab.clone();
            let index = i;
            let selected = self.strip.active() == Some(tab.id);
            let id = tab.id;
            children.push(
                div()
                    .id(("tab", index))
                    .h(px(24.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .px(px(8.))
                    .rounded_lg()
                    .cursor_pointer()
                    .when(selected, |style| style.bg(gpui::white()))
                    .on_click(cx.listener(|this: &mut Self, event: &ClickEvent, _window, cx| {
                        let _ = event;
                        if let Some(tab) = this.strip.tabs().iter().find(|t| t.id == id) {
                            this.strip.activate(tab.id);
                        }
                    }))
                    .child(self.tab_label(tab)),
            );
        }
        div()
            .flex()
            .items_center()
            .h(px(32.))
            .w_full()
            .gap(px(4.))
            .children(children)
            .child(
                div()
                    .id("new-tab")
                    .h(px(24.))
                    .w(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_lg()
                    .cursor_pointer()
                    .on_click(cx.listener(|this: &mut Self, event: &ClickEvent, _window, cx| {
                        let _ = event;
                        this.new_tab(false);
                    }))
                    .child("+"),
            )
    }

    fn tab_label(&self, tab: tabs::Tab) -> impl IntoElement {
        if tab.crashed.is_some() {
            div()
                .text_sm()
                .text_color(gpui::white())
                .child("x")
        } else if tab.title.is_empty() {
            div()
                .text_sm()
                .text_color(gpui::white())
                .child(self.display_text(&tab.url).clone())
        } else {
            div()
                .text_sm()
                .text_color(gpui::white())
                .child(self.truncate(&tab.title))
        }
    }

    fn truncate(&self, text: &str) -> String {
        let mut out = String::new();
        for ch in text.chars() {
            if out.chars().count() >= 24 {
                out.push('…');
                break;
            }
            out.push(ch);
        }
        out
    }

    fn render_toolbar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<ActiveTheme>();
        let tabs = self.strip.tabs().to_vec();
        let active_id = self.strip.active();
        let active_tab = tabs
            .iter()
            .find(|t| Some(&t.id) == active_id)
            .cloned()
            .unwrap_or(tabs.first().cloned().unwrap_or(tabs::Tab {
                id: TabId(0),
                webview: WebViewId(0),
                url: Url::parse("about:blank").unwrap(),
                title: String::new(),
                loading: false,
                can_back: false,
                can_forward: false,
                crashed: None,
            }));
        div()
            .h(px(40.))
            .w_full()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(8.))
            .child(
                div()
                    .id("back")
                    .cursor_pointer()
                    .when(self.toolbar.can_back, |s| s.opacity(1.0))
                    .on_click(cx.listener(|this: &mut Self, event: &ClickEvent, _window, cx| {
                        let _ = event;
                        let _ = cx;
                        this.back_active();
                    }))
                    .child(if self.toolbar.can_back { "<" } else { " " }),
            )
            .child(
                div()
                    .id("forward")
                    .cursor_pointer()
                    .on_click(cx.listener(|this: &mut Self, event: &ClickEvent, _window, cx| {
                        let _ = event;
                        let _ = cx;
                        this.forward_active();
                    }))
                    .child(if self.toolbar.can_forward { ">" } else { " " }),
            )
            .child(
                div()
                    .id("reload")
                    .cursor_pointer()
                    .on_click(cx.listener(|this: &mut Self, event: &ClickEvent, _window, cx| {
                        let _ = event;
                        let _ = cx;
                        this.reload_active();
                    }))
                    .child(if self.toolbar.loading { "Stop" } else { "Reload" }),
            )
            .child(
                div()
                    .id("star")
                    .cursor_pointer()
                    .on_click(cx.listener(|this: &mut Self, event: &ClickEvent, _window, cx| {
                        let _ = event;
                        let _ = cx;
                        this.toggle_bookmark();
                    }))
                    .child(if self.toolbar.is_bookmarked { "*" } else { "☆" }),
            )
            .child(
                div()
                    .id("url-display")
                    .flex_1()
                    .min_h_auto()
                    .child(if self.omnibox_focused {
                        self.omnibox_text.clone()
                    } else {
                        self.toolbar.url_text.clone()
                    }),
            )
            .child(div().w(px(140.)).text_sm().text_color(theme.text).child(self.toolbar.title.clone()))
    }

    fn render_content(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(tab) = self.active_tab()
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
                        .on_click(cx.listener(|this: &mut Self, event: &ClickEvent, _window, cx| {
                            let _ = event;
                            let _ = cx;
                            this.reload_crashed_tab(tab_id);
                        }))
                        .child("Reload"),
                );
        }
        div().size_full()
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
