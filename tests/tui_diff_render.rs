use std::collections::BTreeMap;

use betterreview::{
    app::{AppState, DisplayRow, refresh_display_rows},
    diff::{RenderedDiff, RenderedRow, RowBinding},
    domain::{
        ChangeRequestKey, ChangedFile, CommitOid, DiffPosition, DiffSelection, DiffSide,
        DraftComment, DraftId, FileStatus, PatchAvailability, ProviderCapabilities, ProviderKind,
        ProviderSnapshot, RepoPath,
    },
    state::{ContentIdentity, FileProgress, ReviewSync, SESSION_SCHEMA_VERSION, SessionSnapshot},
    tui::{render, theme},
};
use ratatui::{Terminal, backend::TestBackend, text::Line};
use time::OffsetDateTime;

fn position(side: DiffSide, line: u32) -> DiffPosition {
    DiffPosition {
        path: RepoPath("src/app.rs".into()),
        side,
        line,
        hunk: 0,
    }
}

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
        patch: PatchAvailability::Available("@@ -3,2 +4,2 @@\n context\n-removed\n+added\n".into()),
        base_blob: None,
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
                    base_blob: None,
                    head_blob: Some("head-blob".into()),
                },
                reviewed: false,
                sync: ReviewSync::Synced,
            },
        )]),
        editor: None,
        pending_submit: None,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    };
    let mut app = AppState::new(provider, session);
    app.rendered_diff = Some(RenderedDiff {
        rows: vec![
            RenderedRow {
                text: Line::raw("context"),
                binding: RowBinding {
                    row_index: 0,
                    left: Some(position(DiffSide::Left, 3)),
                    right: Some(position(DiffSide::Right, 4)),
                },
            },
            RenderedRow {
                text: Line::raw("-removed"),
                binding: RowBinding {
                    row_index: 1,
                    left: Some(position(DiffSide::Left, 4)),
                    right: None,
                },
            },
            RenderedRow {
                text: Line::raw("+added"),
                binding: RowBinding {
                    row_index: 2,
                    left: None,
                    right: Some(position(DiffSide::Right, 5)),
                },
            },
        ],
    });
    refresh_display_rows(&mut app);
    app
}

fn draw(state: &AppState) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal.draw(|frame| render(frame, state)).unwrap();
    terminal
}

fn draw_wide(state: &AppState) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    terminal.draw(|frame| render(frame, state)).unwrap();
    terminal
}

