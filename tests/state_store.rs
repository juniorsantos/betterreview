use std::collections::{BTreeMap, BTreeSet};

use betterreview::{
    domain::{
        ChangeRequestKey, CommitOid, DiffPosition, DiffSelection, DiffSide, ProviderKind, RepoPath,
        ReviewOutcome, SubmitMode,
    },
    state::{
        ContentIdentity, EditorSnapshot, FileProgress, JsonSessionStore, PendingSubmit, ReviewSync,
        SESSION_SCHEMA_VERSION, SessionAccess, SessionSnapshot, SessionStore, StateError,
        StatePaths,
    },
};
use tempfile::TempDir;
use time::{Duration, OffsetDateTime};

fn key() -> ChangeRequestKey {
    ChangeRequestKey {
        provider: ProviderKind::GitLab,
        host: "git.example.com".into(),
        repository: "group/project".into(),
        number: 42,
    }
}

fn selection() -> DiffSelection {
    DiffSelection {
        start: DiffPosition {
            path: RepoPath("src/lib.rs".into()),
            side: DiffSide::Right,
            line: 8,
            hunk: 1,
            old_line: None,
            new_line: None,
        },
        end: DiffPosition {
            path: RepoPath("src/lib.rs".into()),
            side: DiffSide::Right,
            line: 10,
            hunk: 1,
            old_line: None,
            new_line: None,
        },
    }
}

fn snapshot(updated_at: OffsetDateTime) -> SessionSnapshot {
    let path = RepoPath("src/lib.rs".into());
    SessionSnapshot {
        schema_version: SESSION_SCHEMA_VERSION,
        key: key(),
        base: CommitOid("base".into()),
        head: CommitOid("head".into()),
        active_file: Some(path.clone()),
        cursor_row: 9,
        scroll_row: 3,
        files: BTreeMap::from([(
            path.clone(),
            FileProgress {
                identity: ContentIdentity {
                    path: path.clone(),
                    base_blob: Some("base-blob".into()),
                    head_blob: Some("head-blob".into()),
                },
                reviewed: true,
                reviewed_hunks: Default::default(),
                sync: ReviewSync::LocalOnly,
            },
        )]),
        editor: Some(EditorSnapshot {
            lines: vec!["uma observacao".into(), "sem credenciais".into()],
            cursor_row: 1,
            grapheme_col: 4,
            original_head: CommitOid("head".into()),
            path,
            selection: selection(),
            stale: false,
        }),
        pending_submit: Some(PendingSubmit {
            summary: "pronto".into(),
            outcome: ReviewOutcome::Approve,
            mode: SubmitMode::Full,
        }),
        updated_at,
    }
}

fn store() -> (TempDir, JsonSessionStore) {
    let root = tempfile::tempdir().unwrap();
    let store = JsonSessionStore::new(StatePaths::new(root.path().join("state"))).unwrap();
    (root, store)
}

#[test]
fn round_trips_session_without_credentials() {
    let (_root, store) = store();
    let snapshot = snapshot(OffsetDateTime::UNIX_EPOCH);
    let mut handle = store.open_writable(&snapshot.key).unwrap();
    handle.save(&snapshot).unwrap();

    let json = std::fs::read_to_string(handle.path()).unwrap();
    assert!(!json.to_ascii_lowercase().contains("token"));
    assert!(!json.to_ascii_lowercase().contains("authorization"));
    assert_eq!(store.load(&snapshot.key).unwrap(), Some(snapshot));
}

#[test]
fn second_writer_receives_read_only_access() {
    let (_root, store) = store();
    let snapshot = snapshot(OffsetDateTime::UNIX_EPOCH);
    let mut writer = store.open_writable(&snapshot.key).unwrap();
    writer.save(&snapshot).unwrap();

    let access = store.open(&snapshot.key).unwrap();
    match access {
        SessionAccess::ReadOnly {
            snapshot: loaded, ..
        } => assert_eq!(loaded, Some(snapshot)),
        SessionAccess::ReadWrite(_) => panic!("second writer acquired the session lock"),
    }
}

#[test]
fn session_filename_is_a_stable_sha256_digest() {
    let (_root, store) = store();
    let first = store.paths().session_path(&key()).unwrap();
    let second = store.paths().session_path(&key()).unwrap();
    let name = first.file_stem().unwrap().to_str().unwrap();

    assert_eq!(first, second);
    assert_eq!(name.len(), 64);
    assert!(name.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert!(!first.to_string_lossy().contains("group"));
}

#[test]
fn save_atomically_replaces_existing_snapshot() {
    let (_root, store) = store();
    let mut first = snapshot(OffsetDateTime::UNIX_EPOCH);
    let mut handle = store.open_writable(&first.key).unwrap();
    handle.save(&first).unwrap();
    first.cursor_row = 77;
    handle.save(&first).unwrap();

    assert_eq!(store.load(&first.key).unwrap(), Some(first));
    let entries = std::fs::read_dir(store.paths().root())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|path| {
        matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("json" | "lock")
        )
    }));
}

