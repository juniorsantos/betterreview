use std::collections::BTreeMap;

use betterreview::{
    app::{
        AppAction, AppEffect, AppEvent, AppState, EffectOutcome, EffectResult, QuitChoice,
        RenderedFile, SubmissionModal, update,
    },
    diff::{ParsedFileDiff, RenderedDiff, RenderedRow, RowBinding},
    domain::{
        ChangeRequestKey, ChangedFile, CommitOid, FileStatus, PatchAvailability,
        ProviderCapabilities, ProviderKind, ProviderSnapshot, RepoPath, ReviewOutcome, SubmitMode,
    },
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
    assert!(state.notices.iter().any(|notice| notice.contains("stale")));
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
            selected_field: 0,
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

#[test]
fn folding_the_active_directory_makes_navigation_skip_its_files() {
    let mut state = app_with_two_directories();

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
