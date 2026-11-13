use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::WorkspaceKey;

const TEMPLATE: &str = include_str!("../../../assets/settings/initial_user_settings.jsonc");

/// Locations of every user-editable config file and directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigPaths {
    pub root: PathBuf,
    pub settings: PathBuf,
    pub keymap: PathBuf,
    pub themes_dir: PathBuf,
    pub workspaces_dir: PathBuf,
    pub plugins_dir: PathBuf,
}

impl ConfigPaths {
    /// Paths under an explicit root.
    pub fn at(root: impl Into<PathBuf>) -> ConfigPaths {
        let root = root.into();
        ConfigPaths {
            settings: root.join("settings.json"),
            keymap: root.join("keymap.json"),
            themes_dir: root.join("themes"),
            workspaces_dir: root.join("workspaces"),
            plugins_dir: root.join("plugins"),
            root,
        }
    }

    /// Per-workspace settings file: `workspaces/<provider>/<id>.json`. Characters
    /// that could leave the directory are replaced by `_`.
    pub fn workspace_file(&self, key: &WorkspaceKey) -> PathBuf {
        let safe = |s: &str| {
            let s: String = s
                .chars()
                .map(|c| if matches!(c, '/' | '\\') { '_' } else { c })
                .collect();
            if s == ".." || s == "." {
                "_".to_string()
            } else {
                s
            }
        };
        self.workspaces_dir
            .join(safe(&key.provider.0))
            .join(format!("{}.json", safe(&key.id.0)))
    }

    /// `WREN_CONFIG_DIR` if set and non-empty, else the platform config dir
    /// (`~/.config/wren`, `%APPDATA%\wren`, `$XDG_CONFIG_HOME/wren`).
    pub fn discover() -> ConfigPaths {
        if let Some(dir) = std::env::var_os("WREN_CONFIG_DIR").filter(|v| !v.is_empty()) {
            return Self::at(dir);
        }
        Self::at(platform_root())
    }

    /// Create the directories and write `settings.json` (commented template)
    /// and `keymap.json` (`[]`), each only when missing.
    pub fn ensure_exists(&self) -> io::Result<()> {
        for dir in [
            &self.root,
            &self.themes_dir,
            &self.workspaces_dir,
            &self.plugins_dir,
        ] {
            fs::create_dir_all(dir)?;
        }
        write_if_missing(&self.settings, TEMPLATE)?;
        write_if_missing(&self.keymap, "[]")?;
        Ok(())
    }
}

fn write_if_missing(path: &Path, text: &str) -> io::Result<()> {
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut f) => io::Write::write_all(&mut f, text.as_bytes()),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(e),
    }
}

fn platform_root() -> PathBuf {
    if cfg!(windows) {
        return dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("wren");
    }
    if let Some(x) = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(x).join("wren");
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("wren")
}

/// `WREN_DATA_DIR` if set and non-empty, else the platform data dir joined with `wren`.
pub fn data_dir() -> PathBuf {
    env_or("WREN_DATA_DIR", dirs::data_dir)
}

/// `WREN_CACHE_DIR` if set and non-empty, else the platform cache dir joined with `wren`.
pub fn cache_dir() -> PathBuf {
    env_or("WREN_CACHE_DIR", dirs::cache_dir)
}

fn env_or(var: &str, platform: fn() -> Option<PathBuf>) -> PathBuf {
    if let Some(dir) = std::env::var_os(var).filter(|v| !v.is_empty()) {
        return PathBuf::from(dir);
    }
    platform()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("wren")
}
