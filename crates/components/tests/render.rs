use components::{Button, IconButton, ListItem, TextInput};
use gpui::TestAppContext;

fn setup(cx: &mut TestAppContext) {
    cx.update(themes::init);
}

#[gpui::test]
fn button_renders(cx: &mut TestAppContext) {
    setup(cx);
    let (_view, cxw) = cx.add_window_view(|_, _| Button::new("ok"));
    cxw.run_until_parked();
}

#[gpui::test]
fn icon_button_renders(cx: &mut TestAppContext) {
    setup(cx);
    let (_view, cxw) = cx.add_window_view(|_, _| IconButton::new("x"));
    cxw.run_until_parked();
}

#[gpui::test]
fn text_input_renders(cx: &mut TestAppContext) {
    setup(cx);
    let (_view, cxw) = cx.add_window_view(|_, _| TextInput::new());
    cxw.run_until_parked();
}

#[gpui::test]
fn list_item_renders(cx: &mut TestAppContext) {
    setup(cx);
    let (_view, cxw) = cx.add_window_view(|_, _| ListItem::new("item"));
    cxw.run_until_parked();
}
