use std::path::PathBuf;

#[test]
fn schema_snapshot() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/settings.schema.json");
    let actual = settings::settings_schema();
    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        let mut text = serde_json::to_string_pretty(&actual).unwrap();
        text.push('\n');
        std::fs::write(&path, text).unwrap();
    }
    let expected: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&path).expect("snapshot missing; run with UPDATE_SNAPSHOTS=1"),
    )
    .unwrap();
    assert_eq!(actual, expected);
}
