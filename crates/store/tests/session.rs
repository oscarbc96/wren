use store::{MemoryStore, SessionSnapshot, SessionTab, SqliteStore, Store};
use tempfile::TempDir;
use url::Url;

fn sqlite() -> (SqliteStore, TempDir) {
    let dir = TempDir::new().unwrap();
    let s = SqliteStore::open(&dir.path().join("t.db")).unwrap();
    (s, dir)
}

fn sample() -> SessionSnapshot {
    SessionSnapshot {
        tabs: vec![SessionTab {
            url: Url::parse("https://example.com/").unwrap(),
            title: "Example".to_string(),
        }],
        active: Some(0),
    }
}

fn run_roundtrip(store: &dyn Store) {
    assert!(store.load_session().unwrap().is_none());
    let s = sample();
    store.save_session(&s).unwrap();
    let loaded = store.load_session().unwrap().unwrap();
    assert_eq!(loaded, s);
}

#[test]
fn session_roundtrip() {
    let (s, _d) = sqlite();
    run_roundtrip(&s);
    run_roundtrip(&MemoryStore::default());
}

#[test]
fn session_save_is_atomic() {
    // sqlite
    {
        let (s, _d) = sqlite();
        let first = sample();
        s.save_session(&first).unwrap();
        let second = SessionSnapshot {
            tabs: vec![SessionTab {
                url: Url::parse("https://other.com/").unwrap(),
                title: "Other".to_string(),
            }],
            active: Some(0),
        };
        s.fail_next_session_save();
        assert!(s.save_session(&second).is_err());
        let loaded = s.load_session().unwrap().unwrap();
        assert_eq!(loaded, first);
    }
    // memory
    {
        let s = MemoryStore::default();
        let first = sample();
        s.save_session(&first).unwrap();
        let second = SessionSnapshot {
            tabs: vec![SessionTab {
                url: Url::parse("https://other.com/").unwrap(),
                title: "Other".to_string(),
            }],
            active: Some(0),
        };
        s.fail_next_session_save();
        assert!(s.save_session(&second).is_err());
        let loaded = s.load_session().unwrap().unwrap();
        assert_eq!(loaded, first);
    }
}
