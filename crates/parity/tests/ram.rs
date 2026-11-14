use parity::{Pid, ProcInfo, parse_workload, tree_rss};

#[test]
fn tree_rss_sums_descendants_only() {
    let procs = vec![
        ProcInfo {
            pid: Pid::from(1usize),
            parent: None,
            rss_bytes: 10,
            phys_footprint_bytes: 100,
        },
        ProcInfo {
            pid: Pid::from(2usize),
            parent: Some(Pid::from(1usize)),
            rss_bytes: 20,
            phys_footprint_bytes: 200,
        },
        ProcInfo {
            pid: Pid::from(3usize),
            parent: Some(Pid::from(99usize)),
            rss_bytes: 30,
            phys_footprint_bytes: 300,
        },
    ];
    assert_eq!(tree_rss(Pid::from(1usize), &procs), 30);
}

#[test]
fn workload_file_parses() {
    let input = "\n# comment\nurl1\n\nurl2\n";
    assert_eq!(parse_workload(input), vec!["url1", "url2"]);
}
