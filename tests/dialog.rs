//! TDD coverage for the unified `Dialog` component (src/tui/widgets/dialog.rs).
//!
//! The component itself is `pub(in crate::tui)`, so — consistent with every
//! other TUI test in this suite — it is exercised through the public
//! `betterreview::tui::render` entry point rather than called directly. The
//! quit confirmation dialog is the simplest consumer to drive: it needs no
//! draft/thread data, just `quit_dialog = true`.

use std::collections::BTreeMap;

use betterreview::{
    app::AppState,
    domain::{
        ChangeRequestKey, ChangedFile, CommitOid, DraftId, FileStatus, PatchAvailability,
        ProviderCapabilities, ProviderKind, ProviderSnapshot, RepoPath,
    },
    state::{ContentIdentity, FileProgress, ReviewSync, SESSION_SCHEMA_VERSION, SessionSnapshot},
    tui::render,
};
use ratatui::{Terminal, backend::TestBackend};

fn base_app() -> AppState {
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

fn screen(state: &AppState, width: u16, height: u16) -> (String, Vec<String>) {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, state)).unwrap();
    let buffer = terminal.backend().buffer();
    let lines: Vec<String> = (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect::<String>()
        })
        .collect();
    (lines.join("\n"), lines)
}

#[test]
fn dialog_uses_a_square_border() {
    let mut app = base_app();
    app.quit_dialog = true;

    let (screen, _) = screen(&app, 80, 24);

    assert!(screen.contains('┌'), "expected a square corner glyph");
    assert!(!screen.contains('╭'), "rounded corners must not be used");
}

#[test]
fn dialog_shows_its_title_on_the_border() {
    let mut app = base_app();
    app.quit_dialog = true;

    let (screen, _) = screen(&app, 80, 24);

    assert!(screen.contains("Quit review"));
}

#[test]
fn dialog_places_hints_as_the_last_inner_line_with_key_styling() {
    let mut app = base_app();
    app.quit_dialog = true;

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, &app)).unwrap();
    let buffer = terminal.backend().buffer().clone();
    let (_, lines) = screen(&app, 80, 24);

    let hint_row = lines
        .iter()
        .position(|line| line.contains("⇥ move"))
        .expect("dialog has a hints row");
    assert!(lines[hint_row + 1].contains('└'));
    assert!(
        lines[hint_row].contains("⇥ move"),
        "expected the hints line directly above the bottom border, got: {:?}",
        lines[hint_row]
    );
    assert!(lines[hint_row].contains("⎋ cancel"));

    // Keys render in accent bold, labels in muted — the shared key-hint
    // styling rule.
    let byte_offset = lines[hint_row].find("⇥ move").unwrap();
    let key_x = lines[hint_row][..byte_offset].chars().count() as u16;
    let key_cell = buffer.cell((key_x, hint_row as u16)).unwrap();
    assert_eq!(key_cell.fg, betterreview::tui::theme::ACCENT);
    let label_x = key_x + 2;
    let label_cell = buffer.cell((label_x, hint_row as u16)).unwrap();
    assert_eq!(label_cell.fg, betterreview::tui::theme::MUTED);
}

#[test]
fn dialog_hints_line_is_horizontally_centered() {
    let mut app = base_app();
    app.quit_dialog = true;

    let (_, lines) = screen(&app, 80, 24);

    let top_row = lines
        .iter()
        .position(|line| line.contains("┌ Quit review"))
        .expect("dialog has a top border");
    let hint_row = lines
        .iter()
        .position(|line| line.contains("⇥ move"))
        .expect("dialog has a hints row");
    let box_chars: Vec<char> = lines[top_row].chars().collect();
    let box_left = box_chars.iter().position(|&c| c == '┌').unwrap();
    let box_right = box_chars.iter().rposition(|&c| c == '┐').unwrap();

    let hint_text = "⇥ move · ↵ confirm · ⎋ cancel";
    let hint_chars: Vec<char> = lines[hint_row].chars().collect();
    let hint_start = hint_chars
        .iter()
        .enumerate()
        .position(|(index, &c)| index > box_left && c == '⇥')
        .expect("hint text present on the row above the bottom border");
    let hint_end = hint_start + hint_text.chars().count();

    let left_margin = hint_start - (box_left + 1);
    let right_margin = box_right - hint_end;
    assert!(
        (left_margin as isize - right_margin as isize).abs() <= 1,
        "hints not centered: left_margin={left_margin} right_margin={right_margin}"
    );
}