fn screen_wide(terminal: &Terminal<TestBackend>) -> String {
    let buffer = terminal.backend().buffer();
    (0..30)
        .map(|y| {
            (0..120)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn screen(terminal: &Terminal<TestBackend>) -> String {
    let buffer = terminal.backend().buffer();
    (0..24)
        .map(|y| {
            (0..80)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn diff_shows_a_single_line_number_column() {
    let terminal = draw(&app());
    let screen = screen(&terminal);

    // New-side numbers for context/added rows; old-side number for removed.
    assert!(screen.contains("    4 context"));
    assert!(screen.contains("    4 -removed"));
    assert!(screen.contains("    5 +added"));
    // The old two-column gutter is gone.
    assert!(!screen.contains("   3    4 context"));
}

#[test]
fn cursor_highlights_the_entire_line_width() {
    let mut state = app();
    state.session.cursor_row = 2;
    refresh_display_rows(&mut state);
    let terminal = draw(&state);
    let buffer = terminal.backend().buffer();

    // Find the row showing the +added line.
    let row = (0..24)
        .find(|y| {
            (0..80)
                .map(|x| buffer.cell((x, *y)).unwrap().symbol())
                .collect::<String>()
                .contains("+added")
        })
        .expect("cursor row rendered");
    // The highlight must reach the right edge of the panel interior, far
    // beyond the text itself.
    let cell = buffer.cell((77, row)).unwrap();
    assert_eq!(cell.style().bg, Some(theme::CURSOR_LINE));
}

fn draft_at_line_5() -> DraftComment {
    DraftComment {
        id: DraftId("d1".into()),
        body: "Please double-check this line".into(),
        selection: Some(DiffSelection {
            start: position(DiffSide::Right, 5),
            end: position(DiffSide::Right, 5),
        }),
        thread_id: None,
    }
}

#[test]
fn comment_box_renders_under_its_line() {
    let mut state = app();
    state.provider.drafts.push(draft_at_line_5());
    refresh_display_rows(&mut state);

    let terminal = draw(&state);
    let screen = screen(&terminal);
    let lines: Vec<&str> = screen.lines().collect();

    let anchor = lines
        .iter()
        .position(|line| line.contains("+added"))
        .expect("anchored diff row rendered");
    // Card: top border with the meta, body line, bottom border with hints.
    assert!(lines[anchor + 1].contains("╭─ @você · draft"));
    assert!(lines[anchor + 2].contains("│ Please double-check this line"));
    assert!(lines[anchor + 3].contains("╰─"));
}

#[test]
fn cursor_highlights_a_comment_row_full_width() {
    let mut state = app();
    state.provider.drafts.push(draft_at_line_5());
    refresh_display_rows(&mut state);
    let comment_index = state
        .display_rows
        .iter()
        .position(|row| matches!(row, DisplayRow::Comment { .. }))
        .expect("comment row present in cache");
    state.display_cursor = comment_index;

    let terminal = draw(&state);
    let buffer = terminal.backend().buffer();
    let row = (0..24)
        .find(|y| {
            (0..80)
                .map(|x| buffer.cell((x, *y)).unwrap().symbol())
                .collect::<String>()
                .contains("draft")
        })
        .expect("comment row rendered");
    let cell = buffer.cell((77, row)).unwrap();
    assert_eq!(cell.style().bg, Some(theme::CURSOR_LINE));
}

#[test]
fn toggle_hides_comment_rows() {
    let mut state = app();
    state.provider.drafts.push(draft_at_line_5());
    refresh_display_rows(&mut state);
    let visible_screen = screen(&draw(&state));
    assert!(visible_screen.contains("draft"));
    assert!(visible_screen.contains("Please double-check this line"));

    state.comments_hidden = true;
    refresh_display_rows(&mut state);
    let hidden_screen = screen(&draw(&state));
    assert!(!hidden_screen.contains("Please double-check this line"));
    assert!(hidden_screen.contains("+added"));
}

#[test]
fn status_shows_spinner_while_saving() {
    let mut state = app();
    state.pending_labels.insert(3, "salvando comentário…");
    state.busy_operations.insert(3);
    let terminal = draw(&state);
    let screen = screen(&terminal);

    assert!(screen.contains("salvando comentário…"));
}

#[test]
fn status_shows_the_latest_notice() {
    let mut state = app();
    state.notices.push("primeiro aviso".into());
    state.notices.push("mova para uma linha de código".into());
    state.notice_ttl = 5;
    let notice_screen = screen(&draw(&state));

    assert!(notice_screen.contains("mova para uma linha de código"));
    assert!(!notice_screen.contains("primeiro aviso"));

    // A notice loses to an active error banner.
    state.error_banner = Some("falha ao salvar".into());
    let banner_screen = screen(&draw(&state));
    assert!(banner_screen.contains("falha ao salvar"));
    assert!(!banner_screen.contains("mova para uma linha de código"));
}

#[test]
fn comment_block_renders_as_a_card_with_action_hints() {
    let mut state = app();
    state.provider.drafts.push(DraftComment {
        id: DraftId("d2".into()),
        body: "corpo do comentário\nsegunda linha".into(),
        selection: Some(DiffSelection {
            start: position(DiffSide::Right, 5),
            end: position(DiffSide::Right, 5),
        }),
        thread_id: None,
    });
    refresh_display_rows(&mut state);

    let terminal = draw_wide(&state);
    let screen = screen_wide(&terminal);

    assert!(screen.contains("╭─ @você · draft"));
    assert!(screen.contains("│ corpo do comentário"));
    assert!(screen.contains("│ segunda linha"));
    assert!(screen.contains("╰─ e editar · x excluir"));
}

#[test]
fn status_shows_the_search_input_while_typing() {
    let mut state = app();
    state.search_input = Some("added".into());

    let screen = screen(&draw(&state));

    assert!(screen.contains("/added▌"));
}

#[test]
fn status_shows_query_and_match_count_for_an_active_search() {
    let mut state = app();
    // "context", "-removed" and "+added" all contain the letter "e".
    state.search_query = Some("e".into());

    let screen = screen(&draw(&state));

    assert!(screen.contains("e"), "sanity: screen renders at all");
    assert!(screen.contains("1/3"));
    assert!(screen.contains("n/N navega"));
    assert!(screen.contains("Esc limpa"));
}

#[test]
fn editor_shows_the_terminal_cursor_at_the_typing_position() {
    use betterreview::state::EditorSnapshot;
    let mut state = app();
    let anchor = position(DiffSide::Right, 5);
    state.session.editor = Some(EditorSnapshot {
        lines: vec!["abc".into()],
        cursor_row: 0,
        grapheme_col: 3,
        original_head: CommitOid("head".into()),
        path: RepoPath("src/app.rs".into()),
        selection: DiffSelection {
            start: anchor.clone(),
            end: anchor,
        },
        stale: false,
    });
    state.editor_open = true;

    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal.draw(|frame| render(frame, &state)).unwrap();
    let at_col_3 = terminal.get_cursor_position().unwrap();

    state.session.editor.as_mut().unwrap().grapheme_col = 0;
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal.draw(|frame| render(frame, &state)).unwrap();
    let at_col_0 = terminal.get_cursor_position().unwrap();

    assert_eq!(
        at_col_3.x,
        at_col_0.x + 3,
        "cursor tracks the typing column"
    );
}
