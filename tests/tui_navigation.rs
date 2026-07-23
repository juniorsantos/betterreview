use std::collections::BTreeMap;

use betterreview::{
    app::{AppAction, AppFocus, AppState},
    diff::{RenderedDiff, RenderedRow, RowBinding},
    domain::{
        ChangeRequestKey, ChangedFile, CommitOid, DiffPosition, DiffSide, FileStatus,
        PatchAvailability, ProviderCapabilities, ProviderKind, ProviderSnapshot, RepoPath,
    },
    state::{ContentIdentity, FileProgress, ReviewSync, SESSION_SCHEMA_VERSION, SessionSnapshot},
    tui::{KeyMap, key_to_action, render},
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, text::Line};
use time::OffsetDateTime;

fn app() -> AppState {
    let paths = [RepoPath("src/app.rs".into()), RepoPath("src/new.rs".into())];
    let files = vec![
        ChangedFile {
            path: paths[0].clone(),
            previous_path: None,
            status: FileStatus::Modified,
            additions: 1,
            deletions: 1,
            patch: PatchAvailability::Available("@@ -1 +1 @@\n-old\n+new\n".into()),
            base_blob: Some("base-app".into()),
            head_blob: Some("head-app".into()),
            remotely_reviewed: Some(true),
        },
        ChangedFile {
            path: paths[1].clone(),
            previous_path: None,
            status: FileStatus::Added,
            additions: 1,
            deletions: 0,
            patch: PatchAvailability::Binary,
            base_blob: None,
            head_blob: Some("head-new".into()),
            remotely_reviewed: Some(false),
        },
    ];
    let key = ChangeRequestKey {
        provider: ProviderKind::GitHub,
        host: "github.com".into(),
        repository: "owner/repo".into(),
        number: 42,
    };
    let provider = ProviderSnapshot {
        key: key.clone(),
        title: "Add terminal review".into(),
        author: "developer".into(),
        web_url: "https://github.com/owner/repo/pull/42".into(),
        base: CommitOid("base".into()),
        head: CommitOid("head".into()),
        files,
        threads: Vec::new(),
        drafts: Vec::new(),
        capabilities: ProviderCapabilities::all_supported(),
    };
    let session = SessionSnapshot {
        schema_version: SESSION_SCHEMA_VERSION,
        key,
        base: CommitOid("base".into()),
        head: CommitOid("head".into()),
        active_file: Some(paths[0].clone()),
        cursor_row: 1,
        scroll_row: 0,
        files: BTreeMap::from([
            (
                paths[0].clone(),
                FileProgress {
                    identity: ContentIdentity {
                        path: paths[0].clone(),
                        base_blob: Some("base-app".into()),
                        head_blob: Some("head-app".into()),
                    },
                    reviewed: true,
                    sync: ReviewSync::Synced,
                },
            ),
            (
                paths[1].clone(),
                FileProgress {
                    identity: ContentIdentity {
                        path: paths[1].clone(),
                        base_blob: None,
                        head_blob: Some("head-new".into()),
                    },
                    reviewed: false,
                    sync: ReviewSync::Pending { desired: true },
                },
            ),
        ]),
        editor: None,
        pending_submit: None,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    };
    let mut app = AppState::new(provider, session);
    app.rendered_diff = Some(RenderedDiff {
        rows: vec![
            RenderedRow {
                text: Line::raw("-old"),
                binding: RowBinding {
                    row_index: 0,
                    left: Some(DiffPosition {
                        path: paths[0].clone(),
                        side: DiffSide::Left,
                        line: 8,
                        hunk: 0,
                    }),
                    right: None,
                },
            },
            RenderedRow {
                text: Line::raw("+new"),
                binding: RowBinding {
                    row_index: 1,
                    left: None,
                    right: Some(DiffPosition {
                        path: paths[0].clone(),
                        side: DiffSide::Right,
                        line: 8,
                        hunk: 0,
                    }),
                },
            },
        ],
    });
    app
}

