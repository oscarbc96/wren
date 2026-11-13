use std::fs;

use crate::{
    ConfigPaths, Layer, Settings, SettingsError, WorkspaceKey, default_layer, effective_for,
    parse_layers,
};

#[derive(Debug, Clone, PartialEq)]
pub enum ReloadOutcome {
    Unchanged,
    Changed,
    Rejected(SettingsError),
}

/// Effective settings loaded from disk. An invalid edit keeps the last good
/// value and records the error until the file is fixed.
pub struct SettingsStore {
    paths: ConfigPaths,
    current: Settings,
    last_error: Option<SettingsError>,
}

#[cfg(feature = "gpui")]
impl gpui::Global for SettingsStore {}

fn read_or_empty(path: &std::path::Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

impl SettingsStore {
    /// Load from disk. An invalid file yields defaults plus `last_error`.
    pub fn load(paths: ConfigPaths) -> SettingsStore {
        let mut store = SettingsStore {
            paths,
            current: Settings::default(),
            last_error: None,
        };
        store.reload();
        store
    }

    pub fn current(&self) -> &Settings {
        &self.current
    }

    pub fn last_error(&self) -> Option<&SettingsError> {
        self.last_error.as_ref()
    }

    /// Settings with the workspace's file (`workspaces/<provider>/<id>.json`) layered
    /// on top. Falls back to `current()` if the workspace file is invalid.
    pub fn for_workspace(&self, key: &WorkspaceKey) -> Settings {
        let user = read_or_empty(&self.paths.settings);
        let ws_path = self.paths.workspace_file(key);
        let ws = read_or_empty(&ws_path);
        let base = [
            default_layer(),
            Layer {
                path: &self.paths.settings,
                text: &user,
            },
        ];
        let workspace = Layer {
            path: &ws_path,
            text: &ws,
        };
        effective_for(Some(key), &base, Some(workspace)).unwrap_or_else(|_| self.current.clone())
    }

    /// Re-read `settings.json`. A missing file means defaults.
    pub fn reload(&mut self) -> ReloadOutcome {
        let user = read_or_empty(&self.paths.settings);
        let layers = [
            default_layer(),
            Layer {
                path: &self.paths.settings,
                text: &user,
            },
        ];
        match parse_layers(&layers) {
            Ok(next) => {
                let changed = next != self.current || self.last_error.is_some();
                self.current = next;
                self.last_error = None;
                if changed {
                    ReloadOutcome::Changed
                } else {
                    ReloadOutcome::Unchanged
                }
            }
            Err(e) => {
                self.last_error = Some(e.clone());
                ReloadOutcome::Rejected(e)
            }
        }
    }
}
