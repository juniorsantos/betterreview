use std::collections::BTreeMap;

use betterreview::{
    app::{AppAction, AppEvent, AppFocus, AppState, CommentEntry, DisplayRow, SubmissionModal},
    domain::{
        ChangeRequestKey, ChangedFile, CommitOid, DiffPosition, DiffSelection, DiffSide,
        FileStatus, PatchAvailability, ProviderCapabilities, ProviderKind, ProviderSnapshot,
        RepoPath, ReviewOutcome, ThreadId,
    },
    state::{
        ContentIdentity, EditorSnapshot, FileProgress, ReviewSync, SESSION_SCHEMA_VERSION,
        SessionSnapshot,
    },
    tui::{KeyMap, handle_key},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use time::OffsetDateTime;

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
        updated_at: OffsetDateTime::UNIX_EPOCH,
    };
    AppState::new(provider, session)
}

fn selection() -> DiffSelection {
    let position = DiffPosition {
        path: RepoPath("src/app.rs".into()),
        side: DiffSide::Right,
        line: 1,
        hunk: 0,
    };
    DiffSelection {
        start: position.clone(),
        end: position,
    }
}

fn app_with_editor(lines: Vec<String>) -> AppState {
    let mut app = base_app();
    app.session.editor = Some(EditorSnapshot {
        lines,
        cursor_row: 0,
        grapheme_col: 0,
        original_head: CommitOid("head".into()),
        path: RepoPath("src/app.rs".into()),
        selection: selection(),
        stale: false,
    });
    app.editor_open = true;
    app
}

fn app_with_modal() -> AppState {
    let mut app = base_app();
    app.submission_modal = Some(SubmissionModal {
        summary: "Ready to merge".into(),
        outcome: ReviewOutcome::Comment,
        selected_field: 0,
    });
    app
}

fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

#[test]
fn enter_saves_the_open_comment_editor() {
    let mut app = app_with_editor(vec!["primeira linha".into()]);
    let mut keymap = KeyMap::default();

    let event = handle_key(
        &mut app,
        &mut keymap,
        key(KeyCode::Enter, KeyModifiers::NONE),
    );

    match event {
        Some(AppEvent::Action(AppAction::CreateDraft(input))) => {
            assert_eq!(input.body.0, "primeira linha");
        }
        other => panic!("expected CreateDraft, got {other:?}"),
    }
}

#[test]
fn alt_enter_inserts_a_newline_instead_of_saving() {
    let mut app = app_with_editor(vec!["linha".into()]);
    let mut keymap = KeyMap::default();

    let event = handle_key(
        &mut app,
        &mut keymap,
        key(KeyCode::Enter, KeyModifiers::ALT),
    );

    assert!(event.is_none());
    assert_eq!(app.session.editor.as_ref().unwrap().lines.len(), 2);
}

#[test]
fn ctrl_s_remains_an_alias_to_save_the_editor() {
    let mut app = app_with_editor(vec!["corpo".into()]);
    let mut keymap = KeyMap::default();

    let event = handle_key(
        &mut app,
        &mut keymap,
        key(KeyCode::Char('s'), KeyModifiers::CONTROL),
    );

    assert!(matches!(
        event,
        Some(AppEvent::Action(AppAction::CreateDraft(_)))
    ));
}

#[test]
fn enter_submits_the_review_from_the_modal() {
    let mut app = app_with_modal();
    let mut keymap = KeyMap::default();

    let event = handle_key(
        &mut app,
        &mut keymap,
        key(KeyCode::Enter, KeyModifiers::NONE),
    );

    match event {
        Some(AppEvent::Action(AppAction::SubmitReview { summary, outcome })) => {
            assert_eq!(summary, "Ready to merge");
            assert_eq!(outcome, ReviewOutcome::Comment);
        }
        other => panic!("expected SubmitReview, got {other:?}"),
    }
}

#[test]
fn quit_dialog_navigates_with_jk_and_confirms_with_enter() {
    let mut app = base_app();
    app.quit_dialog = true;
    let mut keymap = KeyMap::default();

    // Highlight starts on "Manter sessão"; j moves to "Descartar editor".
    let moved = handle_key(
        &mut app,
        &mut keymap,
        key(KeyCode::Char('j'), KeyModifiers::NONE),
    );
    assert!(moved.is_none());
    assert_eq!(app.quit_selected, 1);

    let event = handle_key(
        &mut app,
        &mut keymap,
        key(KeyCode::Enter, KeyModifiers::NONE),
    );
    assert!(matches!(
        event,
        Some(AppEvent::Action(AppAction::ConfirmQuit(
            betterreview::app::QuitChoice::DiscardEditor
        )))
    ));
}

