use creds::{CredStore, MemoryCredStore};
use tempfile::TempDir;

fn build_v10_blob(password: &str, key: &[u8; 16]) -> Vec<u8> {
    use aes::cipher::{BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
    type Aes128CbcEnc = cbc::Encryptor<aes::Aes128>;
    let iv = [0x20u8; 16];
    let mut blob = b"v10".to_vec();
    blob.extend_from_slice(&iv);
    let msg = password.as_bytes();
    let mut buf = vec![0u8; msg.len() + 16];
    buf[..msg.len()].copy_from_slice(msg);
    let ct = Aes128CbcEnc::new(key.into(), (&iv).into())
        .encrypt_padded_mut::<Pkcs7>(&mut buf, msg.len())
        .unwrap();
    blob.extend_from_slice(ct);
    blob
}

fn make_login_data(
    dir: &TempDir,
    origin_url: &str,
    username: &str,
    blob: &[u8],
) -> std::path::PathBuf {
    let db_path = dir.path().join("Login Data");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute_batch(
        "CREATE TABLE logins (
            origin_url TEXT NOT NULL,
            username_value TEXT,
            password_value BLOB
        );",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO logins(origin_url, username_value, password_value) VALUES (?1, ?2, ?3)",
        rusqlite::params![origin_url, username, blob],
    )
    .unwrap();
    db_path
}

#[test]
fn chrome_decrypts_v10() {
    let key = [0x11u8; 16];
    let blob = build_v10_blob("pw123", &key);
    let out = creds::chrome::decrypt_v10(&blob, &key).unwrap();
    assert_eq!(out, "pw123");

    let dir = TempDir::new().unwrap();
    let _db = make_login_data(&dir, "https://example.com/", "alice", &blob);
    unsafe {
        std::env::set_var("WREN_CHROME_SAFE_STORAGE_KEY_HEX", hex::encode(key));
    }
    let store = MemoryCredStore::default();
    let n = creds::chrome::import(dir.path(), &store).unwrap();
    unsafe {
        std::env::remove_var("WREN_CHROME_SAFE_STORAGE_KEY_HEX");
    }
    assert_eq!(n, 1);
    let found = store.find("https://example.com").unwrap();
    assert!(
        found
            .iter()
            .any(|c| c.username == "alice" && c.password == "pw123")
    );
}

#[test]
fn chrome_import_without_key_errors() {
    let dir = TempDir::new().unwrap();
    let blob = b"v10notrealciphertextpadding".to_vec();
    let _db = make_login_data(&dir, "https://example.com/", "alice", &blob);
    unsafe {
        std::env::remove_var("WREN_CHROME_SAFE_STORAGE_KEY_HEX");
        std::env::set_var("WREN_CHROME_SAFE_STORAGE_KEY", "missing");
    }
    let store = MemoryCredStore::default();
    let res = creds::chrome::import(dir.path(), &store);
    unsafe {
        std::env::remove_var("WREN_CHROME_SAFE_STORAGE_KEY");
    }
    assert!(res.is_err(), "expected error without key, got {res:?}");
    assert!(
        store.list_origins().unwrap().is_empty(),
        "must import nothing on error"
    );
}

#[test]
fn fill_script_escapes() {
    let s = creds::fill_script("a\"b\\c", "p\n'q<");
    assert!(s.contains("\\\""), "username quote must be escaped: {s}");
    assert!(s.contains("\\\\"), "backslash must be escaped: {s}");
    assert!(s.contains("\\n"), "newline must be escaped: {s}");
    assert!(
        !s.contains("a\"b"),
        "raw quote should not appear unescaped in dangerous context"
    );
}

mod hex {
    pub fn encode(bytes: [u8; 16]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
}
