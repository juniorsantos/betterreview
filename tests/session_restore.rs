use std::collections::{BTreeMap, BTreeSet};

use betterreview::{
    domain::{
        ChangeRequestKey, ChangedFile, CommitOid, DiffPosition, DiffSelection, DiffSide,
        FileStatus, PatchAvailability, ProviderCapabilities, ProviderKind, ProviderSnapshot,
        RepoPath,
    },
    state::{
        ContentIdentity, EditorSnapshot, FileProgress, RestoreNotice, ReviewSync,
        SESSION_SCHEMA_VERSION, SessionRestorer, SessionSnapshot,
    },
};
use time::OffsetDateTime;

fn key(provider: ProviderKind) -> ChangeRequestKey {
    ChangeRequestKey {
        provider,
        host: "code.example.com".into(),
        repository: "group/project".into(),
        number: 7,
    }
}

fn changed_file(
    path: &str,
    base_blob: Option<&str>,
    head_blob: Option<&str>,
    remotely_reviewed: Option<bool>,
) -> ChangedFile {
    ChangedFile {
        path: RepoPath(path.into()),
        previous_path: None,
        status: FileStatus::Modified,
        additions: 2,
        deletions: 1,
        patch: PatchAvailability::Available(
            "@@ -1,2 +1,3 @@\n old\n-old line\n+new line\n+another\n".into(),
        ),
        base_blob: base_blob.map(str::to_owned),
        head_blob: head_blob.map(str::to_owned),
        remotely_reviewed,
    }
}

fn provider_snapshot(
    provider: ProviderKind,
    head: &str,
    files: Vec<ChangedFile>,
) -> ProviderSnapshot {
    ProviderSnapshot {
        key: key(provider),
        title: "Review".into(),
        author: "dev".into(),
        web_url: "https://code.example.com/group/project/review/7".into(),
        base: CommitOid("base-new".into()),
        head: CommitOid(head.into()),
        files,
        threads: Vec::new(),
        drafts: Vec::new(),
        capabilities: ProviderCapabilities::all_supported(),
    }
}

fn progress(path: &str, base: Option<&str>, head: Option<&str>, sync: ReviewSync) -> FileProgress {
    FileProgress {
        identity: ContentIdentity {
            path: RepoPath(path.into()),
            base_blob: base.map(str::to_owned),
            head_blob: head.map(str::to_owned),
        },
        reviewed: true,
        reviewed_hunks: BTreeSet::from([0]),
        sync,
    }
}

fn saved_session(provider: ProviderKind, head: &str) -> SessionSnapshot {
    let path = RepoPath("src/lib.rs".into());
    SessionSnapshot {
        schema_version: SESSION_SCHEMA_VERSION,
        key: key(provider),
        base: CommitOid("base-old".into()),
        head: CommitOid(head.into()),
        active_file: Some(path.clone()),
        cursor_row: 3,
        scroll_row: 2,
        files: BTreeMap::from([(
            path.clone(),
            progress(
                "src/lib.rs",
                Some("base-1"),
                Some("head-1"),
                ReviewSync::Synced,
            ),
        )]),
        editor: Some(EditorSnapshot {
            lines: vec!["keep this text".into()],
            cursor_row: 0,
            grapheme_col: 4,
            original_head: CommitOid(head.into()),
            path: path.clone(),
            selection: DiffSelection {
                start: DiffPosition {
                    path: path.clone(),
                    side: DiffSide::Right,
                    line: 2,
                    hunk: 1,
                    old_line: None,
                    new_line: None,
                },
                end: DiffPosition {
                    path,
                    side: DiffSide::Right,
                    line: 2,
                    hunk: 1,
                    old_line: None,
                    new_line: None,
                },
            },
            stale: false,
        }),
        pending_submit: None,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    }
}

#[test]
fn unchanged_head_restores_exact_position_and_editor() {
    let saved = saved_session(ProviderKind::GitHub, "head-a");
    let fresh = provider_snapshot(
        ProviderKind::GitHub,
        "head-a",
        vec![changed_file(
            "src/lib.rs",
            Some("base-1"),
            Some("head-1"),
            Some(true),
        )],
    );

    let restored = SessionRestorer::restore(saved.clone(), &fresh);

    assert_eq!(restored.snapshot.active_file, saved.active_file);
    assert_eq!(restored.snapshot.cursor_row, saved.cursor_row);
    assert_eq!(restored.snapshot.scroll_row, saved.scroll_row);
    assert_eq!(restored.snapshot.editor, saved.editor);
    assert!(restored.notices.is_empty());
}

