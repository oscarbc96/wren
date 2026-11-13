//! Zed-style keymap files: JSONC array of `{context, bindings}` blocks.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::SettingsError;

/// One keymap block as written in a keymap file.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct KeymapBlock {
    #[serde(default)]
    pub context: Option<String>,
    #[serde(default)]
    pub bindings: BTreeMap<String, String>,
}

/// A resolved, valid binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub keystrokes: String,
    pub action: String,
    pub context: Option<String>,
}

/// A binding that was dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeymapWarning {
    BadKeystroke {
        keystrokes: String,
        context: Option<String>,
    },
    UnknownAction {
        action: String,
    },
}

/// Parse a keymap file (JSONC). An empty file is an empty keymap.
pub fn parse_keymap(path: &Path, text: &str) -> Result<Vec<KeymapBlock>, SettingsError> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let value = crate::load::parse_jsonc::<serde_json::Value>(text).map_err(|e| {
        let (line, col, message) = crate::load::split_position(&e);
        SettingsError::Parse {
            path: path.to_path_buf(),
            line,
            col,
            message,
        }
    })?;
    serde_path_to_error::deserialize(value).map_err(|e| SettingsError::Type {
        path: PathBuf::from(path),
        pointer: pointer(e.path()),
        message: e.inner().to_string(),
    })
}

fn pointer(path: &serde_path_to_error::Path) -> String {
    use serde_path_to_error::Segment;
    let mut out = String::new();
    for seg in path.iter() {
        out.push('/');
        match seg {
            Segment::Seq { index } => out.push_str(&index.to_string()),
            Segment::Map { key } | Segment::Enum { variant: key } => out.push_str(key),
            Segment::Unknown => out.push('?'),
        }
    }
    out
}

#[cfg(feature = "gpui")]
fn valid_keystrokes(keystrokes: &str) -> bool {
    let mut any = false;
    for chord in keystrokes.split_whitespace() {
        any = true;
        if gpui::Keystroke::parse(chord).is_err() {
            return false;
        }
    }
    any
}

#[cfg(not(feature = "gpui"))]
fn valid_keystrokes(keystrokes: &str) -> bool {
    !keystrokes.trim().is_empty()
}

/// Merge default and user blocks. A user binding for the same keystrokes and
/// context replaces the default. Invalid bindings are dropped with a warning.
pub fn resolve_bindings(
    default: &[KeymapBlock],
    user: &[KeymapBlock],
    known_actions: &dyn Fn(&str) -> bool,
) -> (Vec<Binding>, Vec<KeymapWarning>) {
    let mut bindings: Vec<Binding> = Vec::new();
    let mut warnings = Vec::new();
    for block in default.iter().chain(user) {
        for (keystrokes, action) in &block.bindings {
            if !valid_keystrokes(keystrokes) {
                warnings.push(KeymapWarning::BadKeystroke {
                    keystrokes: keystrokes.clone(),
                    context: block.context.clone(),
                });
                continue;
            }
            if !known_actions(action) {
                warnings.push(KeymapWarning::UnknownAction {
                    action: action.clone(),
                });
                continue;
            }
            let binding = Binding {
                keystrokes: keystrokes.clone(),
                action: action.clone(),
                context: block.context.clone(),
            };
            match bindings
                .iter_mut()
                .find(|b| b.keystrokes == binding.keystrokes && b.context == binding.context)
            {
                Some(slot) => *slot = binding,
                None => bindings.push(binding),
            }
        }
    }
    (bindings, warnings)
}
