use std::collections::BTreeMap;

use betterreview::{
    app::AppState,
    diff::{RenderedDiff, RenderedRow, RowBinding},
    domain::{
        ChangeRequestKey, ChangedFile, CommitOid, DiffPosition, DiffSide, FileStatus,
        PatchAvailability, ProviderCapabilities, ProviderKind, ProviderSnapshot, RepoPath,
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
    app
}

fn draw(state: &AppState) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal.draw(|frame| render(frame, state)).unwrap();
    terminal
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
