use creds::{CredStore, Credential, KeychainStore};

#[test]
#[ignore = "may prompt for Keychain access in non-interactive runs; uses service wren.passwords.test"]
fn keychain_roundtrip() {
    let store = KeychainStore::new("wren.passwords.test");
    let c = Credential {
        origin: "https://example.com".to_string(),
        username: "alice".to_string(),
        password: "secret".to_string(),
    };
    // clean up from previous runs
    let _ = store.delete(&c.origin, &c.username);

    store.save(&c).unwrap();
    let found = store.find(&c.origin).unwrap();
    assert!(found.iter().any(|x| x == &c));

    store.delete(&c.origin, &c.username).unwrap();
    let found = store.find(&c.origin).unwrap();
    assert!(!found.iter().any(|x| x.username == c.username));
}