#[test]
fn dialog_actions_use_compact_background_only_buttons() {
    let mut app = base_app();
    app.quit_dialog = true;
    app.quit_selected = 0;

    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, &app)).unwrap();
    let buffer = terminal.backend().buffer().clone();
    let (screen, lines) = screen(&app, 120, 30);

    assert!(!screen.contains('▶'));

    for (index, label) in [
        "Quit keeping the draft",
        "Quit discarding the draft",
        "Cancel",
    ]
    .into_iter()
    .enumerate()
    {
        let row = lines
            .iter()
            .position(|line| line.contains(label))
            .expect("action rendered");
        let byte = lines[row].find(label).unwrap();
        let x = lines[row][..byte].chars().count() as u16;
        let label_cell = buffer.cell((x, row as u16)).unwrap();
        let background = if index == 0 {
            betterreview::tui::theme::ACCENT_SOFT
        } else {
            betterreview::tui::theme::ACCENT
        };
        assert_eq!(label_cell.fg, betterreview::tui::theme::BG);
        assert_eq!(label_cell.bg, background);
        let label_width = label.chars().count() as u16;
        for padding_x in [x - 1, x + label_width] {
            assert_eq!(buffer.cell((padding_x, row as u16)).unwrap().bg, background);
        }
        for padding_y in [row as u16 - 1, row as u16 + 1] {
            assert_ne!(buffer.cell((x, padding_y)).unwrap().bg, background);
        }
    }
    let action_rows = [
        "Quit keeping the draft",
        "Quit discarding the draft",
        "Cancel",
    ]
    .map(|label| {
        lines
            .iter()
            .position(|line| line.contains(label))
            .expect("action rendered")
    });
    assert!(action_rows.windows(2).all(|rows| rows[0] == rows[1]));
    let selected_row = action_rows[0];
    let byte = lines[selected_row].find("Quit keeping the draft").unwrap();
    let selected_x = lines[selected_row][..byte].chars().count() as u16;
    let selected_style = buffer
        .cell((selected_x, selected_row as u16))
        .unwrap()
        .style();
    assert!(
        selected_style
            .add_modifier
            .contains(ratatui::style::Modifier::BOLD)
    );
    assert!(
        !selected_style
            .add_modifier
            .contains(ratatui::style::Modifier::UNDERLINED)
    );
}

#[test]
fn quit_actions_are_centered_below_a_blank_row() {
    let mut app = base_app();
    app.quit_dialog = true;

    let (_, lines) = screen(&app, 120, 30);
    let top = lines
        .iter()
        .position(|line| line.contains("┌ Quit review "))
        .expect("quit dialog rendered");
    let left_byte = lines[top].find("┌ Quit review ").unwrap();
    let left = lines[top][..left_byte].chars().count();
    let right = left
        + lines[top]
            .chars()
            .skip(left)
            .position(|character| character == '┐')
            .unwrap();
    let actions = lines
        .iter()
        .position(|line| {
            line.contains("Quit keeping the draft")
                && line.contains("Quit discarding the draft")
                && line.contains("Cancel")
        })
        .expect("quit actions rendered");
    let first_byte = lines[actions].find("Quit keeping the draft").unwrap();
    let first = lines[actions][..first_byte].chars().count() - 1;
    let last_byte = lines[actions].find("Cancel").unwrap();
    let last = lines[actions][..last_byte].chars().count() + "Cancel".chars().count() + 1;
    let left_margin = first - (left + 1);
    let right_margin = right - last;
    let top_space: String = lines[top + 1]
        .chars()
        .skip(left + 1)
        .take(right - left - 1)
        .collect();

    assert_eq!(actions, top + 2);
    assert!(top_space.trim().is_empty());
    assert!((left_margin as isize - right_margin as isize).abs() <= 1);
}

#[test]
fn delete_actions_follow_the_quit_dialog_spacing_and_alignment() {
    let mut app = base_app();
    app.delete_dialog = Some(DraftId("draft-1".into()));

    let (_, lines) = screen(&app, 120, 30);
    let top = lines
        .iter()
        .position(|line| line.contains("┌ Delete comment "))
        .expect("delete dialog rendered");
    let border: Vec<char> = lines[top].chars().collect();
    let left = border
        .iter()
        .position(|character| *character == '┌')
        .unwrap();
    let right = border
        .iter()
        .rposition(|character| *character == '┐')
        .unwrap();
    let actions = lines
        .iter()
        .position(|line| line.contains("Delete") && line.contains("Cancel"))
        .expect("delete actions rendered");
    let first_byte = lines[actions].find("Delete").unwrap();
    let first = lines[actions][..first_byte].chars().count() - 1;
    let last_byte = lines[actions].find("Cancel").unwrap();
    let last = lines[actions][..last_byte].chars().count() + "Cancel".chars().count() + 1;
    let top_space: String = lines[top + 1]
        .chars()
        .skip(left + 1)
        .take(right - left - 1)
        .collect();

    assert_eq!(actions, top + 2);
    assert!(top_space.trim().is_empty());
    assert!(
        ((first - (left + 1)) as isize - (right - last) as isize).abs() <= 1,
        "delete actions are not centered"
    );
}

