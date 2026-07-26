use std::collections::BTreeMap;

use betterreview::{
    app::{AppState, SubmissionModal},
    domain::{
        ChangeRequestKey, ChangedFile, CommitOid, DraftComment, DraftId, FileStatus,
        PatchAvailability, ProviderCapabilities, ProviderKind, ProviderSnapshot, RepoPath,
        ReviewOutcome, Support,
    },
    state::{ContentIdentity, FileProgress, ReviewSync, SESSION_SCHEMA_VERSION, SessionSnapshot},
    tui::render,
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
        selected_field: 1,
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

    assert!(screen.contains("3 drafts serão publicados"));
    assert!(screen.contains("COMENTAR"));
    assert!(screen.contains("APROVAR"));
    assert!(screen.contains("PEDIR MUDANÇAS"));
    assert!(screen.contains("Ready to merge"));
    assert!(!screen.contains("Pending comments"));
    assert!(!screen.contains("src/app.rs:42"));
}

#[test]
fn unsupported_outcome_stays_visible_with_its_reason() {
    let mut app = app_with_drafts(1);
    app.provider.capabilities.request_changes = Support::Unsupported {
        reason: "Requires GitLab 17.3 or newer".into(),
    };
    app.submission_modal.as_mut().unwrap().outcome = ReviewOutcome::RequestChanges;

    let screen = screen(&app, 80, 24);

    assert!(screen.contains("PEDIR MUDANÇAS"));
    assert!(screen.contains("Requires GitLab 17.3 or newer"));
    assert!(screen.contains("indisponível"));
}

#[test]
fn submission_modal_remains_usable_on_a_small_terminal() {
    let screen = screen(&app_with_drafts(2), 50, 16);

    assert!(screen.contains("Enviar revisão"));
    assert!(screen.contains("2 drafts"));
    // The full hints line ("Tab campo · ↑/↓ resultado · Enter enviar · Esc
    // cancelar") does not fit inside the dialog's 80%-of-area width cap on a
    // terminal this narrow — confirm what's visible still starts correctly.
    assert!(screen.contains("Tab campo"));
}

/// A versão do crate aparece no cabeçalho renderizado; sem a redação os
/// snapshots quebrariam a cada bump automático de release.
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
