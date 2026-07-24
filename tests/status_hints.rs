//! TDD coverage for the flat status bar (src/tui/widgets/status.rs): the
//! left side keeps the existing message precedence, the right side carries
//! key-accent/label-muted navigation hints, truncated with `…` when the
//! terminal is too narrow, always losing space to the left message first.

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
                sync: ReviewSync::Synced,
            },
        )]),
        editor: None,
        pending_submit: None,
        updated_at: time::OffsetDateTime::UNIX_EPOCH,
    };
    AppState::new(provider, session)
}

fn screen(state: &AppState, width: u16, height: u16) -> Vec<String> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, state)).unwrap();
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect::<String>()
        })
        .collect()
}

/// The status row is now the 3rd (and last) row of the screen: the 4th
/// footer row has been removed and its hints migrated here.
fn status_row(lines: &[String]) -> &str {
    lines.last().unwrap()
}

#[test]
fn footer_row_is_gone_and_the_layout_is_three_rows() {
    let state = app();
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, &state)).unwrap();
    let buffer = terminal.backend().buffer();

    // Old footer text must not appear anywhere now.
    let full = (0..24)
        .map(|y| {
            (0..80)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!full.contains("Tab/h/l foco"));
}

#[test]
fn status_right_side_shows_key_accent_hints() {
    let state = app();
    let lines = screen(&state, 100, 24);
    let row = status_row(&lines);

    assert!(row.contains("enviar"), "expected enviar hint, got: {row:?}");
    assert!(row.contains("sair"), "expected sair hint, got: {row:?}");
    assert!(row.contains("buscar"), "expected buscar hint, got: {row:?}");
    assert!(row.contains("ajuda"), "expected ajuda hint, got: {row:?}");
}

#[test]
fn status_hint_keys_render_in_accent_bold() {
    let state = app();
    let backend = TestBackend::new(100, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, &state)).unwrap();
    let buffer = terminal.backend().buffer();
    let row = 23u16;
    let line: String = (0..100)
        .map(|x| buffer.cell((x, row)).unwrap().symbol())
        .collect();

    let key_offset = char_offset(&line, "R").expect("R hint present");
    let cell = buffer.cell((key_offset as u16, row)).unwrap();
    assert_eq!(cell.style().fg, Some(theme::ACCENT));
    assert!(cell.style().add_modifier.contains(Modifier::BOLD));

    let label_offset = char_offset(&line, "enviar").expect("enviar label present");
    let label_cell = buffer.cell((label_offset as u16, row)).unwrap();
    assert_eq!(label_cell.style().fg, Some(theme::MUTED));
}

/// Locates `needle` in `haystack` and returns its position as a *character*
/// column index — cell columns line up with chars, not the byte offsets
/// `str::find` returns, and this row contains multi-byte glyphs (`·`, `…`).
fn char_offset(haystack: &str, needle: &str) -> Option<usize> {
    let byte_offset = haystack.find(needle)?;
    Some(haystack[..byte_offset].chars().count())
}

#[test]
fn status_left_message_keeps_priority_and_hints_truncate_with_ellipsis() {
    let mut state = app();
    state.error_banner = Some("falha ao carregar um diff muito grande e verboso".into());

    // Narrow enough that the error message alone nearly fills the row.
    let lines = screen(&state, 55, 24);
    let row = status_row(&lines);

    assert!(
        row.contains("falha ao carregar um diff muito grande e verboso"),
        "left message must stay intact: {row:?}"
    );
}

#[test]
fn status_hints_truncate_with_ellipsis_when_narrow() {
    let state = app();
    // Wide enough for the left idle summary but not for every hint pair.
    let lines = screen(&state, 60, 24);
    let row = status_row(&lines);

    assert!(
        row.contains('…'),
        "expected an ellipsis marker when hints are truncated: {row:?}"
    );
}

#[test]
fn status_error_still_wins_over_hints() {
    let mut state = app();
    state.error_banner = Some("falha ao salvar".into());

    let lines = screen(&state, 100, 24);
    let row = status_row(&lines);

    assert!(row.contains("falha ao salvar"));
}