#[test]
fn quit_actions_stay_inline_on_a_small_terminal() {
    let mut app = base_app();
    app.quit_dialog = true;

    let (_, lines) = screen(&app, 50, 16);
    assert!(
        lines
            .iter()
            .any(|line| line.contains("Quit keeping the draft"))
    );
    assert!(!lines.iter().any(|line| line.contains("Keep draft")));
}

#[test]
fn dialog_is_centered_and_never_wider_than_eighty_percent_of_the_area() {
    let mut app = base_app();
    app.quit_dialog = true;

    let (_, lines) = screen(&app, 80, 24);

    let top_row = lines
        .iter()
        .position(|line| line.contains("┌ Quit review"))
        .expect("dialog has a top border");
    // `symbol()` cells are joined as UTF-8 text, so box-drawing glyphs are
    // multi-byte: locate columns by character index, not byte offset.
    let chars: Vec<char> = lines[top_row].chars().collect();
    let left = chars.iter().position(|&c| c == '┌').unwrap();
    let right = chars.iter().rposition(|&c| c == '┐').unwrap();
    let width = right - left + 1;

    assert!(
        width <= 80 * 4 / 5,
        "dialog width {width} exceeds 80% of the 80-column area"
    );

    // Centered: left margin and right margin should be within one column of
    // each other.
    let right_margin = 80 - (right + 1);
    assert!(
        (left as isize - right_margin as isize).abs() <= 1,
        "dialog is not centered: left={left} right_margin={right_margin}"
    );
}

use betterreview::tui::{Dialog, Sizing, Zone, render_dialog};
use ratatui::text::Line;

fn draw_dialog_in(dialog: Dialog<'_>, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| {
            let area = frame.area();
            render_dialog(frame, area, dialog);
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn draw_dialog(dialog: Dialog<'_>) -> String {
    draw_dialog_in(dialog, 100, 30)
}

fn box_height(screen: &str) -> usize {
    screen.lines().filter(|line| line.contains('│')).count() + 2
}

fn box_width(screen: &str) -> usize {
    screen
        .lines()
        .find(|line| line.contains('┌'))
        .map(|line| line.trim_end().len() - line.find('┌').unwrap())
        .unwrap_or(0)
}

#[test]
fn a_dialog_sizes_itself_to_its_content() {
    let short = draw_dialog(Dialog {
        title: Line::raw(" Title "),
        body: vec![Line::raw("one line")],
        hints: "Esc close",
        sizing: Sizing::Content { max_width: 60 },
        zones: Vec::new(),
    });
    let tall = draw_dialog(Dialog {
        title: Line::raw(" Title "),
        body: (0..8).map(|i| Line::raw(format!("line {i}"))).collect(),
        hints: "Esc close",
        sizing: Sizing::Content { max_width: 60 },
        zones: Vec::new(),
    });

    assert!(
        box_height(&tall) > box_height(&short),
        "more content, taller box"
    );
    assert!(box_width(&short) <= 60, "never wider than the maximum");
    assert!(
        box_width(&short) >= "one line".len() + 2,
        "wide enough for the content"
    );
}

#[test]
fn a_dialog_never_outgrows_its_area() {
    let screen = draw_dialog_in(
        Dialog {
            title: Line::raw(" Title "),
            body: (0..200).map(|i| Line::raw(format!("line {i}"))).collect(),
            hints: "Esc close",
            sizing: Sizing::Content { max_width: 200 },
            zones: Vec::new(),
        },
        40,
        12,
    );

    assert!(box_height(&screen) <= 12);
    assert!(box_width(&screen) <= 40);
}

#[test]
fn zones_split_the_interior_vertically() {
    let screen = draw_dialog(Dialog {
        title: Line::raw(" Search "),
        body: vec![Line::raw("results go here")],
        hints: "Enter open",
        sizing: Sizing::Fixed {
            width: 60,
            height: 12,
        },
        zones: vec![Zone::Fill, Zone::Fixed(3)],
    });

    assert!(screen.contains("results go here"));
    assert!(
        screen.lines().filter(|line| line.contains('│')).count() >= 8,
        "the box is drawn with room for both zones"
    );
}

#[test]
fn a_blame_that_cannot_run_explains_itself_in_a_dialog() {
    let mut state = base_app();
    state.blocked = Some(betterreview::app::Blocked {
        title: "Blame unavailable".into(),
        reason: "fatal: bad object 9f8e7d6".into(),
        guidance: "the base commit is not in this clone; fetch the base branch and try again"
            .into(),
    });

    let (screen, _) = screen(&state, 100, 30);

    assert!(screen.contains("Blame unavailable"), "{screen}");
    assert!(
        screen.contains("fatal: bad object"),
        "what the tool actually said has to survive, not a guess about it:\n{screen}"
    );
    assert!(
        screen.contains("fetch the base branch"),
        "and the reader needs to know what to do about it"
    );
}
