use store::{DownloadState, MemoryStore, SqliteStore, Store};
use tempfile::TempDir;

fn sqlite() -> (SqliteStore, TempDir) {
    let dir = TempDir::new().unwrap();
    let s = SqliteStore::open(&dir.path().join("t.db")).unwrap();
    (s, dir)
}

fn roundtrip(store: &dyn Store) {
    let id = store
        .record_download(
            "https://example.com/file.zip",
            "/tmp/file.zip",
            0,
            Some(100),
            DownloadState::InProgress,
        )
        .unwrap();
    let mut rows = store.list_downloads().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, id);
    assert_eq!(rows[0].url, "https://example.com/file.zip");
    assert_eq!(rows[0].path, "/tmp/file.zip");
    assert_eq!(rows[0].bytes, 0);
    assert_eq!(rows[0].total, Some(100));
    assert_eq!(rows[0].state, DownloadState::InProgress);

    store
        .update_download(id, 50, Some(100), DownloadState::InProgress)
        .unwrap();
    rows = store.list_downloads().unwrap();
    assert_eq!(rows[0].bytes, 50);
    store
        .update_download(id, 100, Some(100), DownloadState::Completed)
        .unwrap();
    rows = store.list_downloads().unwrap();
    assert_eq!(rows[0].bytes, 100);
    assert_eq!(rows[0].state, DownloadState::Completed);
}

#[test]
fn downloads_table_roundtrip() {
    let (s, _d) = sqlite();
    roundtrip(&s);
    roundtrip(&MemoryStore::default());
}
