//! Wren engine abstraction: Servo adapter and `FakeEngine` for tests.

pub mod api;
pub mod fake;

pub use api::*;
pub use fake::*;
