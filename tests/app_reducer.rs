use std::collections::BTreeMap;

use betterreview::{
    app::{
        AppAction, AppEffect, AppEvent, AppState, DisplayRow, EffectOutcome, EffectResult,
        QuitChoice, RenderedFile, SubmissionModal, update,
    },
    diff::{ParsedFileDiff, RenderedDiff, RenderedRow, RowBinding},
    domain::{
        ChangeRequestKey, ChangedFile, CommitOid, DiffPosition, DiffSelection, DiffSide,
        DraftComment, DraftId, FileStatus, PatchAvailability, ProviderCapabilities, ProviderKind,
        ProviderSnapshot, RepoPath, ReviewComment, ReviewOutcome, ReviewThread, SubmitMode,
        SubmitResult, Support, ThreadId,
    },
    providers::DraftBody,
    state::{ContentIdentity, FileProgress, ReviewSync, SESSION_SCHEMA_VERSION, SessionSnapshot},
};
use ratatui::text::Line;
use time::OffsetDateTime;

fn file(index: usize) -> ChangedFile {
    ChangedFile {
        path: RepoPath(format!("src/file_{index}.rs")),
        previous_path: None,
        status: FileStatus::Modified,
        additions: 1,
        deletions: 1,
        patch: PatchAvailability::Available("@@ -1 +1 @@\n-old\n+new\n".into()),
        base_blob: Some(format!("base-{index}")),
        head_blob: Some(format!("head-{index}")),
        remotely_reviewed: Some(false),
    }
}

