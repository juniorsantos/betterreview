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
                    reviewed_hunks: Default::default(),
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

    assert!(screen.contains("[2] Files"));
    assert!(screen.contains("▾ src/app/"));
    assert!(screen.contains("▾ docs/"));
    assert!(screen.contains("docs/"));
    // Files show only their basename under the directory header.
    assert!(screen.contains("[ ] M one.rs"));
    assert!(screen.contains("[ ] A two.rs"));
    assert!(screen.contains("[ ] D guide.md"));
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
            .nth(2)
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
                    reviewed_hunks: Default::default(),
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

#[test]
fn reviewed_files_show_a_checked_checkbox() {
    let mut state = app();
    state
        .session
        .files
        .get_mut(&RepoPath("src/app/one.rs".into()))
        .unwrap()
        .reviewed = true;
    let screen = screen(&state);

    assert!(screen.contains("[x] M one.rs"));
    assert!(screen.contains("[ ] A two.rs"));
}

#[test]
fn folded_directory_with_the_active_file_shows_the_highlight() {
    let mut state = app();
    state.collapsed_dirs.insert("src/app".into());
    // active file is src/app/one.rs (inside the folded dir)
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| betterreview::tui::render(frame, &state))
        .unwrap();
    let buffer = terminal.backend().buffer().clone();
    let row = (0..30)
        .find(|y| {
            (0..100)
                .map(|x| buffer.cell((x, *y)).unwrap().symbol())
                .collect::<String>()
                .contains("▸ src/app/")
        })
        .expect("folded header rendered");
    let cell = buffer.cell((3, row)).unwrap();
    assert_eq!(
        cell.style().bg,
        Some(betterreview::tui::theme::CURSOR_LINE),
        "the folded folder holding the active file must be highlighted"
    );
}

#[test]
fn enter_toggles_the_fold_when_files_panel_is_focused() {
    let mut app = app();
    app.focus = betterreview::app::AppFocus::Files;
    let mut keymap = KeyMap::default();

    let event = handle_key(
        &mut app,
        &mut keymap,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
    );

    assert!(matches!(
        event,
        Some(AppEvent::Action(AppAction::ToggleFold))
    ));
}

#[test]
fn files_panel_shows_hunk_progress_for_every_file_with_a_patch() {
    let mut state = app();
    let path = RepoPath("src/app/one.rs".into());
    state.provider.files[0].patch = PatchAvailability::Available(
        "@@ -1 +1 @@\n-old\n+new\n@@ -9 +9 @@\n-old\n+new\n@@ -20 +20 @@\n-old\n+new\n".into(),
    );
    state.refresh_hunk_totals();
    state
        .session
        .files
        .get_mut(&path)
        .unwrap()
        .reviewed_hunks
        .insert(0);
    state.files_expanded = true;

    let screen = screen(&state);

    assert!(
        screen.contains("1/3"),
        "partial progress on the edited file"
    );
    assert!(screen.contains("0/1"), "untouched files still show a total");
}

#[test]
fn files_panel_omits_hunk_progress_when_the_patch_is_unavailable() {
    let mut state = app();
    state.provider.files[0].patch = PatchAvailability::Binary;
    state.refresh_hunk_totals();
    state.files_expanded = true;

    let screen = screen(&state);
    let line = screen
        .lines()
        .find(|line| line.contains("one.rs"))
        .expect("binary file row rendered");

    assert!(!line.contains("/"), "no denominator for a patchless file");
}

#[test]
fn f_hides_the_files_panel_and_shows_it_again() {
    let mut app = app();
    let mut keymap = KeyMap::default();

    let event = handle_key(
        &mut app,
        &mut keymap,
        KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE),
    );
    let Some(AppEvent::Action(AppAction::ToggleFilesVisible)) = event else {
        panic!("expected ToggleFilesVisible, got {event:?}");
    };

    update(&mut app, AppEvent::Action(AppAction::ToggleFilesVisible));
    assert!(app.files_hidden);
    let hidden = screen(&app);
    assert!(!hidden.contains("[2] Files"), "the panel is gone");
    assert!(hidden.contains("[3] Diff"));

    update(&mut app, AppEvent::Action(AppAction::ToggleFilesVisible));
    assert!(!app.files_hidden);
    assert!(screen(&app).contains("[2] Files"));
}

#[test]
fn hiding_the_panel_while_it_holds_the_focus_moves_the_focus_to_the_diff() {
    let mut app = app();
    app.focus = betterreview::app::AppFocus::Files;

    update(&mut app, AppEvent::Action(AppAction::ToggleFilesVisible));

    assert_eq!(app.focus, betterreview::app::AppFocus::Diff);
}

#[test]
fn focusing_the_files_panel_brings_it_back() {
    let mut app = app();
    update(&mut app, AppEvent::Action(AppAction::ToggleFilesVisible));
    assert!(app.files_hidden);

    update(&mut app, AppEvent::Action(AppAction::FocusFiles));

    assert!(
        !app.files_hidden,
        "2 cannot focus a panel that is not there"
    );
    assert_eq!(app.focus, betterreview::app::AppFocus::Files);
}

#[test]
fn the_diff_takes_the_whole_body_when_the_panel_is_hidden() {
    let mut app = app();
    let visible = screen(&app);
    update(&mut app, AppEvent::Action(AppAction::ToggleFilesVisible));
    let hidden = screen(&app);

    let diff_start = |text: &str| {
        text.lines()
            .find(|line| line.contains("[3] Diff"))
            .and_then(|line| line.find("[3] Diff"))
            .expect("diff panel")
    };
    assert!(
        diff_start(&hidden) < diff_start(&visible),
        "the diff panel starts further left once the files panel is gone"
    );
}

#[test]
fn a_long_path_keeps_its_file_name_in_the_panel() {
    let mut state = app();
    state.provider.files[0].path = RepoPath("aadfadf/bsdff/casdfdsf/config.rs".into());
    state.session.active_file = Some(state.provider.files[0].path.clone());
    state.refresh_hunk_totals();

    let screen = screen(&state);

    assert!(
        screen.contains("config.rs"),
        "the name identifies the file and must survive truncation"
    );
}

#[test]
fn a_cjk_file_name_does_not_push_the_panel_border() {
    let mut state = app();
    state.provider.files[0].path = RepoPath("src/app/テストファイル名前.rs".into());
    state.session.active_file = Some(state.provider.files[0].path.clone());
    state.refresh_hunk_totals();

    let screen = screen(&state);
    let borders: Vec<usize> = screen
        .lines()
        .filter(|line| line.contains('│'))
        .filter_map(|line| line.find('│'))
        .collect();

    assert!(
        borders.windows(2).all(|pair| pair[0] == pair[1]),
        "every row must start the panel at the same column: {borders:?}"
    );
}
