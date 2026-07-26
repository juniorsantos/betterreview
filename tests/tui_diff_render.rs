use std::collections::BTreeMap;

use betterreview::{
    app::{AppState, DisplayRow, refresh_display_rows},
    diff::{DiffRow, DiffRowKind, ParsedFileDiff, RenderedDiff, RenderedRow, RowBinding},
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
                reviewed_hunks: Default::default(),
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

/// Two rows a hidden run of context apart: new-file line 1, then line 5 —
/// lines 2-4 are collapsed into a gap.
fn app_with_gap() -> AppState {
    let mut state = app();
    let path = RepoPath("src/app.rs".into());
    state.rendered_diff = Some(RenderedDiff {
        rows: vec![
            RenderedRow {
                text: Line::raw("context"),
                binding: RowBinding {
                    row_index: 0,
                    left: Some(position(DiffSide::Left, 3)),
                    right: Some(position(DiffSide::Right, 1)),
                },
            },
            RenderedRow {
                text: Line::raw("+added"),
                binding: RowBinding {
                    row_index: 1,
                    left: None,
                    right: Some(position(DiffSide::Right, 5)),
                },
            },
        ],
    });
    state.parsed_diff = Some(ParsedFileDiff {
        path: path.clone(),
        head: CommitOid("head".into()),
        rows: vec![
            DiffRow {
                raw: " context".into(),
                kind: DiffRowKind::Context,
                old_line: Some(3),
                new_line: Some(1),
                left: Some(position(DiffSide::Left, 3)),
                right: Some(position(DiffSide::Right, 1)),
            },
            DiffRow {
                raw: "+added".into(),
                kind: DiffRowKind::Added,
                old_line: None,
                new_line: Some(5),
                left: None,
                right: Some(position(DiffSide::Right, 5)),
            },
        ],
        hunks: Vec::new(),
    });
    refresh_display_rows(&mut state);
    state
}

fn app_with_two_hunk_headers() -> AppState {
    let mut state = app();
    let path = RepoPath("src/app.rs".into());
    state.provider.files[0].patch =
        PatchAvailability::Available("@@ -3,1 +1,1 @@\n context\n@@ -9,1 +9,1 @@\n+added\n".into());
    state.refresh_hunk_totals();
    let header = |raw: &str| DiffRow {
        raw: raw.into(),
        kind: DiffRowKind::HunkHeader,
        old_line: None,
        new_line: None,
        left: None,
        right: None,
    };
    state.parsed_diff = Some(ParsedFileDiff {
        path: path.clone(),
        head: CommitOid("head".into()),
        rows: vec![
            header("@@ -3,1 +1,1 @@"),
            DiffRow {
                raw: " context".into(),
                kind: DiffRowKind::Context,
                old_line: Some(3),
                new_line: Some(1),
                left: Some(position(DiffSide::Left, 3)),
                right: Some(position(DiffSide::Right, 1)),
            },
            header("@@ -9,1 +9,1 @@"),
            DiffRow {
                raw: "+added".into(),
                kind: DiffRowKind::Added,
                old_line: None,
                new_line: Some(9),
                left: None,
                right: Some(position(DiffSide::Right, 9)),
            },
        ],
        hunks: vec![
            betterreview::diff::DiffHunk {
                id: 0,
                old_start: 3,
                old_count: 1,
                new_start: 1,
                new_count: 1,
                row_range: 1..2,
            },
            betterreview::diff::DiffHunk {
                id: 1,
                old_start: 9,
                old_count: 1,
                new_start: 9,
                new_count: 1,
                row_range: 3..4,
            },
        ],
    });
    state.rendered_diff = Some(RenderedDiff {
        rows: vec![
            RenderedRow {
                text: Line::raw("@@ -3,1 +1,1 @@"),
                binding: RowBinding {
                    row_index: 0,
                    left: None,
                    right: None,
                },
            },
            RenderedRow {
                text: Line::raw("context"),
                binding: RowBinding {
                    row_index: 1,
                    left: Some(position(DiffSide::Left, 3)),
                    right: Some(position(DiffSide::Right, 1)),
                },
            },
            RenderedRow {
                text: Line::raw("@@ -9,1 +9,1 @@"),
                binding: RowBinding {
                    row_index: 2,
                    left: None,
                    right: None,
                },
            },
            RenderedRow {
                text: Line::raw("+added"),
                binding: RowBinding {
                    row_index: 3,
                    left: None,
                    right: Some(position(DiffSide::Right, 9)),
                },
            },
        ],
    });
    refresh_display_rows(&mut state);
    state
}

#[test]
fn unreviewed_hunk_header_shows_its_position_and_the_marking_key() {
    let state = app_with_two_hunk_headers();
    let screen = screen_wide(&draw_wide(&state));

    assert!(screen.contains("hunk 1/2 · M mark"));
    assert!(screen.contains("hunk 2/2 · M mark"));
    assert!(!screen.contains("@@ -3,1 +1,1 @@"));
}

#[test]
fn reviewed_hunk_header_says_so() {
    let mut state = app_with_two_hunk_headers();
    let path = RepoPath("src/app.rs".into());
    state
        .session
        .files
        .get_mut(&path)
        .unwrap()
        .reviewed_hunks
        .insert(1);
    let screen = screen_wide(&draw_wide(&state));

    assert!(screen.contains("hunk 1/2 · M mark"));
    assert!(screen.contains("hunk 2/2 · ✓ reviewed"));
}

#[test]
fn added_line_background_covers_the_line_number_gutter() {
    let mut state = app();
    state.rendered_diff = Some(RenderedDiff {
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
                text: Line::from(ratatui::text::Span::styled(
                    "+added",
                    ratatui::style::Style::default()
                        .bg(ratatui::style::Color::Rgb(0x1c, 0x44, 0x28)),
                )),
                binding: RowBinding {
                    row_index: 1,
                    left: None,
                    right: Some(position(DiffSide::Right, 5)),
                },
            },
        ],
    });
    state.parsed_diff = None;
    state.session.cursor_row = 0;
    refresh_display_rows(&mut state);

    let terminal = draw(&state);
    let buffer = terminal.backend().buffer();
    let (row, text) = (0..24)
        .map(|y| {
            (
                y,
                (0..80)
                    .map(|x| buffer.cell((x, y)).unwrap().symbol())
                    .collect::<String>(),
            )
        })
        .find(|(_, text)| text.contains("+added"))
        .expect("added row rendered");

    let plus = text.find("+added").unwrap() as u16;
    let expected = Some(ratatui::style::Color::Rgb(0x1c, 0x44, 0x28));
    assert_eq!(buffer.cell((plus - 1, row)).unwrap().style().bg, expected);
    assert_eq!(buffer.cell((plus - 6, row)).unwrap().style().bg, expected);
    assert_eq!(buffer.cell((77, row)).unwrap().style().bg, expected);
}