#[test]
fn quit_dialog_enter_defaults_to_keep_session() {
    let mut app = base_app();
    app.quit_dialog = true;
    let mut keymap = KeyMap::default();

    let event = handle_key(
        &mut app,
        &mut keymap,
        key(KeyCode::Enter, KeyModifiers::NONE),
    );

    assert!(matches!(
        event,
        Some(AppEvent::Action(AppAction::ConfirmQuit(
            betterreview::app::QuitChoice::KeepSession
        )))
    ));
}

#[test]
fn quit_dialog_esc_cancels() {
    let mut app = base_app();
    app.quit_dialog = true;
    let mut keymap = KeyMap::default();

    let event = handle_key(&mut app, &mut keymap, key(KeyCode::Esc, KeyModifiers::NONE));

    assert!(matches!(
        event,
        Some(AppEvent::Action(AppAction::ConfirmQuit(
            betterreview::app::QuitChoice::Cancel
        )))
    ));
}

#[test]
fn esc_closes_the_help_overlay() {
    let mut app = base_app();
    app.help_visible = true;
    let mut keymap = KeyMap::default();

    let event = handle_key(&mut app, &mut keymap, key(KeyCode::Esc, KeyModifiers::NONE));

    assert!(matches!(
        event,
        Some(AppEvent::Action(AppAction::ToggleHelp))
    ));
}

#[test]
fn q_closes_the_help_overlay_without_quitting() {
    let mut app = base_app();
    app.help_visible = true;
    let mut keymap = KeyMap::default();

    let event = handle_key(
        &mut app,
        &mut keymap,
        key(KeyCode::Char('q'), KeyModifiers::NONE),
    );

    assert!(matches!(
        event,
        Some(AppEvent::Action(AppAction::ToggleHelp))
    ));
}

#[test]
fn other_keys_do_not_leak_through_the_help_overlay() {
    let mut app = base_app();
    app.help_visible = true;
    let mut keymap = KeyMap::default();

    let event = handle_key(
        &mut app,
        &mut keymap,
        key(KeyCode::Char('j'), KeyModifiers::NONE),
    );

    assert!(event.is_none());
}

#[test]
fn ctrl_s_remains_an_alias_to_submit_the_review() {
    let mut app = app_with_modal();
    let mut keymap = KeyMap::default();

    let event = handle_key(
        &mut app,
        &mut keymap,
        key(KeyCode::Char('s'), KeyModifiers::CONTROL),
    );

    assert!(matches!(
        event,
        Some(AppEvent::Action(AppAction::SubmitReview { .. }))
    ));
}

#[test]
fn r_replies_when_cursor_is_on_a_thread_comment() {
    let mut app = base_app();
    app.focus = AppFocus::Diff;
    let thread = ThreadId("t1".into());
    app.display_rows = vec![DisplayRow::Comment {
        entry: CommentEntry::Thread {
            thread: thread.clone(),
            comment_index: 0,
        },
        kind: betterreview::app::CommentRowKind::Header,
        text: "please explain".into(),
        author: Some("alice".into()),
    }];
    app.display_cursor = 0;
    let mut keymap = KeyMap::default();

    let event = handle_key(
        &mut app,
        &mut keymap,
        key(KeyCode::Char('r'), KeyModifiers::NONE),
    );

    assert!(matches!(
        event,
        Some(AppEvent::Action(AppAction::ReplyComment(id))) if id == thread
    ));
}

#[test]
fn r_refreshes_elsewhere() {
    let mut app = base_app();
    app.focus = AppFocus::Diff;
    app.display_rows = vec![DisplayRow::Diff { row: 0 }];
    app.display_cursor = 0;
    let mut keymap = KeyMap::default();

    let event = handle_key(
        &mut app,
        &mut keymap,
        key(KeyCode::Char('r'), KeyModifiers::NONE),
    );

    assert!(matches!(event, Some(AppEvent::Action(AppAction::Refresh))));
}

#[test]
fn e_edits_when_cursor_is_on_a_draft_comment() {
    let mut app = base_app();
    app.focus = AppFocus::Diff;
    let draft_id = betterreview::domain::DraftId("d1".into());
    app.display_rows = vec![DisplayRow::Comment {
        entry: CommentEntry::Draft {
            id: draft_id.clone(),
        },
        kind: betterreview::app::CommentRowKind::Header,
        text: "please fix".into(),
        author: None,
    }];
    app.display_cursor = 0;
    let mut keymap = KeyMap::default();

    let event = handle_key(
        &mut app,
        &mut keymap,
        key(KeyCode::Char('e'), KeyModifiers::NONE),
    );

    assert!(matches!(
        event,
        Some(AppEvent::Action(AppAction::EditComment(id))) if id == draft_id
    ));
}

