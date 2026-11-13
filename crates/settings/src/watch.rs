use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use notify::RecursiveMode;
use notify_debouncer_full::{DebounceEventResult, Debouncer, RecommendedCache, new_debouncer};

use crate::{ConfigPaths, WorkspaceKey};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangedFile {
    Settings,
    Keymap,
    Theme(PathBuf),
    Workspace(WorkspaceKey),
}

/// Keeps the watcher alive; drop to stop watching.
pub struct WatchHandle {
    _debouncer: Debouncer<notify::RecommendedWatcher, RecommendedCache>,
}

fn is_temp(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_none_or(|n| n.ends_with('~') || n.ends_with(".swp") || n.ends_with(".tmp"))
}

/// Map an event path (already relative to the config root) to a changed file.
fn classify(paths: &ConfigPaths, rel: &Path) -> Option<ChangedFile> {
    if is_temp(rel) {
        return None;
    }
    let comps: Vec<_> = rel.components().map(|c| c.as_os_str()).collect();
    match comps.as_slice() {
        [f] if *f == "settings.json" => Some(ChangedFile::Settings),
        [f] if *f == "keymap.json" => Some(ChangedFile::Keymap),
        [d, f] if *d == "themes" && Path::new(f).extension().is_some_and(|e| e == "json") => {
            Some(ChangedFile::Theme(paths.themes_dir.join(f)))
        }
        [d, provider, f] if *d == "workspaces" => {
            let p = Path::new(f);
            if p.extension().is_some_and(|e| e == "json") {
                let stem = p.file_stem()?.to_str()?;
                Some(ChangedFile::Workspace(WorkspaceKey::new(
                    provider.to_str()?,
                    stem,
                )))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Watch the config root (not individual files, so atomic renames are seen),
/// debounced to 100 ms. The root is created if missing.
pub fn watch(
    paths: &ConfigPaths,
    on_change: impl Fn(ChangedFile) + Send + 'static,
) -> notify::Result<WatchHandle> {
    std::fs::create_dir_all(&paths.root)?;
    // Events carry canonical paths (e.g. /private/var on macOS).
    let canon = std::fs::canonicalize(&paths.root).unwrap_or_else(|_| paths.root.clone());
    let cfg = paths.clone();
    let mut debouncer = new_debouncer(
        Duration::from_millis(100),
        None,
        move |res: DebounceEventResult| {
            let Ok(events) = res else { return };
            let mut seen: Vec<ChangedFile> = Vec::new();
            for ev in events {
                for p in &ev.event.paths {
                    let rel = p
                        .strip_prefix(&canon)
                        .or_else(|_| p.strip_prefix(&cfg.root));
                    if let Some(c) = rel.ok().and_then(|r| classify(&cfg, r))
                        && !seen.contains(&c)
                    {
                        seen.push(c);
                    }
                }
            }
            for c in seen {
                on_change(c);
            }
        },
    )?;
    debouncer.watch(&paths.root, RecursiveMode::Recursive)?;
    Ok(WatchHandle {
        _debouncer: debouncer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_workspace_files_by_provider_and_id() {
        let paths = ConfigPaths::at("/cfg");
        assert_eq!(
            classify(&paths, Path::new("workspaces/fake/W1.json")),
            Some(ChangedFile::Workspace(WorkspaceKey::new("fake", "W1")))
        );
        assert_eq!(classify(&paths, Path::new("workspaces/W1.json")), None);
        assert_eq!(
            classify(&paths, Path::new("workspaces/fake/W1.json~")),
            None
        );
        assert_eq!(
            classify(&paths, Path::new("settings.json")),
            Some(ChangedFile::Settings)
        );
    }
}
