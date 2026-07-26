use betterreview::{
    app::AppState,
    domain::{
        ChangeRequestKey, ChangedFile, CommitOid, DiffLayout, FileStatus, PatchAvailability,
        ProviderCapabilities, ProviderKind, ProviderSnapshot, RepoPath,
    },
    state::{SESSION_SCHEMA_VERSION, SessionSnapshot},
    tui::{SplitSide, diff_columns},
};
use ratatui::layout::Rect;

fn app() -> AppState {
    let key = ChangeRequestKey {
        provider: ProviderKind::GitHub,
        host: "github.com".into(),
        repository: "owner/repo".into(),
        number: 1,
    };
    let provider = ProviderSnapshot {
        key: key.clone(),
        title: String::new(),
        author: String::new(),
        web_url: String::new(),
        base: CommitOid("b".into()),
        head: CommitOid("h".into()),
        files: vec![ChangedFile {
            path: RepoPath("src/app.rs".into()),
            previous_path: None,
            status: FileStatus::Modified,
            additions: 1,
            deletions: 1,
            patch: PatchAvailability::Available("@@ -1 +1 @@\n-a\n+b\n".into()),
            base_blob: None,
            head_blob: None,
            remotely_reviewed: Some(false),
        }],
        threads: Vec::new(),
        drafts: Vec::new(),
        capabilities: ProviderCapabilities::all_supported(),
    };
    let session = SessionSnapshot {
        schema_version: SESSION_SCHEMA_VERSION,
        key,
        base: CommitOid("b".into()),
        head: CommitOid("h".into()),
        active_file: Some(RepoPath("src/app.rs".into())),
        cursor_row: 0,
        scroll_row: 0,
        files: Default::default(),
        editor: None,
        pending_submit: None,
        updated_at: time::OffsetDateTime::UNIX_EPOCH,
    };
    AppState::new(provider, session)
}

#[test]
fn unified_layout_has_no_columns() {
    let state = app();

    assert!(diff_columns(Rect::new(0, 0, 120, 30), &state).is_none());
}

#[test]
fn split_columns_share_the_width_around_a_divider() {
    let mut state = app();
    state.diff_layout = DiffLayout::Split;
    state.terminal_width = 160;

    let columns = diff_columns(Rect::new(0, 0, 120, 30), &state).expect("split columns");

    assert_eq!(columns.left.x, 0);
    assert_eq!(columns.divider, columns.left.x + columns.left.width);
    assert_eq!(columns.right.x, columns.divider + 3);
    assert_eq!(
        columns.left.width, columns.right.width,
        "both sides get the same room"
    );
    assert!(
        columns.right.x + columns.right.width <= 120,
        "columns stay inside the panel"
    );
}

#[test]
fn expanding_one_side_gives_it_the_whole_panel() {
    let mut state = app();
    state.diff_layout = DiffLayout::Split;
    state.terminal_width = 160;
    state.split_focus = Some(SplitSide::New);

    let columns = diff_columns(Rect::new(0, 0, 120, 30), &state).expect("split columns");

    assert_eq!(columns.right.width, 120);
    assert_eq!(columns.left.width, 0, "the old side is not drawn");
}

#[test]
fn expanding_the_old_side_mirrors_it() {
    let mut state = app();
    state.diff_layout = DiffLayout::Split;
    state.terminal_width = 160;
    state.split_focus = Some(SplitSide::Old);

    let columns = diff_columns(Rect::new(0, 0, 120, 30), &state).expect("split columns");

    assert_eq!(columns.left.width, 120);
    assert_eq!(columns.right.width, 0);
}