fn app_with_long_content() -> AppState {
    let mut state = app();
    state.provider.files = (0..20)
        .map(|index| ChangedFile {
            path: RepoPath(format!("src/file_{index}.rs")),
            previous_path: None,
            status: FileStatus::Modified,
            additions: 1,
            deletions: 0,
            patch: PatchAvailability::Available(format!("@@ -0,0 +1 @@\n+line-{index}\n")),
            base_blob: None,
            head_blob: Some(format!("head-{index}")),
            remotely_reviewed: Some(false),
        })
        .collect();
    state.rendered_diff = Some(RenderedDiff {
        rows: (0..20)
            .map(|index| RenderedRow {
                text: Line::raw(format!("diff-row-{index:02}")),
                binding: RowBinding {
                    row_index: index,
                    left: None,
                    right: None,
                },
            })
            .collect(),
    });
    state
}

fn screen(state: &AppState, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, state)).unwrap();
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            let line = (0..width)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect::<String>();
            line.trim_end().to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn renders_wide_file_panel_canonical_diff_and_shortcuts() {
    let screen = screen(&app(), 120, 36);

    assert!(screen.contains("GitHub owner/repo #42"));
    assert!(screen.contains("Add terminal review"));
    assert!(screen.contains("[x] M src/app.rs"));
    assert!(screen.contains("[~] A src/new.rs"));
    assert!(screen.contains("8      -old"));
    assert!(screen.contains("    8 +new"));
    assert!(screen.contains("]f/[f file"));
    assert!(screen.contains("h/l focus"));
    assert!(screen.contains("R submit"));
}

#[test]
fn narrow_layout_prioritizes_diff_and_can_overlay_files() {
    let mut state = app();
    let diff = screen(&state, 50, 16);
    assert!(diff.contains("-old"));
    assert!(!diff.contains("src/new.rs"));

    state.focus = AppFocus::Files;
    let files = screen(&state, 50, 16);
    assert!(files.contains("src/app.rs"));
    assert!(files.contains("src/new.rs"));
}

#[test]
fn file_panel_scrolls_to_keep_the_active_file_visible() {
    let mut state = app_with_long_content();
    state.focus = AppFocus::Files;
    state.active_file_index = 15;

    let rendered = screen(&state, 80, 12);

    assert!(rendered.contains("src/file_15.rs"));
    assert!(!rendered.contains("src/file_0.rs"));
}

#[test]
fn diff_panel_scrolls_to_keep_the_cursor_visible() {
    let mut state = app_with_long_content();
    state.session.cursor_row = 15;

    let rendered = screen(&state, 80, 12);

    assert!(rendered.contains("diff-row-15"));
    assert!(!rendered.contains("diff-row-00"));
}

#[test]
fn layout_snapshots_cover_wide_medium_and_narrow_terminals() {
    insta::assert_snapshot!("wide_120x36", screen(&app(), 120, 36));
    insta::assert_snapshot!("medium_80x24", screen(&app(), 80, 24));
    insta::assert_snapshot!("narrow_50x16", screen(&app(), 50, 16));
}

#[test]
fn maps_navigation_keys_and_ignores_release_events() {
    assert_eq!(
        key_to_action(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
        Some(AppAction::FocusNext)
    );
    assert_eq!(
        key_to_action(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)),
        Some(AppAction::FocusPrevious)
    );
    assert_eq!(
        key_to_action(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE)),
        Some(AppAction::MoveCursor(1))
    );
    assert_eq!(
        key_to_action(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE)),
        Some(AppAction::FocusPrevious)
    );
    assert_eq!(
        key_to_action(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE)),
        Some(AppAction::FocusNext)
    );
    assert_eq!(
        key_to_action(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)),
        Some(AppAction::FocusPrevious)
    );
    assert_eq!(
        key_to_action(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
        Some(AppAction::FocusNext)
    );
    assert_eq!(
        key_to_action(KeyEvent::new_with_kind(
            KeyCode::Char('m'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
        )),
        None
    );
    assert_eq!(
        key_to_action(KeyEvent::new_with_kind(
            KeyCode::Down,
            KeyModifiers::NONE,
            KeyEventKind::Repeat,
        )),
        Some(AppAction::MoveCursor(1))
    );
}

#[test]
fn maps_prefixed_file_and_unreviewed_navigation() {
    let mut keys = KeyMap::default();
    assert_eq!(
        keys.feed(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE)),
        None
    );
    assert_eq!(
        keys.feed(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE)),
        Some(AppAction::NextFile)
    );
    assert_eq!(
        keys.feed(KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE)),
        None
    );
    assert_eq!(
        keys.feed(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE)),
        Some(AppAction::PreviousUnreviewed)
    );
}