#[test]
fn gap_row_shows_the_hidden_count_and_the_expand_hint() {
    let state = app_with_gap();
    let screen = screen_wide(&draw_wide(&state));

    assert!(screen.contains("· · · 3 hidden lines · · · — z expand"));
}

#[test]
fn expanded_gap_shows_the_cached_context_lines_and_hides_the_gap_hint() {
    let mut state = app_with_gap();
    let path = RepoPath("src/app.rs".into());
    state.file_contexts.insert(
        path,
        vec![
            "line one".into(),
            "line two".into(),
            "line three".into(),
            "line four".into(),
            "line five".into(),
        ],
    );
    state.expanded_gaps.insert(1);
    refresh_display_rows(&mut state);

    let screen = screen(&draw(&state));

    assert!(screen.contains("    2 line two"));
    assert!(screen.contains("    3 line three"));
    assert!(screen.contains("    4 line four"));
    assert!(!screen.contains("hidden lines"));
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
    assert!(lines[anchor + 1].contains("╭─ @you · draft"));
    assert!(
        lines[anchor + 2].trim().starts_with('│'),
        "a blank padding row separates the border from the text"
    );
    assert!(lines[anchor + 3].contains("│   Please double-check this line"));
    assert!(lines[anchor + 5].contains("╰─"));
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
    state.pending_labels.insert(3, "saving comment…");
    state.busy_operations.insert(3);
    let terminal = draw(&state);
    let screen = screen(&terminal);

    assert!(screen.contains("saving comment…"));
}

