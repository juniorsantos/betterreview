use std::collections::BTreeMap;

use betterreview::{
    app::{AppState, SubmissionModal},
    domain::{
        ChangeRequestKey, ChangedFile, CommitOid, DraftComment, DraftId, FileStatus,
        PatchAvailability, ProviderCapabilities, ProviderKind, ProviderSnapshot, RepoPath,
        ReviewOutcome, Support,
    },
    state::{ContentIdentity, FileProgress, ReviewSync, SESSION_SCHEMA_VERSION, SessionSnapshot},
    tui::{render, theme},
};
use ratatui::{Terminal, backend::TestBackend};
use time::OffsetDateTime;

fn app_with_drafts(count: usize) -> AppState {
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
        drafts: (0..count)
            .map(|index| DraftComment {
                id: DraftId(index.to_string()),
                body: format!("draft {index}"),
                selection: None,
                thread_id: None,
            })
            .collect(),
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
        updated_at: OffsetDateTime::UNIX_EPOCH,
    };
    let mut app = AppState::new(provider, session);
    app.submission_modal = Some(SubmissionModal {
        summary: "Ready to merge".into(),
        outcome: ReviewOutcome::Comment,
    });
    app
}

fn screen(state: &AppState, width: u16, height: u16) -> String {
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
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn submission_modal_is_compact_and_has_no_pending_comment_list() {
    let screen = screen(&app_with_drafts(3), 80, 24);

    assert!(screen.contains("3 drafts will be published"));
    assert!(screen.contains("Ready to merge"));
    assert!(!screen.contains("Pending comments"));
    assert!(!screen.contains("src/app.rs:42"));
}

#[test]
fn the_active_verdict_is_named_on_the_title_bar() {
    let mut app = app_with_drafts(1);
    assert!(screen(&app, 80, 24).contains("Submit review · COMMENT"));

    app.submission_modal.as_mut().unwrap().outcome = ReviewOutcome::Approve;
    assert!(screen(&app, 80, 24).contains("Submit review · APPROVE"));

    app.submission_modal.as_mut().unwrap().outcome = ReviewOutcome::RequestChanges;
    assert!(screen(&app, 80, 24).contains("Submit review · REQUEST CHANGES"));
}

#[test]
fn the_summary_carries_a_caret_and_the_verdicts_are_shortcuts() {
    let screen = screen(&app_with_drafts(1), 80, 24);

    assert!(
        screen.contains("Ready to merge▌"),
        "the caret marks the focus"
    );
    assert!(
        screen.contains("COMMENT"),
        "the active verdict remains visible"
    );
    assert!(screen.contains("APPROVE"));
    assert!(screen.contains("REQUEST CHANGES"));
    assert!(
        screen.contains("⇥ verdict"),
        "Tab always reaches the app, unlike option+letter on macOS"
    );
}

#[test]
fn verdict_actions_use_outlines_with_theme_background() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| render(frame, &app_with_drafts(1)))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let (action_row, action_line) = (0..24)
        .find_map(|y| {
            let line = (0..80)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect::<String>();
            (line.contains("APPROVE")
                && line.contains("REQUEST CHANGES")
                && line.contains("COMMENT"))
            .then_some((y, line))
        })
        .expect("action buttons rendered");

    for label in ["APPROVE", "REQUEST CHANGES", "COMMENT"] {
        let byte = action_line.find(label).expect("button label rendered");
        let x = action_line[..byte].chars().count() as u16;
        let cell = buffer.cell((x, action_row)).unwrap();
        assert_eq!(cell.fg, theme::BG);
        assert_eq!(
            cell.bg,
            if label == "COMMENT" {
                theme::ACCENT_SOFT
            } else {
                theme::ACCENT
            }
        );
        if label == "COMMENT" {
            let style = cell.style();
            assert!(style.add_modifier.contains(ratatui::style::Modifier::BOLD));
            assert!(
                style
                    .add_modifier
                    .contains(ratatui::style::Modifier::UNDERLINED)
            );
        }
    }
}