#[test]
fn x_deletes_when_cursor_is_on_a_draft_comment() {
    let mut app = base_app();
    app.focus = AppFocus::Diff;
    let draft_id = betterreview::domain::DraftId("d1".into());
    app.display_rows = vec![DisplayRow::Comment {
        entry: CommentEntry::Draft {
            id: draft_id.clone(),
        },
        kind: betterreview::app::CommentRowKind::Header,
        text: "please fix".into(),
        author: None,
    }];
    app.display_cursor = 0;
    let mut keymap = KeyMap::default();

    let event = handle_key(
        &mut app,
        &mut keymap,
        key(KeyCode::Char('x'), KeyModifiers::NONE),
    );

    assert!(matches!(
        event,
        Some(AppEvent::Action(AppAction::DeleteComment(id))) if id == draft_id
    ));
}

#[test]
fn slash_enters_search_input_and_chars_do_not_leak_to_the_keymap() {
    let mut app = base_app();
    app.focus = AppFocus::Diff;
    let mut keymap = KeyMap::default();

    let event = handle_key(
        &mut app,
        &mut keymap,
        key(KeyCode::Char('/'), KeyModifiers::NONE),
    );
    assert!(event.is_none());
    assert_eq!(app.search_input.as_deref(), Some(""));

    // A key that would otherwise move the cursor must be typed into the
    // query instead of falling through to the keymap.
    let event = handle_key(
        &mut app,
        &mut keymap,
        key(KeyCode::Char('j'), KeyModifiers::NONE),
    );
    assert!(event.is_none());
    assert_eq!(app.search_input.as_deref(), Some("j"));

    let event = handle_key(
        &mut app,
        &mut keymap,
        key(KeyCode::Backspace, KeyModifiers::NONE),
    );
    assert!(event.is_none());
    assert_eq!(app.search_input.as_deref(), Some(""));

    let event = handle_key(
        &mut app,
        &mut keymap,
        key(KeyCode::Enter, KeyModifiers::NONE),
    );
    assert!(matches!(
        event,
        Some(AppEvent::Action(AppAction::ConfirmSearch))
    ));
}

#[test]
fn esc_cancels_search_input() {
    let mut app = base_app();
    app.focus = AppFocus::Diff;
    app.search_input = Some("term".into());
    let mut keymap = KeyMap::default();

    let event = handle_key(&mut app, &mut keymap, key(KeyCode::Esc, KeyModifiers::NONE));

    assert!(matches!(
        event,
        Some(AppEvent::Action(AppAction::CancelSearch))
    ));
}

#[test]
fn n_and_shift_n_navigate_an_active_search() {
    let mut app = base_app();
    app.focus = AppFocus::Diff;
    app.search_query = Some("term".into());
    let mut keymap = KeyMap::default();

    let event = handle_key(
        &mut app,
        &mut keymap,
        key(KeyCode::Char('n'), KeyModifiers::NONE),
    );
    assert!(matches!(
        event,
        Some(AppEvent::Action(AppAction::SearchNext))
    ));

    let event = handle_key(
        &mut app,
        &mut keymap,
        key(KeyCode::Char('N'), KeyModifiers::SHIFT),
    );
    assert!(matches!(
        event,
        Some(AppEvent::Action(AppAction::SearchPrevious))
    ));
}

#[test]
fn number_keys_focus_the_review_panels() {
    let mut app = base_app();
    app.focus = betterreview::app::AppFocus::Diff;
    let mut keymap = KeyMap::default();

    let event = handle_key(
        &mut app,
        &mut keymap,
        key(KeyCode::Char('2'), KeyModifiers::NONE),
    );
    assert!(matches!(
        event,
        Some(AppEvent::Action(AppAction::FocusFiles))
    ));

    let event = handle_key(
        &mut app,
        &mut keymap,
        key(KeyCode::Char('3'), KeyModifiers::NONE),
    );
    assert!(matches!(
        event,
        Some(AppEvent::Action(AppAction::FocusDiff))
    ));
}

#[test]
fn z_expands_when_cursor_is_on_a_gap_row() {
    let mut app = base_app();
    app.focus = AppFocus::Diff;
    app.display_rows = vec![DisplayRow::Gap {
        after_new_line: 4,
        hidden: 2,
    }];
    app.display_cursor = 0;
    let mut keymap = KeyMap::default();

    let event = handle_key(
        &mut app,
        &mut keymap,
        key(KeyCode::Char('z'), KeyModifiers::NONE),
    );

    assert!(matches!(
        event,
        Some(AppEvent::Action(AppAction::ExpandGap))
    ));
}

#[test]
fn z_still_toggles_fold_elsewhere() {
    let mut app = base_app();
    app.focus = AppFocus::Diff;
    app.display_rows = vec![DisplayRow::Diff { row: 0 }];
    app.display_cursor = 0;
    let mut keymap = KeyMap::default();

    let event = handle_key(
        &mut app,
        &mut keymap,
        key(KeyCode::Char('z'), KeyModifiers::NONE),
    );

    assert!(matches!(
        event,
        Some(AppEvent::Action(AppAction::ToggleFold))
    ));
}