#[cfg(unix)]
#[test]
fn session_file_permissions_are_private() {
    use std::os::unix::fs::PermissionsExt;

    let (_root, store) = store();
    let snapshot = snapshot(OffsetDateTime::UNIX_EPOCH);
    let mut handle = store.open_writable(&snapshot.key).unwrap();
    handle.save(&snapshot).unwrap();

    let mode = std::fs::metadata(handle.path())
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn list_is_sorted_by_most_recent_update() {
    let (_root, store) = store();
    let older = snapshot(OffsetDateTime::UNIX_EPOCH);
    let mut newer = older.clone();
    newer.key.number = 99;
    newer.updated_at += Duration::hours(2);

    store
        .open_writable(&older.key)
        .unwrap()
        .save(&older)
        .unwrap();
    store
        .open_writable(&newer.key)
        .unwrap()
        .save(&newer)
        .unwrap();

    let summaries = store.list().unwrap();
    assert_eq!(summaries.len(), 2);
    assert_eq!(summaries[0].key, newer.key);
    assert_eq!(summaries[1].key, older.key);
}

#[test]
fn schema_mismatch_is_visible() {
    let (_root, store) = store();
    let mut snapshot = snapshot(OffsetDateTime::UNIX_EPOCH);
    snapshot.schema_version = SESSION_SCHEMA_VERSION + 1;
    let path = store.paths().session_path(&snapshot.key).unwrap();
    std::fs::write(path, serde_json::to_vec(&snapshot).unwrap()).unwrap();

    assert!(matches!(
        store.load(&snapshot.key),
        Err(StateError::SchemaMismatch { found, .. }) if found == SESSION_SCHEMA_VERSION + 1
    ));
}

#[test]
fn session_written_before_hunk_progress_loads_with_no_reviewed_hunks() {
    let (_root, store) = store();
    let path = store.paths().session_path(&key()).unwrap();
    let mut json = serde_json::to_value(snapshot(OffsetDateTime::UNIX_EPOCH)).unwrap();
    for progress in json["files"].as_object_mut().unwrap().values_mut() {
        assert!(
            progress
                .as_object_mut()
                .unwrap()
                .remove("reviewed_hunks")
                .is_some()
        );
    }
    std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();

    let loaded = store.load(&key()).unwrap().expect("session loads");

    assert_eq!(loaded.schema_version, SESSION_SCHEMA_VERSION);
    let progress = loaded.files.values().next().unwrap();
    assert!(progress.reviewed);
    assert!(progress.reviewed_hunks.is_empty());
}

#[test]
fn reviewed_hunks_round_trip_through_the_session_file() {
    let (_root, store) = store();
    let mut snapshot = snapshot(OffsetDateTime::UNIX_EPOCH);
    for progress in snapshot.files.values_mut() {
        progress.reviewed_hunks = BTreeSet::from([0, 2]);
    }
    let mut handle = store.open_writable(&snapshot.key).unwrap();
    handle.save(&snapshot).unwrap();

    assert_eq!(store.load(&snapshot.key).unwrap(), Some(snapshot));
}

#[test]
fn corrupt_json_is_quarantined() {
    let (_root, store) = store();
    let path = store.paths().session_path(&key()).unwrap();
    std::fs::write(&path, "{definitely-not-json").unwrap();

    let error = store.load(&key()).unwrap_err();
    match error {
        StateError::CorruptSession {
            original,
            quarantined,
            ..
        } => {
            assert_eq!(original, path);
            assert!(!original.exists());
            assert!(quarantined.exists());
            assert!(quarantined.to_string_lossy().contains(".corrupt-"));
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn a_stray_file_in_the_state_directory_is_ignored_instead_of_quarantined() {
    let (_root, store) = store();
    let snapshot = snapshot(OffsetDateTime::UNIX_EPOCH);
    store
        .open_writable(&snapshot.key)
        .unwrap()
        .save(&snapshot)
        .unwrap();
    let stray = store.paths().root().join("config.json");
    std::fs::write(&stray, br#"{"diff_layout":"split"}"#).unwrap();

    let summaries = store.list().unwrap();

    assert_eq!(summaries.len(), 1, "only the real session is listed");
    assert!(stray.exists(), "the config file survives listing");
    assert_eq!(
        std::fs::read_to_string(&stray).unwrap(),
        r#"{"diff_layout":"split"}"#,
        "and is left untouched"
    );
}