#[test]
fn changing_the_verdict_does_not_move_button_labels() {
    let mut app = app_with_drafts(1);
    let positions = |app: &AppState| {
        let screen = screen(app, 80, 24);
        let line = screen
            .lines()
            .find(|line| {
                line.contains("APPROVE")
                    && line.contains("REQUEST CHANGES")
                    && line.contains("COMMENT")
            })
            .expect("action buttons rendered");
        [
            line.find("APPROVE").unwrap(),
            line.find("REQUEST CHANGES").unwrap(),
            line.find("COMMENT").unwrap(),
        ]
    };

    let comment = positions(&app);
    app.submission_modal.as_mut().unwrap().outcome = ReviewOutcome::Approve;
    let approve = positions(&app);
    app.submission_modal.as_mut().unwrap().outcome = ReviewOutcome::RequestChanges;
    let request_changes = positions(&app);

    assert_eq!(comment, approve);
    assert_eq!(approve, request_changes);
}

#[test]
fn submission_modal_uses_square_corners() {
    let screen = screen(&app_with_drafts(1), 80, 24);

    assert!(screen.contains("┌ Submit review"));
    assert!(!screen.contains("╭ Submit review"));
}

#[test]
fn a_supported_outcome_leaves_no_room_for_an_availability_warning() {
    let screen = screen(&app_with_drafts(1), 80, 24);

    assert!(!screen.contains("unavailable"));
    assert!(
        !screen.contains("Comment on the review"),
        "the action line only repeated the verdict already on the title"
    );
}

#[test]
fn unsupported_outcome_stays_visible_with_its_reason() {
    let mut app = app_with_drafts(1);
    app.provider.capabilities.request_changes = Support::Unsupported {
        reason: "Requires GitLab 17.3 or newer".into(),
    };
    app.submission_modal.as_mut().unwrap().outcome = ReviewOutcome::RequestChanges;

    let screen = screen(&app, 80, 24);

    assert!(screen.contains("REQUEST CHANGES"));
    assert!(screen.contains("Requires GitLab 17.3 or newer"));
    assert!(screen.contains("unavailable"));
}

#[test]
fn submission_modal_remains_usable_on_a_small_terminal() {
    let screen = screen(&app_with_drafts(2), 50, 16);

    assert!(screen.contains("Submit review"));
    assert!(screen.contains("2 drafts"));
    assert!(screen.contains("APPROVE"));
}

fn redact_version(screen: String) -> String {
    screen.replace(concat!("v", env!("CARGO_PKG_VERSION")), "vX.Y.Z")
}

#[test]
fn submission_modal_snapshots_cover_regular_and_small_terminals() {
    insta::assert_snapshot!(
        "regular_80x24",
        redact_version(screen(&app_with_drafts(3), 80, 24))
    );
    insta::assert_snapshot!(
        "small_50x16",
        redact_version(screen(&app_with_drafts(2), 50, 16))
    );
}

#[test]
fn the_modal_grows_with_the_summary_instead_of_leaving_dead_rows() {
    let one_line = screen(&app_with_drafts(1), 80, 24);

    let mut app = app_with_drafts(1);
    app.submission_modal.as_mut().unwrap().summary = "one\ntwo\nthree".into();
    let three_lines = screen(&app, 80, 24);

    let box_height = |screen: &str| {
        let lines: Vec<&str> = screen.lines().collect();
        let top = lines
            .iter()
            .position(|line| line.contains("┌ Submit review"))
            .expect("modal top border");
        let bottom = lines
            .iter()
            .position(|line| line.contains('└'))
            .expect("modal bottom border");
        bottom - top
    };
    assert!(
        box_height(&three_lines) > box_height(&one_line),
        "a multi-line summary must make the dialog taller, not scroll away"
    );
    assert!(three_lines.contains("three"));
}
