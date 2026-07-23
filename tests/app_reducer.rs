use std::collections::BTreeMap;

use betterreview::{
    app::{
        AppAction, AppEffect, AppEvent, AppState, EffectOutcome, EffectResult, RenderedFile,
        SubmissionModal, update,
    },
    diff::{ParsedFileDiff, RenderedDiff, RenderedRow, RowBinding},
    domain::{
        ChangeRequestKey, ChangedFile, CommitOid, FileStatus, PatchAvailability,
        ProviderCapabilities, ProviderKind, ProviderSnapshot, RepoPath, ReviewOutcome,
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
fn previous_file_wraps_and_resets_diff_position() {
    let mut state = app_with_reviewed_pattern([false; 4]);
    state.session.cursor_row = 2;
    state.session.scroll_row = 1;

    update(&mut state, AppEvent::Action(AppAction::PreviousFile));

    assert_eq!(state.active_file_index, 3);
    assert_eq!(state.session.cursor_row, 0);
    assert_eq!(state.session.scroll_row, 0);
    assert_eq!(
        state.session.active_file,
        Some(RepoPath("src/file_3.rs".into()))
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