#[test]
fn status_shows_the_latest_notice() {
    let mut state = app();
    state.notices.push("primeiro aviso".into());
    state.notices.push("move to a code line".into());
    state.notice_ttl = 5;
    let notice_screen = screen(&draw(&state));

    assert!(notice_screen.contains("move to a code line"));
    assert!(!notice_screen.contains("primeiro aviso"));

    // A notice loses to an active error banner.
    state.error_banner = Some("falha ao salvar".into());
    let banner_screen = screen(&draw(&state));
    assert!(banner_screen.contains("falha ao salvar"));
    assert!(!banner_screen.contains("move to a code line"));
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

    assert!(screen.contains("╭─ @you · draft"));
    assert!(screen.contains("│   corpo do comentário"));
    assert!(screen.contains("│   segunda linha"));
    assert!(screen.contains("╰─ e edit · x delete"));
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
    assert!(screen.contains("n/N navigate"));
    assert!(screen.contains("Esc clear"));
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

#[test]
fn diff_line_background_extends_to_the_panel_edge() {
    use ratatui::style::{Color, Style};
    let mut state = app();
    // Simulate delta's plus-line background on the row's content spans.
    let bg = Color::Rgb(0x0e, 0x29, 0x19);
    if let Some(diff) = state.rendered_diff.as_mut() {
        diff.rows[2].text = Line::from(vec![ratatui::text::Span::styled(
            "+added",
            Style::default().bg(bg),
        )]);
    }
    refresh_display_rows(&mut state);

    let terminal = draw_wide(&state);
    let buffer = terminal.backend().buffer();
    let row = (0..30)
        .find(|y| {
            (0..120)
                .map(|x| buffer.cell((x, *y)).unwrap().symbol())
                .collect::<String>()
                .contains("+added")
        })
        .expect("plus row rendered");
    // Far beyond the text, still inside the panel: the background continues.
    let cell = buffer.cell((110, row)).unwrap();
    assert_eq!(
        cell.style().bg,
        Some(bg),
        "background must run edge to edge"
    );
}

#[test]
fn comment_card_border_uses_the_comment_color() {
    let mut state = app();
    state.provider.drafts.push(draft_at_line_5());
    refresh_display_rows(&mut state);

    let terminal = draw_wide(&state);
    let buffer = terminal.backend().buffer();
    let row = (0..30)
        .find(|y| {
            (0..120)
                .map(|x| buffer.cell((x, *y)).unwrap().symbol())
                .collect::<String>()
                .contains("╭─ @you")
        })
        .expect("card header rendered");
    let x = (0..120)
        .find(|x| buffer.cell((*x, row)).unwrap().symbol() == "╭")
        .unwrap();
    assert_eq!(
        buffer.cell((x, row)).unwrap().style().fg,
        Some(theme::COMMENT),
        "card borders must use the comment color"
    );
}

#[test]
fn split_layout_draws_both_sides_with_their_own_line_numbers() {
    let mut state = app_with_two_hunk_headers();
    state.terminal_width = 150;
    state.diff_layout = betterreview::domain::DiffLayout::Split;
    refresh_display_rows(&mut state);

    let mut terminal = Terminal::new(TestBackend::new(150, 20)).unwrap();
    terminal.draw(|frame| render(frame, &state)).unwrap();
    let buffer = terminal.backend().buffer();
    let screen: String = (0..20)
        .map(|y| {
            (0..150)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(screen.contains('│'), "the two columns are separated");
    let context_row = screen
        .lines()
        .find(|line| line.matches("context").count() == 2)
        .expect("a context line shows on both sides");
    assert!(
        context_row.contains("    3") && context_row.contains("    1"),
        "each side keeps its own number: old 3, new 1 — got {context_row:?}"
    );
}

#[test]
fn a_line_missing_on_one_side_is_hatched() {
    let mut state = app_with_two_hunk_headers();
    state.terminal_width = 150;
    state.diff_layout = betterreview::domain::DiffLayout::Split;
    refresh_display_rows(&mut state);

    let mut terminal = Terminal::new(TestBackend::new(150, 20)).unwrap();
    terminal.draw(|frame| render(frame, &state)).unwrap();
    let buffer = terminal.backend().buffer();
    let screen: String = (0..20)
        .map(|y| {
            (0..150)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        screen.contains('░'),
        "the added line has no counterpart on the old side"
    );
}

fn app_with_a_long_file() -> AppState {
    let mut state = app();
    let path = RepoPath("src/app.rs".into());
    let rows: Vec<DiffRow> = (0..60)
        .map(|index| {
            if index == 0 {
                DiffRow {
                    raw: "@@ -1,60 +1,60 @@".into(),
                    kind: DiffRowKind::HunkHeader,
                    old_line: None,
                    new_line: None,
                    left: None,
                    right: None,
                }
            } else {
                DiffRow {
                    raw: format!(" line {index}"),
                    kind: DiffRowKind::Context,
                    old_line: Some(index),
                    new_line: Some(index),
                    left: Some(position(DiffSide::Left, index)),
                    right: Some(position(DiffSide::Right, index)),
                }
            }
        })
        .collect();
    state.parsed_diff = Some(ParsedFileDiff {
        path: path.clone(),
        head: CommitOid("head".into()),
        rows,
        hunks: vec![betterreview::diff::DiffHunk {
            id: 0,
            old_start: 1,
            old_count: 59,
            new_start: 1,
            new_count: 59,
            row_range: 1..60,
        }],
    });
    state.rendered_diff = Some(RenderedDiff {
        rows: (0..60)
            .map(|index| RenderedRow {
                text: Line::raw(format!("line {index}")),
                binding: RowBinding {
                    row_index: index,
                    left: Some(position(DiffSide::Left, index as u32)),
                    right: Some(position(DiffSide::Right, index as u32)),
                },
            })
            .collect(),
    });
    refresh_display_rows(&mut state);
    state
}

#[test]
fn scrolling_pins_the_file_and_hunk_to_the_top() {
    let mut state = app_with_a_long_file();
    state.session.cursor_row = 50;
    refresh_display_rows(&mut state);

    let screen = screen(&draw(&state));
    let pinned = screen
        .lines()
        .find(|line| line.contains("src/app.rs"))
        .expect("a row naming the file");

    assert!(
        pinned.contains("hunk 1/1"),
        "file and hunk are pinned together: {pinned:?}"
    );
    assert!(
        !screen.contains("line 1 "),
        "the body really scrolled away from the top"
    );
}

#[test]
fn the_top_of_the_file_shows_no_pinned_row() {
    let mut state = app_with_a_long_file();
    state.session.cursor_row = 1;
    refresh_display_rows(&mut state);

    let screen = screen(&draw(&state));
    let rows: Vec<&str> = screen.lines().collect();
    let occurrences = rows
        .iter()
        .filter(|line| line.contains("src/app.rs"))
        .count();

    assert_eq!(
        occurrences, 1,
        "the real header is visible, so nothing is pinned above it"
    );
}

fn app_with_a_long_line() -> AppState {
    let mut state = app();
    let long = "x".repeat(200);
    state.rendered_diff = Some(RenderedDiff {
        rows: vec![RenderedRow {
            text: Line::raw(format!("+{long}")),
            binding: RowBinding {
                row_index: 0,
                left: None,
                right: Some(position(DiffSide::Right, 5)),
            },
        }],
    });
    state.parsed_diff = None;
    refresh_display_rows(&mut state);
    state
}

#[test]
fn a_cut_line_says_it_was_cut() {
    let state = app_with_a_long_line();

    let screen = screen(&draw(&state));

    assert!(
        screen.contains('…'),
        "truncation must be visible, not silent"
    );
}

#[test]
fn wrapping_shows_the_whole_line_across_rows() {
    let mut state = app_with_a_long_line();
    state.wrap_lines = true;

    let screen = screen(&draw(&state));
    let body_rows = screen.lines().filter(|line| line.contains("xxxx")).count();

    assert!(
        body_rows >= 4,
        "200 columns of content need several rows in a 48-column panel, got {body_rows}"
    );
    assert!(
        !screen
            .lines()
            .filter(|line| line.contains("xxxx"))
            .any(|line| line.contains('…')),
        "no diff row is cut when wrapping"
    );
}
