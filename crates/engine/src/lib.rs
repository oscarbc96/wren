//! Wren engine abstraction: Servo adapter and `FakeEngine` for tests.

pub mod api;
pub mod fake;
pub mod memory_relief;
mod servo_engine;

pub use api::*;
pub use fake::*;
pub use servo_engine::{ServoEngine, content_process_main, content_process_token, to_servo_input};
