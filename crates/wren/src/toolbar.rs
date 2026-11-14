use gpui::{div, prelude::*, px};
use tabs::TabStrip;

#[derive(Clone, Default)]
pub struct ToolbarState {
    pub url_text: String,
    pub can_back: bool,
    pub can_forward: bool,
    pub loading: bool,
    pub is_bookmarked: bool,
    pub focus_omnibox: bool,
    pub crashed: Option<String>,
    pub title: String,
    pub devtools_indicator: Option<String>,
}

impl ToolbarState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn chrome_height(&self) -> f32 {
        78.0
    }

    pub fn focus_omnibox(&mut self) {
        self.focus_omnibox = true;
    }

    pub fn render_view(&self, strip: &TabStrip) -> impl IntoElement {
        let tabs_bar = div()
            .h(px(32.))
            .w_full()
            .flex()
            .items_center()
            .px(px(8.))
            .child(format!("{} tabs", strip.tabs().len()));
        let back = if self.can_back { "<" } else { "-" };
        let fwd = if self.can_forward { ">" } else { "-" };
        let mut toolbar = div()
            .h(px(40.))
            .w_full()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(8.))
            .child(back)
            .child(fwd)
            .child(if self.loading { "Stop" } else { "Reload" })
            .child(div().flex_1().child(if self.url_text.is_empty() {
                "about:blank".to_string()
            } else {
                self.url_text.clone()
            }));
        if let Some(indicator) = &self.devtools_indicator {
            toolbar = toolbar.child(indicator.clone());
        }
        let toolbar = toolbar.child(if self.is_bookmarked { "*" } else { "☆" });
        div().flex().flex_col().child(tabs_bar).child(toolbar)
    }
}
