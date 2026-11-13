use std::sync::Arc;
use std::time::{Duration, Instant};

use engine::{Engine, EngineEvent, LoadState, ViewRect, WebViewId};
use gpui::{App, Context, FocusHandle, Focusable, Render, Window, div, prelude::*, px};
use settings::Settings;
use store::Store;
use tabs::{TabId, TabStrip};
use url::Url;

use crate::toolbar::ToolbarState;

pub struct BrowserWindow<E: Engine + 'static, S: Store + 'static> {
    pub engine: E,
    pub store: Arc<S>,
    pub settings: Settings,
    pub strip: TabStrip,
    pub toolbar: ToolbarState,
    pub focus_handle: FocusHandle,
    last_save: Option<Instant>,
    dirty_since: Option<Instant>,
    shutting_down: bool,
}

impl<E: Engine + 'static, S: Store + 'static> BrowserWindow<E, S> {
    pub fn new(mut engine: E, store: Arc<S>, settings: Settings, cx: &mut Context<Self>) -> Self {
        let new_tab_url = Url::parse(&settings.tabs.new_tab_url)
            .unwrap_or_else(|_| Url::parse("about:blank").unwrap());
        let mut strip = TabStrip::new(settings.tabs.closed_stack);
        let mut restored = false;
        if settings.startup.restore_session
            && let Ok(Some(snap)) = store.load_session()
            && !snap.tabs.is_empty()
        {
            strip = TabStrip::restore(&mut engine, &snap, settings.tabs.closed_stack);
            restored = true;
        }
        if !restored {
            strip.open(&mut engine, &new_tab_url, true);
        }
        let focus_handle = cx.focus_handle();
        Self {
            engine,
            store,
            settings,
            strip,
            toolbar: ToolbarState::new(),
            focus_handle,
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
            let _ = self
                .store
                .record_visit(&tab.url, &tab.title, jiff::Timestamp::now());
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
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let chrome_h = self.toolbar.chrome_height();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(chrome_h))
                    .w_full()
                    .child(self.toolbar.render_view(&self.strip)),
            )
            .child(div().flex_1().w_full().child(self.render_content()))
    }
}

impl<E: Engine + 'static, S: Store + 'static> BrowserWindow<E, S> {
    fn render_content(&self) -> impl IntoElement {
        if let Some(tab) = self.active_tab()
            && let Some(reason) = &tab.crashed
        {
            return div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .child("This tab's process stopped")
                .child(reason.clone())
                .child("Reload");
        }
        div().size_full()
    }
}
