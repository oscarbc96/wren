pub mod actions;
pub mod app;
pub mod cli;
pub mod layout;
pub mod open_in_chrome;
pub mod screenshot;
pub mod task_manager;
pub mod toolbar;
pub mod window;
pub use layout::content_rect;

pub fn run() {
    app::run();
}
