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
actions!(passwords, [Fill, ImportChrome, ImportCsv]);
actions!(app, [Quit]);

/// Maps a `tab::SelectN` action to its 0-based tab index. `tab::Select9` means
/// the last tab, which the caller resolves from the strip length.
pub fn select_action_index(action: &str) -> Option<usize> {
    let n = action.strip_prefix("tab::Select")?.parse::<usize>().ok()?;
    match n {
        1 => Some(0),
        2 => Some(1),
        3 => Some(2),
        4 => Some(3),
        5 => Some(4),
        6 => Some(5),
        7 => Some(6),
        8 => Some(7),
        _ => None,
    }
}
