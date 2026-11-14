use extensions::manifest::{Manifest, ManifestError, MessageEntry, Messages};
use std::collections::HashMap;

#[test]
fn parses_mv3_manifest_with_messages() {
    let json = r#"{
        "manifest_version": 3,
        "name": "__MSG_extName__",
        "version": "1.2.3",
        "description": "__MSG_desc__",
        "default_locale": "en",
        "permissions": ["storage"]
    }"#;
    let mut map = HashMap::new();
    map.insert(
        "extName".to_string(),
        MessageEntry {
            message: "Hello Ext".to_string(),
        },
    );
    map.insert(
        "desc".to_string(),
        MessageEntry {
            message: "A description".to_string(),
        },
    );
    let messages = Messages { map };
    let m = Manifest::parse(json, Some(&messages)).expect("parse");
    assert_eq!(m.manifest_version, 3);
    assert_eq!(m.name, "Hello Ext");
    assert_eq!(m.version, "1.2.3");
    assert_eq!(m.description, "A description");
    assert_eq!(m.permissions, vec!["storage".to_string()]);
}

#[test]
fn mv2_refused() {
    let json = r#"{
        "manifest_version": 2,
        "name": "Old",
        "version": "1.0"
    }"#;
    let err = Manifest::parse(json, None).unwrap_err();
    assert!(matches!(err, ManifestError::Mv2));
}
