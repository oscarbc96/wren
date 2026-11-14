pub mod actions;
pub mod app;
pub mod cli;
pub mod open_in_chrome;
pub mod task_manager;
pub mod toolbar;
pub mod window;

pub fn run() {
    app::run();
}
