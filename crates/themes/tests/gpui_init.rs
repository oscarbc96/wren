use gpui::TestAppContext;
use themes::ActiveTheme;

#[gpui::test]
fn init_sets_active(cx: &mut TestAppContext) {
    cx.update(themes::init);
    let name = cx.read_global::<ActiveTheme, _>(|at, _| at.theme.name.clone());
    assert!(!name.is_empty());
}
