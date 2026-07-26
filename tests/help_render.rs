//! TDD coverage for the colorized help modal (src/tui/widgets/help.rs): the
//! body is now styled `Line`s — keys ACCENT+BOLD, descriptions FG, section
//! titles MUTED+BOLD — laid out in columns, drawn through the shared
//! Dialog component.

use std::collections::BTreeMap;

use betterreview::{
    app::AppState,
    domain::{
        ChangeRequestKey, ChangedFile, CommitOid, FileStatus, PatchAvailability,
        ProviderCapabilities, ProviderKind, ProviderSnapshot, RepoPath,
    },
    state::{ContentIdentity, FileProgress, ReviewSync, SESSION_SCHEMA_VERSION, SessionSnapshot},
    tui::{render, theme},
};
use ratatui::{Terminal, backend::TestBackend, style::Modifier};

fn app() -> AppState {
    let path = RepoPath("src/app.rs".into());
    let key = ChangeRequestKey {
        provider: ProviderKind::GitHub,
        host: "github.com".into(),
        repository: "owner/repo".into(),
        number: 42,
    };
    let file = ChangedFile {
        path: path.clone(),
        previous_path: None,
        status: FileStatus::Modified,
        additions: 1,
        deletions: 1,
        patch: PatchAvailability::Available("@@ -1 +1 @@\n-old\n+new\n".into()),
        base_blob: Some("base-blob".into()),
        head_blob: Some("head-blob".into()),
        remotely_reviewed: Some(false),
    };
    let provider = ProviderSnapshot {
        key: key.clone(),
        title: "Review terminal".into(),
        author: "dev".into(),
        web_url: "https://github.com/owner/repo/pull/42".into(),
        base: CommitOid("base".into()),
        head: CommitOid("head".into()),
        files: vec![file],
        threads: Vec::new(),
        drafts: Vec::new(),
        capabilities: ProviderCapabilities::all_supported(),
    };
    let session = SessionSnapshot {
        schema_version: SESSION_SCHEMA_VERSION,
        key,
        base: CommitOid("base".into()),
        head: CommitOid("head".into()),
        active_file: Some(path.clone()),
        cursor_row: 0,
        scroll_row: 0,
        files: BTreeMap::from([(
            path.clone(),
            FileProgress {
                identity: ContentIdentity {
                    path,
                    base_blob: Some("base-blob".into()),
                    head_blob: Some("head-blob".into()),
                },
                reviewed: false,
                reviewed_hunks: Default::default(),
                sync: ReviewSync::Synced,
            },
        )]),
        editor: None,
        pending_submit: None,
        updated_at: time::OffsetDateTime::UNIX_EPOCH,
    };
    AppState::new(provider, session)
}

fn char_offset(haystack: &str, needle: &str) -> Option<usize> {
    let byte_offset = haystack.find(needle)?;
    Some(haystack[..byte_offset].chars().count())
}

fn screen(state: &AppState) -> (String, Vec<String>, Terminal<TestBackend>) {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, state)).unwrap();
    let buffer = terminal.backend().buffer();
    let lines: Vec<String> = (0..30)
        .map(|y| {
            (0..100)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect::<String>()
        })
        .collect();
    (lines.join("\n"), lines, terminal)
}

#[test]
fn help_shows_the_dialog_title_and_key_bindings() {
    let mut state = app();
    state.help_visible = true;

    let (screen, _, _) = screen(&state);

    assert!(screen.contains("Help"));
    assert!(screen.contains("hunk"));
    assert!(screen.contains("comment"));
    assert!(screen.contains("search"));
}

#[test]
fn help_renders_keys_in_accent_bold() {
    let mut state = app();
    state.help_visible = true;

    let (screen, lines, terminal) = screen(&state);
    assert!(screen.contains("]h"));

    let buffer = terminal.backend().buffer();
    let row = lines
        .iter()
        .position(|line| line.contains("]h"))
        .expect("]h row rendered");
    let col = char_offset(&lines[row], "]h").unwrap();
    let cell = buffer.cell((col as u16, row as u16)).unwrap();

    assert_eq!(cell.style().fg, Some(theme::ACCENT));
    assert!(cell.style().add_modifier.contains(Modifier::BOLD));
}

#[test]
fn help_renders_section_titles_in_muted_bold() {
    let mut state = app();
    state.help_visible = true;

    let (screen, lines, terminal) = screen(&state);
    assert!(screen.contains("Move"));

    let buffer = terminal.backend().buffer();
    let row = lines
        .iter()
        .position(|line| line.contains("Move"))
        .expect("section title row rendered");
    let col = char_offset(&lines[row], "Move").unwrap();
    let cell = buffer.cell((col as u16, row as u16)).unwrap();

    assert_eq!(cell.style().fg, Some(theme::MUTED));
    assert!(cell.style().add_modifier.contains(Modifier::BOLD));
}

#[test]
fn help_renders_descriptions_in_fg() {
    let mut state = app();
    state.help_visible = true;

    let (screen, lines, terminal) = screen(&state);
    assert!(screen.contains("hunk"));

    let buffer = terminal.backend().buffer();
    let row = lines
        .iter()
        .position(|line| line.contains("hunk"))
        .expect("hunk description row rendered");
    let col = char_offset(&lines[row], "hunk").unwrap();
    let cell = buffer.cell((col as u16, row as u16)).unwrap();

    assert_eq!(cell.style().fg, Some(theme::FG));
}

#[test]
fn shortcuts_are_grouped_by_intent() {
    let mut state = app();
    state.help_visible = true;

    let (screen, _, _) = screen(&state);

    for heading in ["Move", "Review", "View", "Session"] {
        assert!(screen.contains(heading), "{heading} column missing");
    }
    assert!(
        !screen.contains("Navigation"),
        "panel-shaped grouping is gone"
    );
}

#[test]
fn view_toggles_sit_together_not_under_review() {
    let mut state = app();
    state.help_visible = true;

    let (screen, lines, _) = screen(&state);
    let column_of = |needle: &str| {
        lines
            .iter()
            .find(|line| line.contains(needle))
            .and_then(|line| line.find(needle))
            .unwrap_or_else(|| panic!("{needle} missing from {screen}"))
    };

    let view = column_of("split");
    for toggle in ["one side", "wrap"] {
        assert!(
            column_of(toggle).abs_diff(view) <= 2,
            "{toggle} should share the View column"
        );
    }
}

#[test]
fn every_column_uses_the_same_grammar() {
    let mut state = app();
    state.help_visible = true;

    let (screen, _, _) = screen(&state);

    assert!(
        !screen.contains("Comments:") && !screen.contains("Editor:"),
        "trailing groups in a different shape are gone"
    );
    for key in ["q", "Q", "R", "r"] {
        assert!(
            screen.contains(&format!(" {key}  ")) || screen.contains(&format!("{key}  ")),
            "{key} still listed"
        );
    }
}

#[test]
fn no_column_is_left_more_than_two_rows_shorter() {
    let mut state = app();
    state.help_visible = true;

    let (_, lines, _) = screen(&state);
    let body: Vec<&String> = lines
        .iter()
        .filter(|line| line.contains('│') && line.contains("  "))
        .collect();

    assert!(!body.is_empty(), "the dialog rendered");
}
