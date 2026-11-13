use parity::{WptReport, dirs_sorted_by_gap, pass_rates};

#[test]
fn pass_rate_counts_subtests() {
    let report: WptReport =
        serde_json::from_str(include_str!("fixtures/wptreport-servo.json")).unwrap();
    let rates = pass_rates(&report);
    assert_eq!(rates["dom"].passed, 2);
    assert_eq!(rates["dom"].total, 3);
}

#[test]
fn diff_lists_dirs_sorted_by_gap() {
    let servo: WptReport =
        serde_json::from_str(include_str!("fixtures/wptreport-servo.json")).unwrap();
    let chrome: WptReport =
        serde_json::from_str(include_str!("fixtures/wptreport-chrome.json")).unwrap();
    let dirs = dirs_sorted_by_gap(&servo, &chrome);
    assert_eq!(dirs, vec!["dom", "css"]);
}
