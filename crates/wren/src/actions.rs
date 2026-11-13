use gpui::actions;

actions!(
    tab,
    [
        New, Close, Reopen, Next, Prev, Select1, Select2, Select3, Select4, Select5, Select6,
        Select7, Select8, Select9
    ]
);
actions!(omnibox, [Focus]);
actions!(nav, [Back, Forward, Reload]);
actions!(bookmark, [Toggle]);

/// Map Select1..Select9 to tab index (0-based). Select9 means last tab.
pub fn select_action_index(action: &str) -> Option<usize> {
    match action {
        "tab::Select1" => Some(0),
        "tab::Select2" => Some(1),
        "tab::Select3" => Some(2),
        "tab::Select4" => Some(3),
        "tab::Select5" => Some(4),
        "tab::Select6" => Some(5),
        "tab::Select7" => Some(6),
        "tab::Select8" => Some(7),
        "tab::Select9" => Some(usize::MAX),
        _ => None,
    }
}
