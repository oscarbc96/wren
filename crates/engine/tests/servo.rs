use engine::content_process_token;

#[test]
fn content_process_token_parses() {
    let args: Vec<String> = ["wren", "--content-process", "abc"]
        .into_iter()
        .map(String::from)
        .collect();
    assert_eq!(content_process_token(&args), Some("abc".to_string()));

    let args: Vec<String> = ["wren"].into_iter().map(String::from).collect();
    assert_eq!(content_process_token(&args), None);

    let args: Vec<String> = ["wren", "--content-process"]
        .into_iter()
        .map(String::from)
        .collect();
    assert_eq!(content_process_token(&args), None);
}

/// Manual steps to verify a killed content process only crashes its tab:
/// 1. Run: `cargo run -p wren --example servo_in_gpui -- --two --exit-after 30`
/// 2. In another shell, find child content processes: `pgrep -P <pid>`
/// 3. Kill one content process: `kill -9 <child-pid>`
/// 4. Expect `CRASHED` for one webview while the other webview keeps reporting events.
#[test]
#[ignore]
fn kill_one_content_process_keeps_others() {}