#[test]
fn unchanged_head_clamps_positions_to_the_active_patch() {
    let mut saved = saved_session(ProviderKind::GitHub, "head-a");
    saved.cursor_row = 100;
    saved.scroll_row = 90;
    let fresh = provider_snapshot(
        ProviderKind::GitHub,
        "head-a",
        vec![changed_file(
            "src/lib.rs",
            Some("base-1"),
            Some("head-1"),
            None,
        )],
    );

    let restored = SessionRestorer::restore(saved, &fresh);

    assert_eq!(restored.snapshot.cursor_row, 4);
    assert_eq!(restored.snapshot.scroll_row, 4);
}

#[test]
fn changed_head_keeps_only_same_path_and_blob_reviewed() {
    let mut saved = saved_session(ProviderKind::GitHub, "head-old");
    saved.files = BTreeMap::from([
        (
            RepoPath("unchanged.rs".into()),
            progress("unchanged.rs", Some("b1"), Some("h1"), ReviewSync::Synced),
        ),
        (
            RepoPath("changed.rs".into()),
            progress("changed.rs", Some("b2"), Some("h2"), ReviewSync::Synced),
        ),
        (
            RepoPath("old-name.rs".into()),
            progress("old-name.rs", Some("b3"), Some("h3"), ReviewSync::Synced),
        ),
    ]);
    let mut renamed = changed_file("renamed.rs", Some("b3"), Some("h3"), None);
    renamed.previous_path = Some(RepoPath("old-name.rs".into()));
    renamed.status = FileStatus::Renamed;
    let fresh = provider_snapshot(
        ProviderKind::GitHub,
        "head-new",
        vec![
            changed_file("unchanged.rs", Some("b1"), Some("h1"), None),
            changed_file("changed.rs", Some("b2"), Some("h2-new"), None),
            renamed,
        ],
    );

    let restored = SessionRestorer::restore(saved, &fresh);

    assert!(restored.snapshot.files[&RepoPath("unchanged.rs".into())].reviewed);
    assert!(!restored.snapshot.files[&RepoPath("changed.rs".into())].reviewed);
    assert!(!restored.snapshot.files[&RepoPath("renamed.rs".into())].reviewed);
    assert_eq!(restored.snapshot.cursor_row, 0);
    assert_eq!(restored.snapshot.scroll_row, 0);
    assert!(restored.snapshot.editor.as_ref().unwrap().stale);
    assert!(restored.notices.contains(&RestoreNotice::EditorBecameStale));
    assert!(restored.notices.iter().any(|notice| matches!(
        notice,
        RestoreNotice::HeadChanged { old, new }
            if old == &CommitOid("head-old".into()) && new == &CommitOid("head-new".into())
    )));
}

#[test]
fn changed_head_matches_identity_when_only_one_blob_side_is_known() {
    let mut saved = saved_session(ProviderKind::GitHub, "head-old");
    saved.files = BTreeMap::from([
        (
            RepoPath("untouched.rs".into()),
            progress("untouched.rs", None, Some("h1"), ReviewSync::Synced),
        ),
        (
            RepoPath("rewritten.rs".into()),
            progress("rewritten.rs", None, Some("h2"), ReviewSync::Synced),
        ),
    ]);
    let fresh = provider_snapshot(
        ProviderKind::GitHub,
        "head-new",
        vec![
            changed_file("untouched.rs", None, Some("h1"), None),
            changed_file("rewritten.rs", None, Some("h2-new"), None),
        ],
    );

    let restored = SessionRestorer::restore(saved, &fresh);

    assert!(restored.snapshot.files[&RepoPath("untouched.rs".into())].reviewed);
    assert!(!restored.snapshot.files[&RepoPath("rewritten.rs".into())].reviewed);
    assert!(!restored.notices.contains(&RestoreNotice::FileReset {
        path: RepoPath("untouched.rs".into())
    }));
}

#[test]
fn github_remote_state_overrides_only_synced_progress() {
    let mut saved = saved_session(ProviderKind::GitHub, "head-a");
    saved
        .files
        .get_mut(&RepoPath("src/lib.rs".into()))
        .unwrap()
        .reviewed = true;
    let fresh = provider_snapshot(
        ProviderKind::GitHub,
        "head-a",
        vec![changed_file(
            "src/lib.rs",
            Some("base-1"),
            Some("head-1"),
            Some(false),
        )],
    );

    let restored = SessionRestorer::restore(saved, &fresh);

    let progress = &restored.snapshot.files[&RepoPath("src/lib.rs".into())];
    assert!(!progress.reviewed);
    assert_eq!(progress.sync, ReviewSync::Synced);
}

