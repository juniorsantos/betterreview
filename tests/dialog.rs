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
        ChangeRequestKey, ChangedFile, CommitOid, FileStatus, PatchAvailability,
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
fn dialog_uses_a_rounded_border() {
    let mut app = base_app();
    app.quit_dialog = true;

    let (screen, _) = screen(&app, 80, 24);

    assert!(screen.contains('╭'), "expected a rounded corner glyph");
}

#[test]
fn dialog_shows_its_title_on_the_border() {
    let mut app = base_app();
    app.quit_dialog = true;

    let (screen, _) = screen(&app, 80, 24);

    assert!(screen.contains("Sair da revisão"));
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

    // Find the box: locate the bottom border row (contains '╰').
    let bottom_row = lines
        .iter()
        .position(|line| line.contains('╰'))
        .expect("dialog has a bottom border");
    let hint_row = bottom_row - 1;
    assert!(
        lines[hint_row].contains("j/k mover"),
        "expected the hints line directly above the bottom border, got: {:?}",
        lines[hint_row]
    );
    assert!(lines[hint_row].contains("Esc cancelar"));

    // Keys render in accent bold, labels in muted — the shared key-hint
    // styling rule.
    let byte_offset = lines[hint_row].find("j/k mover").unwrap();
    let key_x = lines[hint_row][..byte_offset].chars().count() as u16;
    let key_cell = buffer.cell((key_x, hint_row as u16)).unwrap();
    assert_eq!(key_cell.fg, betterreview::tui::theme::ACCENT);
    let label_x = key_x + 4; // first char of "mover"
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
        .position(|line| line.contains('╭'))
        .expect("dialog has a top border");
    let bottom_row = lines
        .iter()
        .position(|line| line.contains('╰'))
        .expect("dialog has a bottom border");
    let box_chars: Vec<char> = lines[top_row].chars().collect();
    let box_left = box_chars.iter().position(|&c| c == '╭').unwrap();
    let box_right = box_chars.iter().rposition(|&c| c == '╮').unwrap();

    let hint_text = "j/k mover · Enter confirmar · Esc cancelar";
    let hint_row = bottom_row - 1;
    let hint_chars: Vec<char> = lines[hint_row].chars().collect();
    let hint_start = hint_chars
        .iter()
        .enumerate()
        .position(|(index, &c)| index > box_left && c == 'j')
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
fn dialog_selected_menu_row_gets_a_filled_background_and_the_new_marker() {
    let mut app = base_app();
    app.quit_dialog = true;
    app.quit_selected = 0;

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, &app)).unwrap();
    let buffer = terminal.backend().buffer().clone();
    let (screen, lines) = screen(&app, 80, 24);

    assert!(screen.contains("▶ Sair mantendo o rascunho"));
    assert!(
        !screen.contains("▸ Sair mantendo o rascunho"),
        "old marker must be gone"
    );

    let selected_row = lines
        .iter()
        .position(|line| line.contains("▶ Sair mantendo o rascunho"))
        .expect("selected row rendered");

    // Derive the dialog's left/right columns from the rounded top border
    // (not the selected row's own '│' chars — the underlying files panel
    // also has '│' borders further left on the same screen row).
    let top_row = lines
        .iter()
        .position(|line| line.contains('╭'))
        .expect("dialog has a top border");
    let top_chars: Vec<char> = lines[top_row].chars().collect();
    let box_left = top_chars.iter().position(|&c| c == '╭').unwrap();
    let box_right = top_chars.iter().rposition(|&c| c == '╮').unwrap();

    // The selection background spans the full row width between the
    // borders, not just the marker/text glyphs.
    for x in (box_left as u16 + 1)..(box_right as u16) {
        let cell = buffer.cell((x, selected_row as u16)).unwrap();
        assert_eq!(
            cell.bg,
            betterreview::tui::theme::SELECTION,
            "column {x} of the selected row is missing the fill background"
        );
    }

    let unselected_row = lines
        .iter()
        .position(|line| line.contains("Sair descartando o rascunho"))
        .expect("unselected row rendered");
    assert!(lines[unselected_row].contains("  Sair descartando o rascunho"));
    let cell = buffer
        .cell((box_left as u16 + 1, unselected_row as u16))
        .unwrap();
    assert_ne!(cell.bg, betterreview::tui::theme::SELECTION);
}

#[test]
fn dialog_is_centered_and_never_wider_than_eighty_percent_of_the_area() {
    let mut app = base_app();
    app.quit_dialog = true;

    let (_, lines) = screen(&app, 80, 24);

    let top_row = lines
        .iter()
        .position(|line| line.contains('╭'))
        .expect("dialog has a top border");
    // `symbol()` cells are joined as UTF-8 text, so box-drawing glyphs are
    // multi-byte: locate columns by character index, not byte offset.
    let chars: Vec<char> = lines[top_row].chars().collect();
    let left = chars.iter().position(|&c| c == '╭').unwrap();
    let right = chars.iter().rposition(|&c| c == '╮').unwrap();
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
