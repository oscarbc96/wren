use creds::{CredStore, MemoryCredStore};
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn csv_import_chrome_format() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "name,url,username,password").unwrap();
    writeln!(f, "Example,https://example.com/login,alice,secret1").unwrap();
    writeln!(f, "Example,https://example.com/other,bob,secret2").unwrap();
    f.flush().unwrap();

    let store = MemoryCredStore::default();
    let n = creds::csv::import(f.path(), &store).unwrap();
    assert_eq!(n, 2);
    let found = store.find("https://example.com").unwrap();
    assert!(
        found
            .iter()
            .any(|c| c.username == "alice" && c.password == "secret1")
    );
    assert!(
        found
            .iter()
            .any(|c| c.username == "bob" && c.password == "secret2")
    );
}
