use gpui::{Context, Render, Window, div, prelude::*, px};
use gpui_elements::editable_text;
use themes::ActiveTheme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    Primary,
    Ghost,
}

#[derive(Clone)]
pub struct Button {
    pub label: String,
    pub variant: ButtonVariant,
}

impl Button {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            variant: ButtonVariant::Primary,
        }
    }

    pub fn primary(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            variant: ButtonVariant::Primary,
        }
    }

    pub fn ghost(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            variant: ButtonVariant::Ghost,
        }
    }

    pub fn variant(mut self, v: ButtonVariant) -> Self {
        self.variant = v;
        self
    }
}

impl Render for Button {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<ActiveTheme>();
        let (bg, fg) = match self.variant {
            ButtonVariant::Primary => (theme.accent, theme.accent_text),
            ButtonVariant::Ghost => (gpui::transparent_black(), theme.text),
        };
        div()
            .px(px(12.))
            .py(px(6.))
            .rounded(px(6.))
            .bg(bg)
            .text_color(fg)
            .child(self.label.clone())
    }
}

#[derive(Clone)]
pub struct IconButton {
    pub icon: String,
}

impl IconButton {
    pub fn new(icon: impl Into<String>) -> Self {
        Self { icon: icon.into() }
    }
}

impl Render for IconButton {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<ActiveTheme>();
        div()
            .size(px(28.))
            .flex()
            .items_center()
            .justify_center()
            .text_color(theme.text)
            .child(self.icon.clone())
    }
}

#[derive(Clone)]
pub struct TextInput {
    pub placeholder: String,
}

impl TextInput {
    pub fn new() -> Self {
        Self {
            placeholder: String::new(),
        }
    }

    pub fn with_placeholder(placeholder: impl Into<String>) -> Self {
        Self {
            placeholder: placeholder.into(),
        }
    }
}

impl Default for TextInput {
    fn default() -> Self {
        Self::new()
    }
}

impl Render for TextInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _theme = cx.global::<ActiveTheme>();
        editable_text::text_input("wren_text_input")
    }
}

#[derive(Clone)]
pub struct ListItem {
    pub label: String,
}

impl ListItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
        }
    }
}

impl Render for ListItem {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.global::<ActiveTheme>();
        div()
            .px(px(8.))
            .py(px(4.))
            .text_color(theme.text)
            .child(self.label.clone())
    }
}
