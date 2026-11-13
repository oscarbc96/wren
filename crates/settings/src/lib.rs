//! Settings: typed model, layered JSONC loading and schema.
//! File store, first run and watching.

mod edit;
mod error;
mod keymap;
mod load;
mod model;
pub mod paths;
mod schema;
mod store;
mod watch;

pub use edit::update_value;
pub use error::SettingsError;
pub use keymap::{Binding, KeymapBlock, KeymapWarning, parse_keymap, resolve_bindings};
pub use load::{Layer, default_layer, effective_for, parse_layers};
pub use model::*;
pub use paths::ConfigPaths;
pub use schema::{settings_schema, unknown_keys};
pub use store::{ReloadOutcome, SettingsStore};
pub use watch::{ChangedFile, WatchHandle, watch};
