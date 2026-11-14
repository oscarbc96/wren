use crate::manifest::{ExtensionId, Manifest};
use std::path::PathBuf;
use store::Store;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("store: {0}")]
    Store(#[from] store::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("not found: {0}")]
    NotFound(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledExtension {
    pub id: ExtensionId,
    pub manifest: Manifest,
    pub path: PathBuf,
    pub enabled: bool,
}

pub struct Registry<'a> {
    store: &'a dyn Store,
}

impl<'a> Registry<'a> {
    pub fn new(store: &'a dyn Store) -> Self {
        Self { store }
    }

    pub fn install(
        &self,
        id: &ExtensionId,
        manifest: &Manifest,
        path: &std::path::Path,
    ) -> Result<(), RegistryError> {
        let manifest_json = serde_json::to_string(manifest)?;
        self.store.upsert_extension(
            id.as_str(),
            &manifest.version,
            &manifest_json,
            &path.to_string_lossy(),
            true,
        )?;
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<InstalledExtension>, RegistryError> {
        let rows = self.store.list_extensions()?;
        rows.into_iter().map(row_to_installed).collect()
    }

    pub fn get(&self, id: &ExtensionId) -> Result<Option<InstalledExtension>, RegistryError> {
        let row = self.store.get_extension(id.as_str())?;
        row.map(row_to_installed).transpose()
    }

    pub fn set_enabled(&self, id: &ExtensionId, enabled: bool) -> Result<(), RegistryError> {
        self.store.set_extension_enabled(id.as_str(), enabled)?;
        Ok(())
    }

    pub fn remove(&self, id: &ExtensionId) -> Result<(), RegistryError> {
        if let Some(row) = self.store.get_extension(id.as_str())? {
            let path = PathBuf::from(&row.path);
            self.store.remove_extension(id.as_str())?;
            if path.exists() {
                std::fs::remove_dir_all(path)?;
            }
        }
        Ok(())
    }

    pub fn installed_version(&self, id: &ExtensionId) -> Result<Option<String>, RegistryError> {
        Ok(self.store.get_extension(id.as_str())?.map(|r| r.version))
    }
}

fn row_to_installed(r: store::ExtensionRow) -> Result<InstalledExtension, RegistryError> {
    let manifest: Manifest = serde_json::from_str(&r.manifest_json)?;
    Ok(InstalledExtension {
        id: ExtensionId(r.id),
        manifest,
        path: PathBuf::from(r.path),
        enabled: r.enabled,
    })
}