#[test]
fn github_pending_and_failed_desires_survive_remote_disagreement() {
    for sync in [
        ReviewSync::Pending { desired: true },
        ReviewSync::Failed {
            desired: true,
            message: "offline".into(),
        },
    ] {
        let mut saved = saved_session(ProviderKind::GitHub, "head-a");
        saved
            .files
            .get_mut(&RepoPath("src/lib.rs".into()))
            .unwrap()
            .sync = sync.clone();
        let fresh = provider_snapshot(
            ProviderKind::GitHub,
            "head-a",
            vec![changed_file(
                "src/lib.rs",
                Some("base-1"),
                Some("head-1"),
                Some(false),
            )],
        );

        let restored = SessionRestorer::restore(saved, &fresh);
        let progress = &restored.snapshot.files[&RepoPath("src/lib.rs".into())];
        assert!(progress.reviewed);
        assert_eq!(progress.sync, sync);
    }
}

#[test]
fn gitlab_local_only_progress_survives_restart() {
    let mut saved = saved_session(ProviderKind::GitLab, "head-a");
    saved
        .files
        .get_mut(&RepoPath("src/lib.rs".into()))
        .unwrap()
        .sync = ReviewSync::LocalOnly;
    let fresh = provider_snapshot(
        ProviderKind::GitLab,
        "head-a",
        vec![changed_file(
            "src/lib.rs",
            Some("base-1"),
            Some("head-1"),
            None,
        )],
    );

    let restored = SessionRestorer::restore(saved, &fresh);

    let progress = &restored.snapshot.files[&RepoPath("src/lib.rs".into())];
    assert!(progress.reviewed);
    assert_eq!(progress.sync, ReviewSync::LocalOnly);
}

#[test]
fn missing_blob_identity_resets_reviewed_progress() {
    let mut saved = saved_session(ProviderKind::GitLab, "head-old");
    saved.files = BTreeMap::from([(
        RepoPath("src/lib.rs".into()),
        progress("src/lib.rs", None, None, ReviewSync::LocalOnly),
    )]);
    let fresh = provider_snapshot(
        ProviderKind::GitLab,
        "head-new",
        vec![changed_file("src/lib.rs", None, None, None)],
    );

    let restored = SessionRestorer::restore(saved, &fresh);

    assert!(!restored.snapshot.files[&RepoPath("src/lib.rs".into())].reviewed);
    assert!(restored.notices.contains(&RestoreNotice::FileReset {
        path: RepoPath("src/lib.rs".into())
    }));
}

#[test]
fn reviewed_hunks_survive_a_new_head_when_the_file_is_untouched() {
    let mut saved = saved_session(ProviderKind::GitHub, "head-old");
    saved.files = BTreeMap::from([
        (
            RepoPath("untouched.rs".into()),
            progress("untouched.rs", None, Some("h1"), ReviewSync::Synced),
        ),
        (
            RepoPath("rewritten.rs".into()),
            progress("rewritten.rs", None, Some("h2"), ReviewSync::Synced),
        ),
    ]);
    let fresh = provider_snapshot(
        ProviderKind::GitHub,
        "head-new",
        vec![
            changed_file("untouched.rs", None, Some("h1"), None),
            changed_file("rewritten.rs", None, Some("h2-new"), None),
        ],
    );

    let restored = SessionRestorer::restore(saved, &fresh);

    assert_eq!(
        restored.snapshot.files[&RepoPath("untouched.rs".into())].reviewed_hunks,
        BTreeSet::from([0])
    );
    assert!(
        restored.snapshot.files[&RepoPath("rewritten.rs".into())]
            .reviewed_hunks
            .is_empty(),
        "a rewritten file starts over, hunks included"
    );
}

#[test]
fn a_reviewed_file_from_an_older_session_adopts_all_its_hunks() {
    let mut saved = saved_session(ProviderKind::GitHub, "head-a");
    saved
        .files
        .get_mut(&RepoPath("src/lib.rs".into()))
        .unwrap()
        .reviewed_hunks
        .clear();
    let fresh = provider_snapshot(
        ProviderKind::GitHub,
        "head-a",
        vec![changed_file(
            "src/lib.rs",
            Some("base-1"),
            Some("head-1"),
            Some(true),
        )],
    );

    let restored = SessionRestorer::restore(saved, &fresh);

    let progress = &restored.snapshot.files[&RepoPath("src/lib.rs".into())];
    assert!(progress.reviewed);
    assert_eq!(progress.reviewed_hunks, BTreeSet::from([0]));
}
