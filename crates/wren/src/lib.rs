pub mod actions;
pub mod app;
pub mod cdp_bridge;
pub mod cdp_domains;
pub mod cli;
pub mod downloads;
pub mod layout;
pub mod memory_pressure;
pub mod open_in_chrome;
pub mod screenshot;
pub mod task_manager;
pub mod toolbar;
pub mod window;
pub use layout::content_rect;

pub fn run() {
    app::run();
}