fn app_with_reviewed_pattern(pattern: [bool; 4]) -> AppState {
    let files = (0..4).map(file).collect::<Vec<_>>();
    let progress = files
        .iter()
        .zip(pattern)
        .map(|(file, reviewed)| {
            (
                file.path.clone(),
                FileProgress {
                    identity: ContentIdentity {
                        path: file.path.clone(),
                        base_blob: file.base_blob.clone(),
                        head_blob: file.head_blob.clone(),
                    },
                    reviewed,
                    reviewed_hunks: Default::default(),
                    sync: ReviewSync::Synced,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let key = ChangeRequestKey {
        provider: ProviderKind::GitHub,
        host: "github.com".into(),
        repository: "owner/repo".into(),
        number: 10,
    };
    let provider = ProviderSnapshot {
        key: key.clone(),
        title: "Change".into(),
        author: "dev".into(),
        web_url: "https://github.com/owner/repo/pull/10".into(),
        base: CommitOid("base".into()),
        head: CommitOid("new-head".into()),
        files,
        threads: Vec::new(),
        drafts: Vec::new(),
        capabilities: ProviderCapabilities::all_supported(),
    };
    let session = SessionSnapshot {
        schema_version: SESSION_SCHEMA_VERSION,
        key,
        base: CommitOid("base".into()),
        head: CommitOid("new-head".into()),
        active_file: Some(RepoPath("src/file_0.rs".into())),
        cursor_row: 0,
        scroll_row: 0,
        files: progress,
        editor: None,
        pending_submit: None,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    };
    AppState::new(provider, session)
}

fn app_with_paths_all_unreviewed(paths: &[&str]) -> AppState {
    let files = paths
        .iter()
        .enumerate()
        .map(|(index, path)| ChangedFile {
            path: RepoPath((*path).to_owned()),
            previous_path: None,
            status: FileStatus::Modified,
            additions: 1,
            deletions: 1,
            patch: PatchAvailability::Available("@@ -1 +1 @@\n-old\n+new\n".into()),
            base_blob: Some(format!("base-{index}")),
            head_blob: Some(format!("head-{index}")),
            remotely_reviewed: Some(false),
        })
        .collect::<Vec<_>>();
    let progress = files
        .iter()
        .map(|file| {
            (
                file.path.clone(),
                FileProgress {
                    identity: ContentIdentity {
                        path: file.path.clone(),
                        base_blob: file.base_blob.clone(),
                        head_blob: file.head_blob.clone(),
                    },
                    reviewed: false,
                    reviewed_hunks: Default::default(),
                    sync: ReviewSync::Synced,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let key = ChangeRequestKey {
        provider: ProviderKind::GitHub,
        host: "github.com".into(),
        repository: "owner/repo".into(),
        number: 10,
    };
    let provider = ProviderSnapshot {
        key: key.clone(),
        title: "Change".into(),
        author: "dev".into(),
        web_url: "https://github.com/owner/repo/pull/10".into(),
        base: CommitOid("base".into()),
        head: CommitOid("new-head".into()),
        files,
        threads: Vec::new(),
        drafts: Vec::new(),
        capabilities: ProviderCapabilities::all_supported(),
    };
    let session = SessionSnapshot {
        schema_version: SESSION_SCHEMA_VERSION,
        key,
        base: CommitOid("base".into()),
        head: CommitOid("new-head".into()),
        active_file: Some(RepoPath(paths[0].to_owned())),
        cursor_row: 0,
        scroll_row: 0,
        files: progress,
        editor: None,
        pending_submit: None,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    };
    AppState::new(provider, session)
}

fn rendered_file(text: &str) -> RenderedFile {
    RenderedFile {
        parsed: ParsedFileDiff {
            path: RepoPath("src/file_0.rs".into()),
            head: CommitOid("new-head".into()),
            rows: Vec::new(),
            hunks: Vec::new(),
        },
        rendered: RenderedDiff {
            rows: vec![RenderedRow {
                text: Line::raw(text.to_owned()),
                binding: RowBinding {
                    row_index: 0,
                    left: None,
                    right: None,
                },
            }],
        },
    }
}

fn rendered_file_with_rows(count: usize) -> RenderedFile {
    RenderedFile {
        parsed: ParsedFileDiff {
            path: RepoPath("src/file_0.rs".into()),
            head: CommitOid("new-head".into()),
            rows: Vec::new(),
            hunks: Vec::new(),
        },
        rendered: RenderedDiff {
            rows: (0..count)
                .map(|index| RenderedRow {
                    text: Line::raw(format!("row-{index}")),
                    binding: RowBinding {
                        row_index: index,
                        left: None,
                        right: None,
                    },
                })
                .collect(),
        },
    }
}

#[test]
fn next_unreviewed_skips_reviewed_files_and_wraps() {
    let mut state = app_with_reviewed_pattern([true, false, true, false]);

    let effects = update(&mut state, AppEvent::Action(AppAction::NextUnreviewed));

    assert!(
        effects
            .iter()
            .any(|effect| matches!(effect.effect, AppEffect::RenderActiveFile { .. }))
    );
    assert_eq!(state.active_file_index, 1);
    update(&mut state, AppEvent::Action(AppAction::NextUnreviewed));
    assert_eq!(state.active_file_index, 3);
    update(&mut state, AppEvent::Action(AppAction::NextUnreviewed));
    assert_eq!(state.active_file_index, 1);
}

#[test]
fn unreviewed_navigation_skips_generated_files() {
    let mut state =
        app_with_paths_all_unreviewed(&["src/a.rs", "Cargo.lock", "src/b.rs", "vendor/lib.rs"]);

    let effects = update(&mut state, AppEvent::Action(AppAction::NextUnreviewed));

    assert!(
        effects
            .iter()
            .any(|effect| matches!(effect.effect, AppEffect::RenderActiveFile { .. }))
    );
    // Starting from src/a.rs (index 0), the generated Cargo.lock (index 1) is
    // skipped even though it is unreviewed.
    assert_eq!(state.active_file_index, 2);
    update(&mut state, AppEvent::Action(AppAction::NextUnreviewed));
    // vendor/lib.rs (index 3) is also generated and skipped, wrapping back to
    // src/a.rs (index 0).
    assert_eq!(state.active_file_index, 0);
}

#[test]
fn direct_file_navigation_stops_at_both_boundaries() {
    let mut state = app_with_reviewed_pattern([false; 4]);

    let effects = update(&mut state, AppEvent::Action(AppAction::PreviousFile));

    assert_eq!(state.active_file_index, 0);
    assert!(effects.is_empty());

    state.active_file_index = 3;
    state.session.active_file = Some(RepoPath("src/file_3.rs".into()));
    let effects = update(&mut state, AppEvent::Action(AppAction::NextFile));

    assert_eq!(state.active_file_index, 3);
    assert!(effects.is_empty());
}

#[test]
fn file_navigation_resets_diff_position() {
    let mut state = app_with_reviewed_pattern([false; 4]);
    state.active_file_index = 1;
    state.session.active_file = Some(RepoPath("src/file_1.rs".into()));
    state.session.cursor_row = 2;
    state.session.scroll_row = 1;

    update(&mut state, AppEvent::Action(AppAction::PreviousFile));

    assert_eq!(state.active_file_index, 0);
    assert_eq!(state.session.cursor_row, 0);
    assert_eq!(state.session.scroll_row, 0);
    assert_eq!(
        state.session.active_file,
        Some(RepoPath("src/file_0.rs".into()))
    );
}

#[test]
fn vertical_movement_changes_the_selected_file_when_files_have_focus() {
    let mut state = app_with_reviewed_pattern([false; 4]);
    state.focus = betterreview::app::AppFocus::Files;

    let effects = update(&mut state, AppEvent::Action(AppAction::MoveCursor(1)));

    assert_eq!(state.active_file_index, 1);
    assert_eq!(
        state.session.active_file,
        Some(RepoPath("src/file_1.rs".into()))
    );
    assert!(
        effects
            .iter()
            .any(|effect| matches!(effect.effect, AppEffect::RenderActiveFile { .. }))
    );
}

#[test]
fn reviewed_toggle_updates_local_state_and_schedules_sync_and_save() {
    let mut state = app_with_reviewed_pattern([false; 4]);

    let effects = update(&mut state, AppEvent::Action(AppAction::ToggleReviewed));

    let progress = &state.session.files[&RepoPath("src/file_0.rs".into())];
    assert!(progress.reviewed);
    assert_eq!(progress.sync, ReviewSync::Pending { desired: true });
    assert!(effects.iter().any(|effect| matches!(
        effect.effect,
        AppEffect::SetFileReviewed { reviewed: true, .. }
    )));
    assert!(
        effects
            .iter()
            .any(|effect| matches!(effect.effect, AppEffect::SaveSession { .. }))
    );
}

#[test]
fn ignores_effect_result_from_old_head_generation() {
    let mut state = app_with_reviewed_pattern([false; 4]);
    let rendered = rendered_file("old result");

    update(
        &mut state,
        AppEvent::EffectFinished(Box::new(EffectResult {
            id: 1,
            generation: Some(CommitOid("old-head".into())),
            outcome: EffectOutcome::Rendered(Ok(rendered)),
        })),
    );

    assert!(state.rendered_diff.is_none());
    assert!(
        state
            .notices
            .iter()
            .any(|notice| notice.contains("operação antiga ignorada"))
    );
}

#[test]
fn accepts_render_result_from_current_generation() {
    let mut state = app_with_reviewed_pattern([false; 4]);
    let rendered = rendered_file("current result");

    update(
        &mut state,
        AppEvent::EffectFinished(Box::new(EffectResult {
            id: 1,
            generation: Some(CommitOid("new-head".into())),
            outcome: EffectOutcome::Rendered(Ok(rendered)),
        })),
    );

    assert_eq!(state.rendered_diff.as_ref().unwrap().rows.len(), 1);
    assert_eq!(
        state.parsed_diff.as_ref().unwrap().head,
        CommitOid("new-head".into())
    );
    assert!(state.error_banner.is_none());
}

#[test]
fn resume_keeps_the_cursor_position_after_first_render() {
    let mut state = app_with_reviewed_pattern([false; 4]);
    state.session.cursor_row = 2;
    let rendered = rendered_file_with_rows(4);

    update(
        &mut state,
        AppEvent::EffectFinished(Box::new(EffectResult {
            id: 1,
            generation: Some(CommitOid("new-head".into())),
            outcome: EffectOutcome::Rendered(Ok(rendered)),
        })),
    );

    assert_eq!(
        state.display_cursor, 3,
        "resuming a session must land the cursor back on session.cursor_row, not on row 0"
    );

    update(&mut state, AppEvent::Action(AppAction::MoveCursor(1)));

    assert_eq!(
        state.display_cursor, 4,
        "the first move must step from the restored position, not from 0"
    );
    assert_eq!(state.session.cursor_row, 3);
}

#[test]
fn draft_creation_refreshes_display_rows() {
    let mut state = app_with_reviewed_pattern([false; 4]);
    let path = RepoPath("src/file_0.rs".into());
    let rendered = RenderedFile {
        parsed: ParsedFileDiff {
            path: path.clone(),
            head: CommitOid("new-head".into()),
            rows: Vec::new(),
            hunks: Vec::new(),
        },
        rendered: RenderedDiff {
            rows: vec![diff_row(
                0,
                Some(comment_pos(&path, DiffSide::Left, 1)),
                Some(comment_pos(&path, DiffSide::Right, 1)),
            )],
        },
    };
    update(
        &mut state,
        AppEvent::EffectFinished(Box::new(EffectResult {
            id: 1,
            generation: Some(CommitOid("new-head".into())),
            outcome: EffectOutcome::Rendered(Ok(rendered)),
        })),
    );
    assert!(
        state
            .display_rows
            .iter()
            .all(|row| !matches!(row, DisplayRow::Comment { .. })),
        "no drafts yet, so no comment rows should be cached"
    );

    let draft = DraftComment {
        id: DraftId("d1".into()),
        body: "please fix".into(),
        selection: Some(DiffSelection {
            start: comment_pos(&path, DiffSide::Right, 1),
            end: comment_pos(&path, DiffSide::Right, 1),
        }),
        thread_id: None,
    };
    update(
        &mut state,
        AppEvent::EffectFinished(Box::new(EffectResult {
            id: 2,
            generation: Some(CommitOid("new-head".into())),
            outcome: EffectOutcome::DraftCreated(Ok(draft)),
        })),
    );

    assert!(
        state
            .display_rows
            .iter()
            .any(|row| matches!(row, DisplayRow::Comment { .. })),
        "creating a draft must refresh the cached display rows"
    );
}

#[test]
fn failed_effect_becomes_a_visible_error() {
    let mut state = app_with_reviewed_pattern([false; 4]);

    update(
        &mut state,
        AppEvent::EffectFinished(Box::new(EffectResult {
            id: 4,
            generation: Some(CommitOid("new-head".into())),
            outcome: EffectOutcome::Rendered(Err("delta unavailable".into())),
        })),
    );

    assert_eq!(state.error_banner.as_deref(), Some("delta unavailable"));
}

#[test]
fn review_modal_opens_with_comment_outcome() {
    let mut state = app_with_reviewed_pattern([false; 4]);

    update(&mut state, AppEvent::Action(AppAction::OpenSubmit));

    assert_eq!(
        state.submission_modal,
        Some(SubmissionModal {
            summary: String::new(),
            outcome: ReviewOutcome::Comment,
        })
    );
}

#[test]
fn tick_saves_only_when_state_is_dirty() {
    let mut state = app_with_reviewed_pattern([false; 4]);
    assert!(update(&mut state, AppEvent::Tick).is_empty());
    state.dirty = true;

    let effects = update(&mut state, AppEvent::Tick);

    assert_eq!(effects.len(), 1);
    assert!(matches!(effects[0].effect, AppEffect::SaveSession { .. }));
    assert!(!state.dirty);
}

#[test]
fn submit_persists_intent_before_scheduling_remote_write() {
    let mut state = app_with_reviewed_pattern([false; 4]);

    let effects = update(
        &mut state,
        AppEvent::Action(AppAction::SubmitReview {
            summary: "ready".into(),
            outcome: ReviewOutcome::Approve,
        }),
    );

    let pending = state.session.pending_submit.as_ref().unwrap();
    assert_eq!(pending.summary, "ready");
    assert_eq!(pending.outcome, ReviewOutcome::Approve);
    assert_eq!(pending.mode, SubmitMode::Full);
    assert!(matches!(effects[0].effect, AppEffect::SaveSession { .. }));
    assert!(matches!(effects[1].effect, AppEffect::SubmitReview { .. }));
}

#[test]
fn quit_flow_can_cancel_or_discard_the_editor() {
    let mut state = app_with_reviewed_pattern([false; 4]);
    // Without a parked draft, q leaves immediately — no dialog.
    update(&mut state, AppEvent::Action(AppAction::Quit));
    assert!(!state.quit_dialog);
    assert!(state.quit_requested);

    // With a parked draft the dialog opens.
    state.quit_requested = false;
    let anchor = DiffPosition {
        path: RepoPath("src/file_0.rs".into()),
        side: DiffSide::Right,
        line: 1,
        hunk: 0,
    };
    state.session.editor = Some(betterreview::state::EditorSnapshot {
        lines: vec!["rascunho".into()],
        cursor_row: 0,
        grapheme_col: 0,
        original_head: CommitOid("new-head".into()),
        path: RepoPath("src/file_0.rs".into()),
        selection: DiffSelection {
            start: anchor.clone(),
            end: anchor,
        },
        stale: false,
    });
    update(&mut state, AppEvent::Action(AppAction::Quit));
    assert!(state.quit_dialog);

    update(
        &mut state,
        AppEvent::Action(AppAction::ConfirmQuit(QuitChoice::Cancel)),
    );
    assert!(!state.quit_dialog);
    assert!(!state.quit_requested);

    update(&mut state, AppEvent::Action(AppAction::Quit));
    update(
        &mut state,
        AppEvent::Action(AppAction::ConfirmQuit(QuitChoice::DiscardEditor)),
    );
    assert!(state.quit_requested);
    assert!(state.session.editor.is_none());
    assert!(state.editing_draft.is_none());
    assert!(state.replying_thread.is_none());
    assert!(!state.editor_open);
}

#[test]
fn keep_session_discards_mode_editors() {
    use betterreview::domain::{DiffPosition, DiffSelection, DiffSide};
    use betterreview::state::EditorSnapshot;
    let mut state = app_with_reviewed_pattern([false; 4]);
    let path = RepoPath("src/file_0.rs".into());
    let position = DiffPosition {
        path: path.clone(),
        side: DiffSide::Right,
        line: 1,
        hunk: 0,
    };
    state.session.editor = Some(EditorSnapshot {
        lines: vec!["being edited".into()],
        cursor_row: 0,
        grapheme_col: 0,
        original_head: CommitOid("new-head".into()),
        path,
        selection: DiffSelection {
            start: position.clone(),
            end: position,
        },
        stale: false,
    });
    state.editor_open = true;
    state.editing_draft = Some(DraftId("d1".into()));

    update(&mut state, AppEvent::Action(AppAction::Quit));
    update(
        &mut state,
        AppEvent::Action(AppAction::ConfirmQuit(QuitChoice::KeepSession)),
    );

    assert!(
        state.session.editor.is_none(),
        "a mode editor cannot be trusted after resume: it must be discarded, not persisted"
    );
    assert!(!state.editor_open);
    assert!(state.editing_draft.is_none());
    assert!(state.replying_thread.is_none());
    assert!(state.quit_requested);
    assert!(state.dirty);
}

#[test]
fn notices_expire_after_a_few_ticks() {
    use betterreview::domain::{DiffPosition, DiffSelection, DiffSide};
    use betterreview::state::EditorSnapshot;
    let mut state = app_with_reviewed_pattern([false; 4]);
    let path = RepoPath("src/file_0.rs".into());
    let position = DiffPosition {
        path: path.clone(),
        side: DiffSide::Right,
        line: 1,
        hunk: 0,
    };
    state.session.editor = Some(EditorSnapshot {
        lines: vec!["parked text".into()],
        cursor_row: 0,
        grapheme_col: 0,
        original_head: CommitOid("new-head".into()),
        path: path.clone(),
        selection: DiffSelection {
            start: position.clone(),
            end: position,
        },
        stale: false,
    });
    state.editor_open = false;
    state.editing_draft = None;
    state.replying_thread = None;

    let draft = DraftComment {
        id: DraftId("d1".into()),
        body: "old body".into(),
        selection: Some(DiffSelection {
            start: comment_pos(&path, DiffSide::Right, 1),
            end: comment_pos(&path, DiffSide::Right, 1),
        }),
        thread_id: None,
    };
    state.provider.drafts.push(draft.clone());

    update(
        &mut state,
        AppEvent::Action(AppAction::EditComment(draft.id.clone())),
    );

    assert_eq!(state.notice_ttl, 12, "a refusal must arm the notice ttl");

    for remaining in (0..12).rev() {
        update(&mut state, AppEvent::Tick);
        assert_eq!(state.notice_ttl, remaining);
    }
}

#[test]
fn submit_complete_schedules_a_snapshot_refresh() {
    let mut state = app_with_reviewed_pattern([false; 4]);
    state.session.pending_submit = Some(betterreview::state::PendingSubmit {
        summary: "ready".into(),
        outcome: ReviewOutcome::Approve,
        mode: SubmitMode::Full,
    });

    let effects = update(
        &mut state,
        AppEvent::EffectFinished(Box::new(EffectResult {
            id: 1,
            generation: Some(CommitOid("new-head".into())),
            outcome: EffectOutcome::ReviewSubmitted(Ok(SubmitResult::Complete)),
        })),
    );

    assert!(state.session.pending_submit.is_none());
    assert!(
        effects
            .iter()
            .any(|envelope| matches!(envelope.effect, AppEffect::RefreshSnapshot)),
        "a published draft must not remain interactive: schedule a refresh"
    );
}

fn added_row(path: &str, line: u32) -> betterreview::diff::DiffRow {
    use betterreview::domain::{DiffPosition, DiffSide};
    betterreview::diff::DiffRow {
        raw: "+new".into(),
        kind: betterreview::diff::DiffRowKind::Added,
        old_line: None,
        new_line: Some(line),
        left: None,
        right: Some(DiffPosition {
            path: RepoPath(path.into()),
            side: DiffSide::Right,
            line,
            hunk: 0,
        }),
    }
}

#[test]
fn stale_editor_is_replaced_when_opening_a_new_comment() {
    use betterreview::domain::{DiffPosition, DiffSelection, DiffSide};
    use betterreview::state::EditorSnapshot;
    let mut state = app_with_reviewed_pattern([false; 4]);
    let position = DiffPosition {
        path: RepoPath("src/file_0.rs".into()),
        side: DiffSide::Right,
        line: 1,
        hunk: 0,
    };
    state.session.editor = Some(EditorSnapshot {
        lines: vec!["texto antigo".into()],
        cursor_row: 0,
        grapheme_col: 0,
        original_head: CommitOid("old-head".into()),
        path: RepoPath("src/file_0.rs".into()),
        selection: DiffSelection {
            start: position.clone(),
            end: position,
        },
        stale: true,
    });
    state.parsed_diff = Some(ParsedFileDiff {
        path: RepoPath("src/file_0.rs".into()),
        head: CommitOid("new-head".into()),
        rows: vec![added_row("src/file_0.rs", 1)],
        hunks: Vec::new(),
    });
    state.session.cursor_row = 0;

    update(&mut state, AppEvent::Action(AppAction::OpenComment));

    let editor = state.session.editor.as_ref().expect("fresh editor");
    assert!(!editor.stale, "stale editor must be replaced, not reopened");
    assert_eq!(editor.lines, vec![String::new()]);
    assert!(state.editor_open);
}

fn app_with_two_directories() -> AppState {
    let mut state = app_with_reviewed_pattern([false; 4]);
    let paths = ["a/one.rs", "a/two.rs", "b/three.rs", "b/four.rs"];
    for (file, path) in state.provider.files.iter_mut().zip(paths) {
        file.path = RepoPath(path.into());
    }
    let progress = state
        .session
        .files
        .values()
        .cloned()
        .zip(paths)
        .map(|(mut progress, path)| {
            progress.identity.path = RepoPath(path.into());
            (RepoPath(path.into()), progress)
        })
        .collect();
    state.session.files = progress;
    state.session.active_file = Some(RepoPath("a/one.rs".into()));
    state.active_file_index = 0;
    state
}

fn comment_pos(path: &RepoPath, side: DiffSide, line: u32) -> DiffPosition {
    DiffPosition {
        path: path.clone(),
        side,
        line,
        hunk: 0,
    }
}

fn diff_row(
    row_index: usize,
    left: Option<DiffPosition>,
    right: Option<DiffPosition>,
) -> RenderedRow {
    RenderedRow {
        text: Line::raw(format!("row-{row_index}")),
        binding: RowBinding {
            row_index,
            left,
            right,
        },
    }
}

/// Three diff rows with a multi-line thread comment anchored to row 0
/// (right line 1). With comments shown the display rows are:
/// `[Diff{0}, Comment(start), Comment, Comment, Diff{1}, Diff{2}]`.
fn state_with_multiline_comment() -> AppState {
    let mut state = app_with_reviewed_pattern([false; 4]);
    let path = RepoPath("src/file_0.rs".into());
    state.rendered_diff = Some(RenderedDiff {
        rows: vec![
            diff_row(
                0,
                Some(comment_pos(&path, DiffSide::Left, 1)),
                Some(comment_pos(&path, DiffSide::Right, 1)),
            ),
            diff_row(1, Some(comment_pos(&path, DiffSide::Left, 2)), None),
            diff_row(2, None, Some(comment_pos(&path, DiffSide::Right, 3))),
        ],
    });
    state.provider.threads.push(ReviewThread {
        id: ThreadId("t1".into()),
        path,
        resolved: false,
        outdated: false,
        comments: vec![ReviewComment {
            id: "c1".into(),
            author: "alice".into(),
            body: "line1\nline2\nline3".into(),
            position: Some(comment_pos(
                &RepoPath("src/file_0.rs".into()),
                DiffSide::Right,
                1,
            )),
            pending: false,
        }],
    });
    betterreview::app::refresh_display_rows(&mut state);
    state
}

#[test]
fn cursor_walks_through_comment_blocks() {
    let mut state = state_with_multiline_comment();
    assert_eq!(state.display_cursor, 0);

    update(&mut state, AppEvent::Action(AppAction::MoveCursor(1)));
    assert_eq!(state.display_cursor, 1, "lands on the comment block start");
    assert_eq!(state.session.cursor_row, 0);

    update(&mut state, AppEvent::Action(AppAction::MoveCursor(1)));
    assert_eq!(
        state.display_cursor, 8,
        "body and footer rows are skipped, landing on Diff{{1}}"
    );
    assert_eq!(state.session.cursor_row, 1);

    update(&mut state, AppEvent::Action(AppAction::MoveCursor(1)));
    assert_eq!(state.display_cursor, 9);
    assert_eq!(state.session.cursor_row, 2);

    update(&mut state, AppEvent::Action(AppAction::MoveCursor(1)));
    assert_eq!(
        state.active_file_index, 1,
        "past the last row the review flows into the next file"
    );
    update(&mut state, AppEvent::Action(AppAction::PreviousFile));
    assert_eq!(state.active_file_index, 0);
}

#[test]
fn cursor_on_comment_keeps_session_row() {
    let mut state = state_with_multiline_comment();
    state.session.cursor_row = 5;
    state.display_cursor = 0;

    update(&mut state, AppEvent::Action(AppAction::MoveCursor(1)));

    assert_eq!(state.display_cursor, 1);
    assert_eq!(
        state.session.cursor_row, 5,
        "landing on a comment row must not touch session.cursor_row"
    );
}

#[test]
fn selection_refused_on_comment_rows() {
    let mut state = state_with_multiline_comment();
    state.display_cursor = 1;

    let effects = update(&mut state, AppEvent::Action(AppAction::ToggleSelection));

    assert!(effects.is_empty());
    assert!(state.selection_anchor.is_none());
    assert!(
        state
            .notices
            .iter()
            .any(|notice| notice.contains("mova para uma linha de código"))
    );
}

#[test]
fn selection_can_be_canceled_from_a_comment_row() {
    let mut state = state_with_multiline_comment();
    state.selection_anchor = Some(0);
    state.display_cursor = 1;

    let effects = update(&mut state, AppEvent::Action(AppAction::ToggleSelection));

    assert!(effects.is_empty());
    assert!(
        state.selection_anchor.is_none(),
        "an existing selection must be cancelable even from a comment row"
    );
}

#[test]
fn toggle_comments_resyncs_cursor() {
    let mut state = state_with_multiline_comment();
    state.display_cursor = 8;
    state.session.cursor_row = 1;

    update(&mut state, AppEvent::Action(AppAction::ToggleComments));

    assert!(state.comments_hidden);
    assert_eq!(
        state.display_cursor, 1,
        "hidden display rows are just the diff rows: Diff{{1}} is index 1"
    );

    update(&mut state, AppEvent::Action(AppAction::ToggleComments));

    assert!(!state.comments_hidden);
    assert_eq!(
        state.display_cursor, 8,
        "re-synced back to Diff{{1}} once comments are shown again"
    );
}

#[test]
fn folding_the_active_directory_makes_navigation_skip_its_files() {
    let mut state = app_with_two_directories();
    state.focus = betterreview::app::AppFocus::Files;

    update(&mut state, AppEvent::Action(AppAction::ToggleFold));

    assert!(state.collapsed_dirs.contains("a"));
    update(&mut state, AppEvent::Action(AppAction::NextFile));
    assert_eq!(
        state.provider.files[state.active_file_index].path.0,
        "b/three.rs"
    );

    update(&mut state, AppEvent::Action(AppAction::ToggleFold));
    assert!(state.collapsed_dirs.contains("b"));
    assert_eq!(state.collapsed_dirs.len(), 2);
}

#[test]
fn edit_opens_editor_with_draft_body_and_enter_updates() {
    let mut state = app_with_reviewed_pattern([false; 4]);
    let path = RepoPath("src/file_0.rs".into());
    let draft = DraftComment {
        id: DraftId("d1".into()),
        body: "old body\nsecond line".into(),
        selection: Some(DiffSelection {
            start: comment_pos(&path, DiffSide::Right, 1),
            end: comment_pos(&path, DiffSide::Right, 1),
        }),
        thread_id: None,
    };
    state.provider.drafts.push(draft.clone());

    update(
        &mut state,
        AppEvent::Action(AppAction::EditComment(draft.id.clone())),
    );

    assert_eq!(state.editing_draft, Some(draft.id.clone()));
    assert!(state.editor_open);
    let editor = state.session.editor.as_ref().expect("editor opened");
    assert_eq!(editor.lines, vec!["old body", "second line"]);

    let effects = update(
        &mut state,
        AppEvent::Action(AppAction::UpdateDraft {
            id: draft.id.clone(),
            body: DraftBody("new body".into()),
        }),
    );
    assert!(matches!(effects[0].effect, AppEffect::UpdateDraft { .. }));

    let updated = DraftComment {
        id: draft.id.clone(),
        body: "new body".into(),
        selection: draft.selection.clone(),
        thread_id: None,
    };
    update(
        &mut state,
        AppEvent::EffectFinished(Box::new(EffectResult {
            id: effects[0].id,
            generation: Some(CommitOid("new-head".into())),
            outcome: EffectOutcome::DraftUpdated(Ok(updated)),
        })),
    );

    assert!(state.editing_draft.is_none());
    assert!(!state.editor_open);
    assert!(state.session.editor.is_none());
    assert_eq!(
        state
            .provider
            .drafts
            .iter()
            .find(|d| d.id == draft.id)
            .map(|d| d.body.as_str()),
        Some("new body")
    );
}

#[test]
fn edit_refuses_when_a_fresh_draft_is_parked() {
    use betterreview::domain::{DiffPosition, DiffSelection, DiffSide};
    use betterreview::state::EditorSnapshot;
    let mut state = app_with_reviewed_pattern([false; 4]);
    let path = RepoPath("src/file_0.rs".into());
    let position = DiffPosition {
        path: path.clone(),
        side: DiffSide::Right,
        line: 1,
        hunk: 0,
    };
    let parked = EditorSnapshot {
        lines: vec!["parked text".into()],
        cursor_row: 0,
        grapheme_col: 0,
        original_head: CommitOid("new-head".into()),
        path: path.clone(),
        selection: DiffSelection {
            start: position.clone(),
            end: position,
        },
        stale: false,
    };
    state.session.editor = Some(parked.clone());
    state.editor_open = false;
    state.editing_draft = None;
    state.replying_thread = None;

    let draft = DraftComment {
        id: DraftId("d1".into()),
        body: "old body".into(),
        selection: Some(DiffSelection {
            start: comment_pos(&path, DiffSide::Right, 1),
            end: comment_pos(&path, DiffSide::Right, 1),
        }),
        thread_id: None,
    };
    state.provider.drafts.push(draft.clone());

    update(
        &mut state,
        AppEvent::Action(AppAction::EditComment(draft.id.clone())),
    );

    assert_eq!(
        state.session.editor.as_ref().map(|editor| &editor.lines),
        Some(&parked.lines),
        "parked draft body must survive untouched"
    );
    assert!(state.editing_draft.is_none());
    assert!(!state.editor_open);
    assert!(
        state
            .notices
            .iter()
            .any(|notice| notice.contains("comentário não salvo")),
        "expected a notice about the unsaved draft, got {:?}",
        state.notices
    );
}

#[test]
fn reply_refuses_when_a_fresh_draft_is_parked() {
    use betterreview::domain::{DiffPosition, DiffSelection, DiffSide};
    use betterreview::state::EditorSnapshot;
    let mut state = app_with_reviewed_pattern([false; 4]);
    let path = RepoPath("src/file_0.rs".into());
    let position = DiffPosition {
        path: path.clone(),
        side: DiffSide::Right,
        line: 1,
        hunk: 0,
    };
    let parked = EditorSnapshot {
        lines: vec!["parked text".into()],
        cursor_row: 0,
        grapheme_col: 0,
        original_head: CommitOid("new-head".into()),
        path: path.clone(),
        selection: DiffSelection {
            start: position.clone(),
            end: position,
        },
        stale: false,
    };
    state.session.editor = Some(parked.clone());
    state.editor_open = false;
    state.editing_draft = None;
    state.replying_thread = None;

    let thread = ReviewThread {
        id: ThreadId("t1".into()),
        path: path.clone(),
        resolved: false,
        outdated: false,
        comments: vec![ReviewComment {
            id: "c1".into(),
            author: "alice".into(),
            body: "please explain".into(),
            position: Some(comment_pos(&path, DiffSide::Right, 1)),
            pending: false,
        }],
    };
    state.provider.threads.push(thread.clone());

    update(
        &mut state,
        AppEvent::Action(AppAction::ReplyComment(thread.id.clone())),
    );

    assert_eq!(
        state.session.editor.as_ref().map(|editor| &editor.lines),
        Some(&parked.lines),
        "parked draft body must survive untouched"
    );
    assert!(state.replying_thread.is_none());
    assert!(!state.editor_open);
    assert!(
        state
            .notices
            .iter()
            .any(|notice| notice.contains("comentário não salvo")),
        "expected a notice about the unsaved draft, got {:?}",
        state.notices
    );
}

#[test]
fn delete_dialog_confirms_and_removes_the_draft() {
    let mut state = app_with_reviewed_pattern([false; 4]);
    let draft = DraftComment {
        id: DraftId("d1".into()),
        body: "please fix".into(),
        selection: None,
        thread_id: None,
    };
    state.provider.drafts.push(draft.clone());

    update(
        &mut state,
        AppEvent::Action(AppAction::DeleteComment(draft.id.clone())),
    );
    assert_eq!(state.delete_dialog, Some(draft.id.clone()));
    assert_eq!(state.delete_selected, 0);

    let effects = update(
        &mut state,
        AppEvent::Action(AppAction::ConfirmDeleteChoice(true)),
    );
    assert!(state.delete_dialog.is_none());
    assert!(matches!(effects[0].effect, AppEffect::DeleteDraft { .. }));

    update(
        &mut state,
        AppEvent::EffectFinished(Box::new(EffectResult {
            id: effects[0].id,
            generation: Some(CommitOid("new-head".into())),
            outcome: EffectOutcome::DraftDeleted {
                id: draft.id.clone(),
                result: Ok(()),
            },
        })),
    );

    assert!(!state.provider.drafts.iter().any(|d| d.id == draft.id));
}

#[test]
fn reply_on_thread_dispatches_reply() {
    let mut state = app_with_reviewed_pattern([false; 4]);
    let path = RepoPath("src/file_0.rs".into());
    let thread = ReviewThread {
        id: ThreadId("t1".into()),
        path: path.clone(),
        resolved: false,
        outdated: false,
        comments: vec![ReviewComment {
            id: "c1".into(),
            author: "alice".into(),
            body: "please explain".into(),
            position: Some(comment_pos(&path, DiffSide::Right, 1)),
            pending: false,
        }],
    };
    state.provider.threads.push(thread.clone());

    update(
        &mut state,
        AppEvent::Action(AppAction::ReplyComment(thread.id.clone())),
    );

    assert_eq!(state.replying_thread, Some(thread.id.clone()));
    assert!(state.editor_open);
    let editor = state.session.editor.as_ref().expect("editor opened");
    assert_eq!(editor.lines, vec![String::new()]);

    let effects = update(
        &mut state,
        AppEvent::Action(AppAction::Reply {
            thread: thread.id.clone(),
            body: DraftBody("thanks, fixed".into()),
        }),
    );
    assert!(matches!(effects[0].effect, AppEffect::Reply { .. }));

    let mut updated_thread = thread.clone();
    updated_thread.comments.push(ReviewComment {
        id: "c2".into(),
        author: "dev".into(),
        body: "thanks, fixed".into(),
        position: Some(comment_pos(&path, DiffSide::Right, 1)),
        pending: false,
    });
    update(
        &mut state,
        AppEvent::EffectFinished(Box::new(EffectResult {
            id: effects[0].id,
            generation: Some(CommitOid("new-head".into())),
            outcome: EffectOutcome::ThreadUpdated(Ok(updated_thread)),
        })),
    );

    assert!(state.replying_thread.is_none());
    assert!(!state.editor_open);
    assert!(state.session.editor.is_none());
}

#[test]
fn scheduling_a_draft_registers_a_pending_label() {
    let mut state = app_with_reviewed_pattern([false; 4]);
    let input = betterreview::providers::NewDraftComment {
        body: betterreview::providers::DraftBody("corpo".into()),
        selection: betterreview::domain::DiffSelection {
            start: betterreview::domain::DiffPosition {
                path: RepoPath("src/file_0.rs".into()),
                side: betterreview::domain::DiffSide::Right,
                line: 1,
                hunk: 0,
            },
            end: betterreview::domain::DiffPosition {
                path: RepoPath("src/file_0.rs".into()),
                side: betterreview::domain::DiffSide::Right,
                line: 1,
                hunk: 0,
            },
        },
        suggestion: None,
        operation_id: "op".into(),
    };

    let effects = update(&mut state, AppEvent::Action(AppAction::CreateDraft(input)));

    assert_eq!(effects.len(), 1);
    assert_eq!(
        state.pending_labels.get(&effects[0].id).copied(),
        Some("salvando comentário…")
    );
}

#[test]
fn finished_effect_clears_its_label_and_tick_spins_while_busy() {
    let mut state = app_with_reviewed_pattern([false; 4]);
    state.pending_labels.insert(7, "salvando comentário…");
    state.busy_operations.insert(7);

    let frame_before = state.spinner_frame;
    update(&mut state, AppEvent::Tick);
    assert!(
        state.spinner_frame != frame_before,
        "tick advances the spinner"
    );

    update(
        &mut state,
        AppEvent::EffectFinished(Box::new(EffectResult {
            id: 7,
            generation: None,
            outcome: EffectOutcome::Completed(Ok(())),
        })),
    );
    assert!(state.pending_labels.is_empty());
}

/// Two hunks of two rows each: `[HunkHeader, Context, HunkHeader, Context]`.
/// Comments are absent, so the display rows are
/// `[FileHeader, HunkHeader{0}, Diff{1}, Gap, HunkHeader{1}, Diff{3}, Gap]`.
fn state_with_two_hunks() -> AppState {
    let mut state = app_with_reviewed_pattern([false; 4]);
    let path = RepoPath("src/file_0.rs".into());
    state.provider.files[0].patch =
        PatchAvailability::Available("@@ -1 +1 @@\n context\n@@ -5 +5 @@\n context\n".into());
    state.refresh_hunk_totals();
    state.parsed_diff = Some(ParsedFileDiff {
        path: path.clone(),
        head: CommitOid("new-head".into()),
        rows: vec![
            betterreview::diff::DiffRow {
                raw: "@@ -1 +1 @@".into(),
                kind: betterreview::diff::DiffRowKind::HunkHeader,
                old_line: None,
                new_line: None,
                left: None,
                right: None,
            },
            betterreview::diff::DiffRow {
                raw: " context".into(),
                kind: betterreview::diff::DiffRowKind::Context,
                old_line: Some(1),
                new_line: Some(1),
                left: Some(comment_pos(&path, DiffSide::Left, 1)),
                right: Some(comment_pos(&path, DiffSide::Right, 1)),
            },
            betterreview::diff::DiffRow {
                raw: "@@ -5 +5 @@".into(),
                kind: betterreview::diff::DiffRowKind::HunkHeader,
                old_line: None,
                new_line: None,
                left: None,
                right: None,
            },
            betterreview::diff::DiffRow {
                raw: " context".into(),
                kind: betterreview::diff::DiffRowKind::Context,
                old_line: Some(5),
                new_line: Some(5),
                left: Some(comment_pos(&path, DiffSide::Left, 5)),
                right: Some(comment_pos(&path, DiffSide::Right, 5)),
            },
        ],
        hunks: vec![
            betterreview::diff::DiffHunk {
                id: 0,
                old_start: 1,
                old_count: 1,
                new_start: 1,
                new_count: 1,
                row_range: 1..2,
            },
            betterreview::diff::DiffHunk {
                id: 1,
                old_start: 5,
                old_count: 1,
                new_start: 5,
                new_count: 1,
                row_range: 3..4,
            },
        ],
    });
    state.rendered_diff = Some(RenderedDiff {
        rows: (0..4).map(|index| diff_row(index, None, None)).collect(),
    });
    betterreview::app::refresh_display_rows(&mut state);
    state
}

#[test]
fn each_hunk_gets_a_header_row_in_place_of_its_raw_at_line() {
    let state = state_with_two_hunks();

    assert_eq!(state.display_rows[1], DisplayRow::HunkHeader { hunk: 0 });
    assert_eq!(state.display_rows[2], DisplayRow::Diff { row: 1 });
    assert_eq!(state.display_rows[4], DisplayRow::HunkHeader { hunk: 1 });
    assert_eq!(state.display_rows[5], DisplayRow::Diff { row: 3 });
    assert!(
        matches!(state.display_rows[3], DisplayRow::Gap { .. }),
        "the hidden-lines gap sits above the header it precedes, not below it"
    );
}

#[test]
fn bracket_h_jumps_to_the_next_hunk_header() {
    let mut state = state_with_two_hunks();
    assert_eq!(state.display_cursor, 2, "starts on the first hunk's code");

    update(&mut state, AppEvent::Action(AppAction::NextHunk));

    assert_eq!(state.display_cursor, 4, "lands on the second hunk's header");
    assert_eq!(
        state.session.cursor_row, 1,
        "a header is not a code row; the code cursor stays put"
    );

    update(&mut state, AppEvent::Action(AppAction::PreviousHunk));

    assert_eq!(state.display_cursor, 1, "back to the first hunk's header");
}

#[test]
fn hunk_jump_clamps_at_the_last_hunk() {
    let mut state = state_with_two_hunks();
    state.display_cursor = 5;
    state.session.cursor_row = 3;

    update(&mut state, AppEvent::Action(AppAction::NextHunk));

    assert_eq!(
        state.display_cursor, 5,
        "clamped, no wrap past the last hunk"
    );
    assert!(
        state
            .notices
            .iter()
            .any(|notice| notice.contains("não há próximo hunk"))
    );
}

/// Two single-line comment threads anchored on rows 0 and 2. With comments
/// shown the display rows are:
/// `[Diff{0}, Comment(t1), Diff{1}, Diff{2}, Comment(t2)]`.
fn state_with_two_comment_blocks() -> AppState {
    let mut state = app_with_reviewed_pattern([false; 4]);
    let path = RepoPath("src/file_0.rs".into());
    state.rendered_diff = Some(RenderedDiff {
        rows: vec![
            diff_row(
                0,
                Some(comment_pos(&path, DiffSide::Left, 1)),
                Some(comment_pos(&path, DiffSide::Right, 1)),
            ),
            diff_row(
                1,
                Some(comment_pos(&path, DiffSide::Left, 2)),
                Some(comment_pos(&path, DiffSide::Right, 2)),
            ),
            diff_row(2, None, Some(comment_pos(&path, DiffSide::Right, 3))),
        ],
    });
    state.provider.threads.push(ReviewThread {
        id: ThreadId("t1".into()),
        path: path.clone(),
        resolved: false,
        outdated: false,
        comments: vec![ReviewComment {
            id: "c1".into(),
            author: "alice".into(),
            body: "first".into(),
            position: Some(comment_pos(&path, DiffSide::Right, 1)),
            pending: false,
        }],
    });
    state.provider.threads.push(ReviewThread {
        id: ThreadId("t2".into()),
        path: path.clone(),
        resolved: false,
        outdated: false,
        comments: vec![ReviewComment {
            id: "c2".into(),
            author: "bob".into(),
            body: "second".into(),
            position: Some(comment_pos(&path, DiffSide::Right, 3)),
            pending: false,
        }],
    });
    betterreview::app::refresh_display_rows(&mut state);
    state
}

#[test]
fn bracket_c_jumps_between_comment_blocks() {
    let mut state = state_with_two_comment_blocks();
    assert_eq!(state.display_cursor, 0);

    update(&mut state, AppEvent::Action(AppAction::NextComment));
    assert_eq!(state.display_cursor, 1, "lands on the first comment block");

    update(&mut state, AppEvent::Action(AppAction::NextComment));
    assert_eq!(state.display_cursor, 8, "lands on the second comment block");

    let effects = update(&mut state, AppEvent::Action(AppAction::NextComment));
    assert!(effects.is_empty());
    assert_eq!(
        state.display_cursor, 8,
        "clamped, no wrap past the last comment"
    );
    assert!(
        state
            .notices
            .iter()
            .any(|notice| notice.contains("não há próximo comentário"))
    );

    update(&mut state, AppEvent::Action(AppAction::PreviousComment));
    assert_eq!(state.display_cursor, 1, "back to the first comment block");
}

/// Four plain diff rows, two of which contain the word "needle": rows 1 and
/// 3. No comments, so display rows mirror the rendered rows one-to-one.
fn state_with_search_fixture() -> AppState {
    let mut state = app_with_reviewed_pattern([false; 4]);
    state.rendered_diff = Some(RenderedDiff {
        rows: [" alpha", " beta needle", " gamma", " delta NEEDLE"]
            .into_iter()
            .enumerate()
            .map(|(index, text)| RenderedRow {
                text: Line::raw(text),
                binding: RowBinding {
                    row_index: index,
                    left: None,
                    right: None,
                },
            })
            .collect(),
    });
    betterreview::app::refresh_display_rows(&mut state);
    state
}

#[test]
fn search_jumps_to_the_first_match() {
    let mut state = state_with_search_fixture();
    state.search_input = Some("needle".into());

    update(&mut state, AppEvent::Action(AppAction::ConfirmSearch));

    assert_eq!(state.search_query.as_deref(), Some("needle"));
    assert!(state.search_input.is_none());
    assert_eq!(
        state.display_cursor, 1,
        "lands on the first match at/after the cursor"
    );
    assert_eq!(state.session.cursor_row, 1);
}

#[test]
fn search_matching_only_a_comment_body_lands_on_its_block_header() {
    // "line2" only appears inside the comment's second body line; jumping to
    // it must still land on the block's Header row (the only navigation
    // stop inside a comment block), not on the Body row itself.
    let mut state = state_with_multiline_comment();
    state.search_input = Some("line2".into());

    update(&mut state, AppEvent::Action(AppAction::ConfirmSearch));

    assert_eq!(
        state.display_cursor, 1,
        "a match inside a comment body must land on the block's Header row"
    );
}

#[test]
fn n_wraps_around_matches() {
    let mut state = state_with_search_fixture();
    state.search_query = Some("needle".into());
    state.display_cursor = 1;
    state.session.cursor_row = 1;

    update(&mut state, AppEvent::Action(AppAction::SearchNext));
    assert_eq!(state.display_cursor, 3, "moves to the second match");

    update(&mut state, AppEvent::Action(AppAction::SearchNext));
    assert_eq!(
        state.display_cursor, 1,
        "wraps back around to the first match"
    );

    update(&mut state, AppEvent::Action(AppAction::SearchPrevious));
    assert_eq!(state.display_cursor, 3, "wraps backward to the last match");
}

#[test]
fn esc_clears_the_search() {
    let mut state = state_with_search_fixture();
    state.search_input = Some("needle".into());
    state.search_query = Some("needle".into());

    update(&mut state, AppEvent::Action(AppAction::CancelSearch));

    assert!(state.search_input.is_none());
    assert!(state.search_query.is_none());
}

#[test]
fn search_with_no_matches_leaves_a_notice() {
    let mut state = state_with_search_fixture();
    state.search_input = Some("missing".into());

    let effects = update(&mut state, AppEvent::Action(AppAction::ConfirmSearch));

    assert!(effects.is_empty());
    assert_eq!(
        state.search_query.as_deref(),
        Some("missing"),
        "the query is fixed even when nothing matches"
    );
    assert!(
        state
            .notices
            .iter()
            .any(|notice| notice.contains("sem resultados"))
    );
}

#[test]
fn updating_a_draft_keeps_its_anchor() {
    use betterreview::domain::{DiffPosition, DiffSelection, DiffSide, DraftComment, DraftId};
    let mut state = app_with_reviewed_pattern([false; 4]);
    let anchor = DiffPosition {
        path: RepoPath("src/file_0.rs".into()),
        side: DiffSide::Right,
        line: 1,
        hunk: 0,
    };
    state.provider.drafts.push(DraftComment {
        id: DraftId("d1".into()),
        body: "antes".into(),
        selection: Some(DiffSelection {
            start: anchor.clone(),
            end: anchor,
        }),
        thread_id: None,
    });

    // The provider's update response carries no position information.
    update(
        &mut state,
        AppEvent::EffectFinished(Box::new(EffectResult {
            id: 9,
            generation: None,
            outcome: EffectOutcome::DraftUpdated(Ok(DraftComment {
                id: DraftId("d1".into()),
                body: "depois".into(),
                selection: None,
                thread_id: None,
            })),
        })),
    );

    let draft = &state.provider.drafts[0];
    assert_eq!(draft.body, "depois");
    assert!(
        draft.selection.is_some(),
        "anchor must survive an update whose response omits it"
    );
}

#[test]
fn toggling_reviewed_from_the_diff_shows_a_notice() {
    let mut state = app_with_reviewed_pattern([false; 4]);
    state.focus = betterreview::app::AppFocus::Diff;

    update(&mut state, AppEvent::Action(AppAction::ToggleReviewed));
    assert!(
        state
            .notices
            .last()
            .is_some_and(|notice| notice.contains("revisado"))
    );

    update(&mut state, AppEvent::Action(AppAction::ToggleReviewed));
    assert!(
        state
            .notices
            .last()
            .is_some_and(|notice| notice.contains("desmarcado"))
    );
}

#[test]
fn j_at_the_last_row_advances_to_the_next_file() {
    let mut state = app_with_reviewed_pattern([false; 4]);
    state.focus = betterreview::app::AppFocus::Diff;
    state.rendered_diff = Some(RenderedDiff {
        rows: vec![RenderedRow {
            text: Line::raw("only"),
            binding: RowBinding {
                row_index: 0,
                left: None,
                right: None,
            },
        }],
    });
    betterreview::app::refresh_display_rows(&mut state);
    assert_eq!(state.active_file_index, 0);

    update(&mut state, AppEvent::Action(AppAction::MoveCursor(1)));

    assert_eq!(state.active_file_index, 1, "crossed into the next file");
    assert_eq!(state.display_cursor, 0);
}

#[test]
fn k_at_the_first_row_returns_to_the_previous_file_end() {
    let mut state = app_with_reviewed_pattern([false; 4]);
    state.focus = betterreview::app::AppFocus::Diff;
    update(&mut state, AppEvent::Action(AppAction::NextFile));
    assert_eq!(state.active_file_index, 1);
    state.rendered_diff = Some(RenderedDiff {
        rows: vec![RenderedRow {
            text: Line::raw("only"),
            binding: RowBinding {
                row_index: 0,
                left: None,
                right: None,
            },
        }],
    });
    betterreview::app::refresh_display_rows(&mut state);
    state.display_cursor = 0;

    update(&mut state, AppEvent::Action(AppAction::MoveCursor(-1)));
    assert_eq!(
        state.active_file_index, 0,
        "crossed back to the previous file"
    );

    // When the previous file's diff lands, the cursor sits at its END.
    let generation = Some(state.provider.head.clone());
    update(
        &mut state,
        AppEvent::EffectFinished(Box::new(EffectResult {
            id: 99,
            generation,
            outcome: EffectOutcome::Rendered(Ok(rendered_file("row"))),
        })),
    );
    assert_eq!(
        state.display_cursor,
        state.display_rows.len().saturating_sub(1),
        "positioned at the last row of the previous file"
    );
}

#[test]
fn file_header_and_metadata_rows_are_hidden_from_the_display() {
    use betterreview::diff::{DiffRow, DiffRowKind, ParsedFileDiff};
    let mut state = app_with_reviewed_pattern([false; 4]);
    let mk = |raw: &str, kind| DiffRow {
        raw: raw.into(),
        kind,
        old_line: None,
        new_line: None,
        left: None,
        right: None,
    };
    state.parsed_diff = Some(ParsedFileDiff {
        path: RepoPath("src/file_0.rs".into()),
        head: CommitOid("new-head".into()),
        rows: vec![
            mk("diff --git a/x b/x", DiffRowKind::Header),
            mk("index 1..2", DiffRowKind::Metadata),
            mk("@@ -1 +1 @@", DiffRowKind::HunkHeader),
            mk("+new", DiffRowKind::Added),
        ],
        hunks: Vec::new(),
    });
    state.rendered_diff = Some(RenderedDiff {
        rows: (0..4)
            .map(|index| RenderedRow {
                text: Line::raw(format!("row-{index}")),
                binding: RowBinding {
                    row_index: index,
                    left: None,
                    right: None,
                },
            })
            .collect(),
    });
    betterreview::app::refresh_display_rows(&mut state);

    let diff_rows: Vec<usize> = state
        .display_rows
        .iter()
        .filter_map(|row| match row {
            betterreview::app::DisplayRow::Diff { row } => Some(*row),
            _ => None,
        })
        .collect();
    assert_eq!(
        diff_rows,
        vec![3],
        "header, metadata and the raw @@ line are hidden; code stays"
    );
}

#[test]
fn cursor_on_a_now_hidden_row_snaps_to_the_nearest_following_diff_row() {
    use betterreview::diff::{DiffRow, DiffRowKind, ParsedFileDiff};
    let mut state = app_with_reviewed_pattern([false; 4]);
    let mk = |raw: &str, kind| DiffRow {
        raw: raw.into(),
        kind,
        old_line: None,
        new_line: None,
        left: None,
        right: None,
    };
    state.parsed_diff = Some(ParsedFileDiff {
        path: RepoPath("src/file_0.rs".into()),
        head: CommitOid("new-head".into()),
        rows: vec![
            mk("diff --git a/x b/x", DiffRowKind::Header),
            mk("index 1..2", DiffRowKind::Metadata),
            mk("@@ -1 +1 @@", DiffRowKind::HunkHeader),
            mk("+new", DiffRowKind::Added),
        ],
        hunks: Vec::new(),
    });
    state.rendered_diff = Some(RenderedDiff {
        rows: (0..4)
            .map(|index| RenderedRow {
                text: Line::raw(format!("row-{index}")),
                binding: RowBinding {
                    row_index: index,
                    left: None,
                    right: None,
                },
            })
            .collect(),
    });
    // The old session's cursor points at parsed row 0 (the `diff --git`
    // header), which is now filtered out of the display entirely.
    state.session.cursor_row = 0;
    state.dirty = false;

    betterreview::app::refresh_display_rows(&mut state);

    let landed = state.display_rows.get(state.display_cursor);
    assert!(
        matches!(landed, Some(betterreview::app::DisplayRow::Diff { row: 3 })),
        "must snap to the nearest following Diff row instead of falling back to index 0, got {landed:?}"
    );
    assert_eq!(
        state.session.cursor_row, 3,
        "session.cursor_row must be rewritten to the row actually landed on"
    );
    assert!(
        state.dirty,
        "snapping the cursor to a new row must mark the session dirty"
    );
}

#[test]
fn a_created_draft_appears_as_an_inline_card() {
    use betterreview::domain::{DiffPosition, DiffSelection, DiffSide, DraftComment, DraftId};
    let mut state = app_with_reviewed_pattern([false; 4]);
    let anchor = DiffPosition {
        path: RepoPath("src/file_0.rs".into()),
        side: DiffSide::Right,
        line: 1,
        hunk: 0,
    };
    state.rendered_diff = Some(RenderedDiff {
        rows: vec![RenderedRow {
            text: Line::raw("+new"),
            binding: RowBinding {
                row_index: 0,
                left: None,
                right: Some(anchor.clone()),
            },
        }],
    });
    betterreview::app::refresh_display_rows(&mut state);
    assert!(
        !state
            .display_rows
            .iter()
            .any(|row| matches!(row, betterreview::app::DisplayRow::Comment { .. }))
    );

    update(
        &mut state,
        AppEvent::EffectFinished(Box::new(EffectResult {
            id: 5,
            generation: None,
            outcome: EffectOutcome::DraftCreated(Ok(DraftComment {
                id: DraftId("novo".into()),
                body: "apareça".into(),
                selection: Some(DiffSelection {
                    start: anchor.clone(),
                    end: anchor,
                }),
                thread_id: None,
            })),
        })),
    );

    assert!(
        state
            .display_rows
            .iter()
            .any(|row| matches!(row, betterreview::app::DisplayRow::Comment { text, .. } if text == "apareça")),
        "the freshly created draft must appear anchored in the display"
    );
}

#[test]
fn submitting_an_unsupported_outcome_is_refused_with_a_notice() {
    let mut state = app_with_reviewed_pattern([false; 4]);
    state.provider.capabilities.approve = Support::Unsupported {
        reason: "o GitHub não permite no próprio pull request".into(),
    };
    state.submission_modal = Some(betterreview::app::SubmissionModal {
        summary: "ok".into(),
        outcome: ReviewOutcome::Approve,
    });

    let effects = update(
        &mut state,
        AppEvent::Action(AppAction::SubmitReview {
            summary: "ok".into(),
            outcome: ReviewOutcome::Approve,
        }),
    );

    assert!(effects.is_empty(), "no submit effect may be scheduled");
    assert!(state.session.pending_submit.is_none());
    assert!(
        state
            .notices
            .last()
            .is_some_and(|notice| notice.contains("não permite"))
    );
}

// --- Hidden context: gap/context display rows, expand flow, guards ---

fn gap_index(state: &AppState) -> usize {
    state
        .display_rows
        .iter()
        .position(|row| matches!(row, DisplayRow::Gap { .. }))
        .expect("a gap row must be present")
}

#[test]
fn gap_row_sits_between_the_two_hunks_with_the_right_hidden_count() {
    let state = state_with_two_hunks();

    assert_eq!(
        state.display_rows,
        vec![
            DisplayRow::FileHeader {
                path: "src/file_0.rs".into()
            },
            DisplayRow::HunkHeader { hunk: 0 },
            DisplayRow::Diff { row: 1 },
            DisplayRow::Gap {
                after_new_line: 1,
                hidden: 3
            },
            DisplayRow::HunkHeader { hunk: 1 },
            DisplayRow::Diff { row: 3 },
            DisplayRow::Gap {
                after_new_line: 5,
                hidden: 0
            },
        ],
        "lines 2-4 sit between the hunk covering line 1 and the one covering line 5"
    );
}

fn state_with_leading_gap() -> AppState {
    let mut state = app_with_reviewed_pattern([false; 4]);
    let path = RepoPath("src/file_0.rs".into());
    state.parsed_diff = Some(ParsedFileDiff {
        path: path.clone(),
        head: CommitOid("new-head".into()),
        rows: vec![
            betterreview::diff::DiffRow {
                raw: "@@ -5 +5 @@".into(),
                kind: betterreview::diff::DiffRowKind::HunkHeader,
                old_line: None,
                new_line: None,
                left: None,
                right: None,
            },
            betterreview::diff::DiffRow {
                raw: " context".into(),
                kind: betterreview::diff::DiffRowKind::Context,
                old_line: Some(5),
                new_line: Some(5),
                left: Some(comment_pos(&path, DiffSide::Left, 5)),
                right: Some(comment_pos(&path, DiffSide::Right, 5)),
            },
        ],
        hunks: Vec::new(),
    });
    state.rendered_diff = Some(RenderedDiff {
        rows: (0..2).map(|index| diff_row(index, None, None)).collect(),
    });
    betterreview::app::refresh_display_rows(&mut state);
    state
}

#[test]
fn leading_gap_appears_when_the_first_hunk_starts_past_line_one() {
    let state = state_with_leading_gap();

    assert_eq!(
        state.display_rows,
        vec![
            DisplayRow::FileHeader {
                path: "src/file_0.rs".into()
            },
            DisplayRow::Gap {
                after_new_line: 0,
                hidden: 4
            },
            DisplayRow::Diff { row: 1 },
            DisplayRow::Gap {
                after_new_line: 5,
                hidden: 0
            },
        ]
    );
}

fn state_with_trailing_gap() -> AppState {
    let mut state = app_with_reviewed_pattern([false; 4]);
    let path = RepoPath("src/file_0.rs".into());
    state.parsed_diff = Some(ParsedFileDiff {
        path: path.clone(),
        head: CommitOid("new-head".into()),
        rows: vec![betterreview::diff::DiffRow {
            raw: " context".into(),
            kind: betterreview::diff::DiffRowKind::Context,
            old_line: Some(1),
            new_line: Some(1),
            left: Some(comment_pos(&path, DiffSide::Left, 1)),
            right: Some(comment_pos(&path, DiffSide::Right, 1)),
        }],
        hunks: Vec::new(),
    });
    state.rendered_diff = Some(RenderedDiff {
        rows: (0..1).map(|index| diff_row(index, None, None)).collect(),
    });
    state
        .file_contexts
        .insert(path, vec!["one".into(), "two".into(), "three".into()]);
    betterreview::app::refresh_display_rows(&mut state);
    state
}

#[test]
fn trailing_gap_appears_when_the_cached_file_has_more_lines_than_the_diff() {
    let state = state_with_trailing_gap();

    assert_eq!(
        state.display_rows,
        vec![
            DisplayRow::FileHeader {
                path: "src/file_0.rs".into()
            },
            DisplayRow::Diff { row: 0 },
            DisplayRow::Gap {
                after_new_line: 1,
                hidden: 2
            },
        ]
    );
}

#[test]
fn expanded_gap_emits_context_rows_from_the_cached_file() {
    let mut state = state_with_two_hunks();
    let path = RepoPath("src/file_0.rs".into());
    state.file_contexts.insert(
        path,
        vec![
            "one".into(),
            "two".into(),
            "three".into(),
            "four".into(),
            "five".into(),
        ],
    );
    state.expanded_gaps.insert(1);
    betterreview::app::refresh_display_rows(&mut state);

    assert_eq!(
        state.display_rows,
        vec![
            DisplayRow::FileHeader {
                path: "src/file_0.rs".into()
            },
            DisplayRow::HunkHeader { hunk: 0 },
            DisplayRow::Diff { row: 1 },
            DisplayRow::Context {
                new_line: 2,
                text: "two".into()
            },
            DisplayRow::Context {
                new_line: 3,
                text: "three".into()
            },
            DisplayRow::Context {
                new_line: 4,
                text: "four".into()
            },
            DisplayRow::HunkHeader { hunk: 1 },
            DisplayRow::Diff { row: 3 },
        ]
    );
}

#[test]
fn z_on_an_uncached_gap_schedules_load_file_context_and_labels_it() {
    let mut state = state_with_two_hunks();
    state.display_cursor = gap_index(&state);

    let effects = update(&mut state, AppEvent::Action(AppAction::ExpandGap));

    assert_eq!(effects.len(), 1);
    match &effects[0].effect {
        AppEffect::LoadFileContext { path, revision } => {
            assert_eq!(path, &RepoPath("src/file_0.rs".into()));
            assert_eq!(revision, &CommitOid("new-head".into()));
        }
        other => panic!("expected LoadFileContext, got {other:?}"),
    }
    assert_eq!(state.pending_gap, Some(1));
    assert_eq!(
        state.pending_labels.get(&effects[0].id),
        Some(&"carregando contexto…")
    );
}

#[test]
fn z_on_a_cached_gap_toggles_expansion_without_scheduling_an_effect() {
    let mut state = state_with_two_hunks();
    let path = RepoPath("src/file_0.rs".into());
    state
        .file_contexts
        .insert(path, vec!["a".into(), "b".into(), "c".into(), "d".into()]);
    state.display_cursor = gap_index(&state);

    let effects = update(&mut state, AppEvent::Action(AppAction::ExpandGap));

    assert!(effects.is_empty());
    assert!(state.expanded_gaps.contains(&1));
    assert!(
        state
            .display_rows
            .iter()
            .any(|row| matches!(row, DisplayRow::Context { .. })),
        "the gap must have expanded into context rows"
    );
}

#[test]
fn file_context_loaded_expands_the_pending_gap() {
    let mut state = state_with_two_hunks();
    state.display_cursor = gap_index(&state);
    state.pending_gap = Some(1);

    update(
        &mut state,
        AppEvent::EffectFinished(Box::new(EffectResult {
            id: 7,
            generation: Some(CommitOid("new-head".into())),
            outcome: EffectOutcome::FileContextLoaded {
                path: RepoPath("src/file_0.rs".into()),
                result: Ok("one\ntwo\nthree\nfour\nfive".into()),
            },
        })),
    );

    assert_eq!(state.pending_gap, None);
    assert!(state.expanded_gaps.contains(&1));
    assert_eq!(
        state.file_contexts.get(&RepoPath("src/file_0.rs".into())),
        Some(&vec![
            "one".to_owned(),
            "two".to_owned(),
            "three".to_owned(),
            "four".to_owned(),
            "five".to_owned(),
        ])
    );
    assert!(
        state
            .display_rows
            .iter()
            .any(|row| matches!(row, DisplayRow::Context { .. })),
        "the newly loaded file must expand the gap that requested it"
    );
}

#[test]
fn file_context_load_failure_shows_an_error_banner() {
    let mut state = state_with_two_hunks();
    state.pending_gap = Some(1);

    update(
        &mut state,
        AppEvent::EffectFinished(Box::new(EffectResult {
            id: 7,
            generation: Some(CommitOid("new-head".into())),
            outcome: EffectOutcome::FileContextLoaded {
                path: RepoPath("src/file_0.rs".into()),
                result: Err("network error".into()),
            },
        })),
    );

    assert_eq!(state.pending_gap, None);
    assert_eq!(state.error_banner.as_deref(), Some("network error"));
}

#[test]
fn activating_another_file_clears_expanded_gaps_and_the_pending_one() {
    let mut state = state_with_two_hunks();
    state.expanded_gaps.insert(1);
    state.pending_gap = Some(9);

    update(&mut state, AppEvent::Action(AppAction::NextFile));

    assert!(state.expanded_gaps.is_empty());
    assert_eq!(state.pending_gap, None);
}

#[test]
fn selection_is_refused_on_a_gap_row() {
    let mut state = state_with_two_hunks();
    state.display_cursor = gap_index(&state);

    let effects = update(&mut state, AppEvent::Action(AppAction::ToggleSelection));

    assert!(effects.is_empty());
    assert!(state.selection_anchor.is_none());
    assert!(
        state
            .notices
            .iter()
            .any(|notice| notice == "mova para uma linha de código")
    );
}

#[test]
fn opening_a_comment_is_refused_on_a_gap_row() {
    let mut state = state_with_two_hunks();
    state.display_cursor = gap_index(&state);

    update(&mut state, AppEvent::Action(AppAction::OpenComment));

    assert!(state.session.editor.is_none());
    assert!(
        state
            .notices
            .iter()
            .any(|notice| notice == "mova para uma linha de código")
    );
}

#[test]
fn folded_directories_stay_reachable_for_unfolding() {
    let mut state = app_with_two_directories();
    state.focus = betterreview::app::AppFocus::Files;

    // Fold both directories.
    update(&mut state, AppEvent::Action(AppAction::ToggleFold));
    update(&mut state, AppEvent::Action(AppAction::NextFile));
    update(&mut state, AppEvent::Action(AppAction::ToggleFold));
    assert_eq!(state.collapsed_dirs.len(), 2);

    // With everything folded, j/k still walks between the folded
    // directories (their representative first file)...
    update(&mut state, AppEvent::Action(AppAction::PreviousFile));
    assert_eq!(
        state.provider.files[state.active_file_index].path.0,
        "a/one.rs"
    );
    update(&mut state, AppEvent::Action(AppAction::NextFile));
    assert_eq!(
        state.provider.files[state.active_file_index].path.0,
        "b/three.rs"
    );

    // ...so z can unfold the directory under the highlight again.
    update(&mut state, AppEvent::Action(AppAction::ToggleFold));
    assert!(!state.collapsed_dirs.contains("b"));
}

#[test]
fn activate_file_selects_the_clicked_file() {
    let mut state = app_with_two_directories();
    assert_eq!(state.active_file_index, 0);

    update(&mut state, AppEvent::Action(AppAction::ActivateFile(2)));

    assert_eq!(state.active_file_index, 2);
    assert_eq!(
        state.session.active_file,
        Some(RepoPath("b/three.rs".into()))
    );
}

#[test]
fn activate_file_ignores_a_folded_non_representative_index() {
    let mut state = app_with_two_directories();
    state.collapsed_dirs.insert("b".into());

    // "b/four.rs" (index 3) is hidden behind the folded "b" directory; only
    // its representative, "b/three.rs" (index 2), is reachable.
    update(&mut state, AppEvent::Action(AppAction::ActivateFile(3)));

    assert_eq!(state.active_file_index, 0);
}

#[test]
fn toggle_fold_dir_folds_and_unfolds_an_arbitrary_directory() {
    let mut state = app_with_two_directories();

    update(
        &mut state,
        AppEvent::Action(AppAction::ToggleFoldDir("b".into())),
    );
    assert!(state.collapsed_dirs.contains("b"));

    update(
        &mut state,
        AppEvent::Action(AppAction::ToggleFoldDir("b".into())),
    );
    assert!(!state.collapsed_dirs.contains("b"));
}

#[test]
fn jump_to_display_row_lands_directly_on_a_diff_row() {
    let mut state = state_with_multiline_comment();

    update(&mut state, AppEvent::Action(AppAction::JumpToDisplayRow(8)));

    assert_eq!(state.display_cursor, 8);
    assert_eq!(state.session.cursor_row, 1);
}

#[test]
fn jump_to_display_row_snaps_a_comment_body_click_to_its_header() {
    let mut state = state_with_multiline_comment();

    // Row 2 is a Body row inside the comment block that starts at row 1.
    update(&mut state, AppEvent::Action(AppAction::JumpToDisplayRow(2)));

    assert_eq!(state.display_cursor, 1);
}

#[test]
fn jump_to_display_row_clamps_past_the_end_of_the_display_rows() {
    let mut state = state_with_multiline_comment();
    let last = state.display_rows.len() - 1;

    update(
        &mut state,
        AppEvent::Action(AppAction::JumpToDisplayRow(999)),
    );

    assert_eq!(state.display_cursor, last);
}

// --- Hunk-level review progress ---

fn reviewed_hunks(state: &AppState) -> Vec<u32> {
    state.session.files[&RepoPath("src/file_0.rs".into())]
        .reviewed_hunks
        .iter()
        .copied()
        .collect()
}

fn file_reviewed(state: &AppState) -> bool {
    state.session.files[&RepoPath("src/file_0.rs".into())].reviewed
}

fn set_file_reviewed_effects(effects: &[betterreview::app::EffectEnvelope]) -> Vec<bool> {
    effects
        .iter()
        .filter_map(|envelope| match &envelope.effect {
            AppEffect::SetFileReviewed { reviewed, .. } => Some(*reviewed),
            _ => None,
        })
        .collect()
}

#[test]
fn shift_m_marks_the_hunk_under_the_cursor_and_saves_the_session() {
    let mut state = state_with_two_hunks();
    assert_eq!(
        state.display_cursor, 2,
        "cursor sits on the first hunk's code"
    );

    let effects = update(&mut state, AppEvent::Action(AppAction::ToggleHunkReviewed));

    assert_eq!(reviewed_hunks(&state), vec![0]);
    assert!(!file_reviewed(&state), "one hunk of two is not the file");
    assert!(
        effects
            .iter()
            .any(|envelope| matches!(envelope.effect, AppEffect::SaveSession { .. }))
    );
}

#[test]
fn shift_m_on_a_header_row_marks_that_hunk() {
    let mut state = state_with_two_hunks();
    state.display_cursor = 4;

    update(&mut state, AppEvent::Action(AppAction::ToggleHunkReviewed));

    assert_eq!(reviewed_hunks(&state), vec![1]);
}

#[test]
fn shift_m_twice_unmarks_the_hunk() {
    let mut state = state_with_two_hunks();

    update(&mut state, AppEvent::Action(AppAction::ToggleHunkReviewed));
    update(&mut state, AppEvent::Action(AppAction::ToggleHunkReviewed));

    assert!(reviewed_hunks(&state).is_empty());
}

#[test]
fn marking_every_hunk_marks_the_file_and_syncs_it() {
    let mut state = state_with_two_hunks();

    update(&mut state, AppEvent::Action(AppAction::ToggleHunkReviewed));
    state.display_cursor = 4;
    let effects = update(&mut state, AppEvent::Action(AppAction::ToggleHunkReviewed));

    assert_eq!(reviewed_hunks(&state), vec![0, 1]);
    assert!(file_reviewed(&state));
    assert_eq!(set_file_reviewed_effects(&effects), vec![true]);
    assert_eq!(
        state.session.files[&RepoPath("src/file_0.rs".into())].sync,
        ReviewSync::Pending { desired: true }
    );
}

#[test]
fn unmarking_a_hunk_of_a_fully_reviewed_file_unmarks_the_file() {
    let mut state = state_with_two_hunks();
    update(&mut state, AppEvent::Action(AppAction::ToggleHunkReviewed));
    state.display_cursor = 4;
    update(&mut state, AppEvent::Action(AppAction::ToggleHunkReviewed));

    let effects = update(&mut state, AppEvent::Action(AppAction::ToggleHunkReviewed));

    assert_eq!(reviewed_hunks(&state), vec![0]);
    assert!(!file_reviewed(&state));
    assert_eq!(set_file_reviewed_effects(&effects), vec![false]);
}

#[test]
fn m_fills_every_hunk_and_clears_them_again() {
    let mut state = state_with_two_hunks();

    update(&mut state, AppEvent::Action(AppAction::ToggleReviewed));
    assert_eq!(reviewed_hunks(&state), vec![0, 1]);
    assert!(file_reviewed(&state));

    update(&mut state, AppEvent::Action(AppAction::ToggleReviewed));
    assert!(reviewed_hunks(&state).is_empty());
    assert!(!file_reviewed(&state));
}

#[test]
fn shift_m_on_a_file_without_hunks_leaves_a_notice() {
    let mut state = app_with_reviewed_pattern([false; 4]);
    state.provider.files[0].patch = PatchAvailability::Binary;
    state.refresh_hunk_totals();

    let effects = update(&mut state, AppEvent::Action(AppAction::ToggleHunkReviewed));

    assert!(effects.is_empty());
    assert!(reviewed_hunks(&state).is_empty());
    assert!(state.notices.iter().any(|notice| notice.contains("hunk")));
}
