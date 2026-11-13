pub mod classify;
pub mod display;
pub mod suggest;

pub use classify::{Input, classify, search_url};
pub use display::display_host;
pub use suggest::{Suggestion, SuggestionKind, suggest};
