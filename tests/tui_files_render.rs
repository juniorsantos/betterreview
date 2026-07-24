use std::collections::BTreeMap;

use betterreview::{
    app::{AppAction, AppEvent, AppState, update},
    domain::{
        ChangeRequestKey, ChangedFile, CommitOid, FileStatus, PatchAvailability,
        ProviderCapabilities, ProviderKind, ProviderSnapshot, RepoPath,
    },
    state::{ContentIdentity, FileProgress, ReviewSync, SESSION_SCHEMA_VERSION, SessionSnapshot},
    tui::{KeyMap, handle_key},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use time::OffsetDateTime;

fn file(path: &str, status: FileStatus, additions: u32, deletions: u32) -> ChangedFile {
    ChangedFile {
        path: RepoPath(path.into()),
        previous_path: None,
        status,
        additions,
        deletions,
        patch: PatchAvailability::Available("@@ -1 +1 @@\n-old\n+new\n".into()),
        base_blob: None,
        head_blob: Some(format!("{path}-head")),
        remotely_reviewed: Some(false),
    }
}

fn app() -> AppState {
    let key = ChangeRequestKey {
        provider: ProviderKind::GitHub,
        host: "github.com".into(),
        repository: "owner/repo".into(),
        number: 42,
    };
    let files = vec![
        file("src/app/one.rs", FileStatus::Modified, 3, 1),
        file("src/app/two.rs", FileStatus::Added, 9, 0),
        file("docs/guide.md", FileStatus::Deleted, 0, 4),
    ];
    let progress = files
        .iter()
        .map(|file| {
            (
                file.path.clone(),
                FileProgress {
                    identity: ContentIdentity {
                        path: file.path.clone(),
                        base_blob: None,
                        head_blob: file.head_blob.clone(),
                    },
                    reviewed: false,
                    sync: ReviewSync::Synced,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let provider = ProviderSnapshot {
        key: key.clone(),
        title: "Review terminal".into(),
        author: "dev".into(),
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
        active_file: Some(RepoPath("src/app/one.rs".into())),
        cursor_row: 0,
        scroll_row: 0,
        files: progress,
        editor: None,
        pending_submit: None,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    };
    AppState::new(provider, session)
}

fn screen(state: &AppState) -> String {
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| betterreview::tui::render(frame, state))
        .unwrap();
    let buffer = terminal.backend().buffer();
    (0..30)
        .map(|y| {
            (0..100)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn files_panel_groups_entries_by_directory() {
    let screen = screen(&app());

    assert!(screen.contains("src/app/"));
    assert!(screen.contains("docs/"));
    // Files show only their basename under the directory header.
    assert!(screen.contains("M one.rs"));
    assert!(screen.contains("A two.rs"));
    assert!(screen.contains("D guide.md"));
    assert!(!screen.contains("M src/app/one.rs"));
}

#[test]
fn files_panel_shows_addition_and_deletion_counts() {
    let screen = screen(&app());

    assert!(screen.contains("+3 -1"));
    assert!(screen.contains("+9"));
    assert!(screen.contains("-4"));
}

#[test]
fn folded_directories_hide_their_files_and_show_progress() {
    let mut state = app();
    state.collapsed_dirs.insert("src/app".into());
    let screen = screen(&state);

    assert!(screen.contains("▸ src/app/ (0/2)"));
    assert!(!screen.contains("one.rs"));
    assert!(!screen.contains("two.rs"));
    assert!(screen.contains("D guide.md"));
}

#[test]
fn e_toggles_the_expanded_files_panel() {
    let mut app = app();
    let mut keymap = KeyMap::default();

    let event = handle_key(
        &mut app,
        &mut keymap,
        KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
    );

    let Some(AppEvent::Action(AppAction::ToggleFilesPanel)) = event else {
        panic!("expected ToggleFilesPanel, got {event:?}");
    };
    assert!(!app.files_expanded);
    update(&mut app, AppEvent::Action(AppAction::ToggleFilesPanel));
    assert!(app.files_expanded);

    // Expanded panel is wider: the top-right corner of the Files block moves.
    let narrow = screen(&{
        let mut state = self::app();
        state.files_expanded = false;
        state
    });
    let wide = screen(&{
        let mut state = self::app();
        state.files_expanded = true;
        state
    });
    let corner = |text: &str| {
        text.lines()
            .nth(1)
            .map(|line| line.find('┐').unwrap_or(0))
            .unwrap_or(0)
    };
    assert!(corner(&wide) > corner(&narrow));
}

#[test]
fn generated_files_show_muted_marker_instead_of_status_letter() {
    let key = ChangeRequestKey {
        provider: ProviderKind::GitHub,
        host: "github.com".into(),
        repository: "owner/repo".into(),
        number: 42,
    };
    let files = vec![
        file("src/app/one.rs", FileStatus::Modified, 3, 1),
        file("Cargo.lock", FileStatus::Modified, 5, 2),
    ];
    let progress = files
        .iter()
        .map(|file| {
            (
                file.path.clone(),
                FileProgress {
                    identity: ContentIdentity {
                        path: file.path.clone(),
                        base_blob: None,
                        head_blob: file.head_blob.clone(),
                    },
                    reviewed: false,
                    sync: ReviewSync::Synced,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let provider = ProviderSnapshot {
        key: key.clone(),
        title: "Review terminal".into(),
        author: "dev".into(),
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
        active_file: Some(RepoPath("src/app/one.rs".into())),
        cursor_row: 0,
        scroll_row: 0,
        files: progress,
        editor: None,
        pending_submit: None,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    };
    let state = AppState::new(provider, session);

    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| betterreview::tui::render(frame, &state))
        .unwrap();
    let buffer = terminal.backend().buffer().clone();
    let text = screen(&state);

    // The generated file's status letter ("M") is replaced by the muted
    // marker, while the ordinary file keeps its colored status letter.
    assert!(text.contains("⊘ Cargo.lock"));
    assert!(!text.contains("M Cargo.lock"));
    assert!(text.contains("M one.rs"));
    // +5 -2 counts remain visible for the generated file.
    assert!(text.contains("+5 -2"));

    let marker_pos = text
        .lines()
        .enumerate()
        .find_map(|(y, line)| line.find('⊘').map(|x| (x as u16, y as u16)))
        .expect("marker cell not found");
    let marker_cell = buffer.cell(marker_pos).unwrap();
    assert_eq!(marker_cell.fg, betterreview::tui::theme::MUTED);
}
