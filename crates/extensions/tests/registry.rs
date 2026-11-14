use extensions::manifest::{ExtensionId, Manifest};
use extensions::registry::Registry;
use store::SqliteStore;
use tempfile::TempDir;

#[test]
fn registry_roundtrip() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("wren.db");
    let store = SqliteStore::open(&db).unwrap();
    let reg = Registry::new(&store);

    let id = ExtensionId("abcdefghijklmnopabcdefghijklmnop".to_string());
    let manifest = Manifest {
        name: "Test Ext".to_string(),
        version: "1.0.0".to_string(),
        manifest_version: 3,
        ..Default::default()
    };
    let ext_path = dir.path().join("ext");
    std::fs::create_dir_all(&ext_path).unwrap();

    reg.install(&id, &manifest, &ext_path).unwrap();
    assert_eq!(
        reg.installed_version(&id).unwrap().as_deref(),
        Some("1.0.0")
    );

    let got = reg.get(&id).unwrap().expect("present");
    assert_eq!(got.id, id);
    assert_eq!(got.manifest.name, "Test Ext");
    assert!(got.enabled);

    let list = reg.list().unwrap();
    assert_eq!(list.len(), 1);

    reg.set_enabled(&id, false).unwrap();
    assert!(!reg.get(&id).unwrap().unwrap().enabled);

    reg.remove(&id).unwrap();
    assert!(reg.get(&id).unwrap().is_none());
    assert!(reg.list().unwrap().is_empty());
    assert!(!ext_path.exists(), "remove should delete files");
}
