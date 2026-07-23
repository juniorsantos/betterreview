use crate::{
    domain::{ProviderKind, ReviewOutcome, SubmitMode, SubmitRequest, SubmitResult, Support},
    state::{PendingSubmit, ReviewSync},
};

use super::{
    AppAction, AppEffect, AppEvent, AppFocus, AppState, EffectEnvelope, EffectOutcome,
    EffectResult, QuitChoice, SubmissionModal,
};

pub fn update(state: &mut AppState, event: AppEvent) -> Vec<EffectEnvelope> {
    match event {
        AppEvent::Action(action) => action_update(state, action),
        AppEvent::Terminal(_) => Vec::new(),
        AppEvent::Tick => {
            if state.dirty {
                state.dirty = false;
                vec![envelope(
                    state,
                    None,
                    AppEffect::SaveSession {
                        snapshot: Box::new(state.session.clone()),
                    },
                )]
            } else {
                Vec::new()
            }
        }
        AppEvent::EffectFinished(result) => finish_effect(state, *result),
    }
}

fn action_update(state: &mut AppState, action: AppAction) -> Vec<EffectEnvelope> {
    match action {
        AppAction::FocusNext => {
            state.focus = match state.focus {
                AppFocus::Files => AppFocus::Diff,
                AppFocus::Diff => AppFocus::Threads,
                AppFocus::Threads => AppFocus::Files,
            };
            Vec::new()
        }
        AppAction::FocusPrevious => {
            state.focus = match state.focus {
                AppFocus::Files => AppFocus::Threads,
                AppFocus::Diff => AppFocus::Files,
                AppFocus::Threads => AppFocus::Diff,
            };
            Vec::new()
        }
        AppAction::NextFile => navigate_by(state, 1),
        AppAction::PreviousFile => navigate_by(state, -1),
        AppAction::NextUnreviewed => navigate_unreviewed(state, 1),
        AppAction::PreviousUnreviewed => navigate_unreviewed(state, -1),
        AppAction::MoveCursor(delta) => {
            let row_count = state
                .rendered_diff
                .as_ref()
                .map(|diff| diff.rows.len())
                .unwrap_or(0);
            state.session.cursor_row = move_index(state.session.cursor_row, delta, row_count);
            state.dirty = true;
            Vec::new()
        }
        AppAction::ToggleReviewed => toggle_reviewed(state),
        AppAction::ToggleSelection => {
            state.selection_anchor = match state.selection_anchor {
                Some(_) => None,
                None => Some(state.session.cursor_row),
            };
            state.dirty = true;
            Vec::new()
        }
        AppAction::OpenComment | AppAction::OpenSuggestion => {
            if state.selection_anchor.is_none() {
                state.selection_anchor = Some(state.session.cursor_row);
            }
            Vec::new()
        }
        AppAction::OpenThreads => {
            state.thread_panel_open = true;
            state.focus = AppFocus::Threads;
            Vec::new()
        }
        AppAction::OpenSubmit => {
            state.submission_modal = Some(SubmissionModal {
                summary: String::new(),
                outcome: ReviewOutcome::Comment,
                selected_field: 0,
            });
            Vec::new()
        }
        AppAction::CreateDraft(input) => vec![envelope(
            state,
            Some(state.provider.head.clone()),
            AppEffect::CreateDraft { input },
        )],
        AppAction::UpdateDraft { id, body } => vec![envelope(
            state,
            Some(state.provider.head.clone()),
            AppEffect::UpdateDraft { id, body },
        )],
        AppAction::DeleteDraft(id) => vec![envelope(
            state,
            Some(state.provider.head.clone()),
            AppEffect::DeleteDraft { id },
        )],
        AppAction::Reply { thread, body } => vec![envelope(
            state,
            Some(state.provider.head.clone()),
            AppEffect::Reply { thread, body },
        )],
        AppAction::ResolveThread { thread, resolved } => vec![envelope(
            state,
            Some(state.provider.head.clone()),
            AppEffect::ResolveThread { thread, resolved },
        )],
        AppAction::SubmitReview { summary, outcome } => {
            let request = SubmitRequest {
                expected_head: state.provider.head.clone(),
                summary: summary.clone(),
                outcome,
                mode: SubmitMode::Full,
            };
            state.session.pending_submit = Some(PendingSubmit {
                summary,
                outcome,
                mode: SubmitMode::Full,
            });
            state.submission_modal = None;
            vec![
                envelope(
                    state,
                    None,
                    AppEffect::SaveSession {
                        snapshot: Box::new(state.session.clone()),
                    },
                ),
                envelope(
                    state,
                    Some(state.provider.head.clone()),
                    AppEffect::SubmitReview { request },
                ),
            ]
        }
        AppAction::DiscardReview => vec![envelope(
            state,
            Some(state.provider.head.clone()),
            AppEffect::DiscardReview,
        )],
        AppAction::CancelSubmit => {
            state.submission_modal = None;
            Vec::new()
        }
        AppAction::Refresh => vec![envelope(
            state,
            Some(state.provider.head.clone()),
            AppEffect::RefreshSnapshot,
        )],
        AppAction::Quit => {
            state.quit_dialog = true;
            Vec::new()
        }
        AppAction::ConfirmQuit(choice) => {
            match choice {
                QuitChoice::KeepSession => state.quit_requested = true,
                QuitChoice::DiscardEditor => {
                    state.session.editor = None;
                    state.dirty = true;
                    state.quit_requested = true;
                }
                QuitChoice::Cancel => {}
            }
            state.quit_dialog = false;
            Vec::new()
        }
        AppAction::ToggleHelp => {
            state.help_visible = !state.help_visible;
            Vec::new()
        }
    }
}

fn navigate_by(state: &mut AppState, delta: i32) -> Vec<EffectEnvelope> {
    let count = state.provider.files.len();
    if count == 0 {
        return Vec::new();
    }
    let index = move_index_wrapped(state.active_file_index, delta, count);
    activate_file(state, index)
}

fn navigate_unreviewed(state: &mut AppState, delta: i32) -> Vec<EffectEnvelope> {
    let count = state.provider.files.len();
    if count == 0 {
        return Vec::new();
    }
    let mut index = state.active_file_index;
    for _ in 0..count {
        index = move_index_wrapped(index, delta, count);
        let path = &state.provider.files[index].path;
        if !state
            .session
            .files
            .get(path)
            .is_some_and(|progress| progress.reviewed)
        {
            return activate_file(state, index);
        }
    }
    Vec::new()
}

fn activate_file(state: &mut AppState, index: usize) -> Vec<EffectEnvelope> {
    state.active_file_index = index;
    state.session.active_file = state
        .provider
        .files
        .get(index)
        .map(|file| file.path.clone());
    state.session.cursor_row = 0;
    state.session.scroll_row = 0;
    state.parsed_diff = None;
    state.rendered_diff = None;
    state.selection_anchor = None;
    state.dirty = false;
    let Some(file) = state.provider.files.get(index).cloned() else {
        return Vec::new();
    };
    let generation = Some(state.provider.head.clone());
    vec![
        envelope(
            state,
            generation,
            AppEffect::RenderActiveFile {
                file,
                width: state.terminal_width,
            },
        ),
        envelope(
            state,
            None,
            AppEffect::SaveSession {
                snapshot: Box::new(state.session.clone()),
            },
        ),
    ]
}

fn toggle_reviewed(state: &mut AppState) -> Vec<EffectEnvelope> {
    let Some(path) = state
        .provider
        .files
        .get(state.active_file_index)
        .map(|file| file.path.clone())
    else {
        return Vec::new();
    };
    let Some(progress) = state.session.files.get_mut(&path) else {
        return Vec::new();
    };
    let reviewed = !progress.reviewed;
    progress.reviewed = reviewed;
    let remote_supported = matches!(
        state.provider.capabilities.mark_file_reviewed,
        Support::Supported
    );
    progress.sync = match (state.provider.key.provider, remote_supported) {
        (ProviderKind::GitHub, true) => ReviewSync::Pending { desired: reviewed },
        _ => ReviewSync::LocalOnly,
    };
    let mut effects = Vec::new();
    if remote_supported {
        let generation = Some(state.provider.head.clone());
        effects.push(envelope(
            state,
            generation,
            AppEffect::SetFileReviewed {
                path: path.clone(),
                reviewed,
            },
        ));
    }
    effects.push(envelope(
        state,
        None,
        AppEffect::SaveSession {
            snapshot: Box::new(state.session.clone()),
        },
    ));
    effects
}

fn finish_effect(state: &mut AppState, result: EffectResult) -> Vec<EffectEnvelope> {
    state.busy_operations.remove(&result.id);
    if result
        .generation
        .as_ref()
        .is_some_and(|generation| generation != &state.provider.head)
    {
        state
            .notices
            .push("ignored stale operation from an older head".into());
        return Vec::new();
    }
    match result.outcome {
        EffectOutcome::Rendered(result) => match result {
            Ok(result) => {
                state.parsed_diff = Some(result.parsed);
                state.rendered_diff = Some(result.rendered);
                state.error_banner = None;
            }
            Err(message) => state.error_banner = Some(message),
        },
        EffectOutcome::Saved(result) | EffectOutcome::Completed(result) => set_error(state, result),
        EffectOutcome::FileReviewed {
            path,
            reviewed,
            result,
        } => match result {
            Ok(()) => {
                if let Some(progress) = state.session.files.get_mut(&path) {
                    progress.reviewed = reviewed;
                    progress.sync = ReviewSync::Synced;
                }
            }
            Err(message) => {
                if let Some(progress) = state.session.files.get_mut(&path) {
                    progress.sync = ReviewSync::Failed {
                        desired: reviewed,
                        message: message.clone(),
                    };
                }
                state.error_banner = Some(message);
            }
        },
        EffectOutcome::SnapshotRefreshed(result) => match *result {
            Ok(snapshot) => state.provider = snapshot,
            Err(message) => state.error_banner = Some(message),
        },
        EffectOutcome::DraftCreated(result) | EffectOutcome::DraftUpdated(result) => match result {
            Ok(draft) => {
                state.provider.drafts.retain(|item| item.id != draft.id);
                state.provider.drafts.push(draft);
                state.session.editor = None;
            }
            Err(message) => state.error_banner = Some(message),
        },
        EffectOutcome::ThreadUpdated(result) => match result {
            Ok(thread) => {
                state.provider.threads.retain(|item| item.id != thread.id);
                state.provider.threads.push(thread);
            }
            Err(message) => state.error_banner = Some(message),
        },
        EffectOutcome::ReviewSubmitted(result) => match result {
            Ok(SubmitResult::Complete) => state.session.pending_submit = None,
            Ok(SubmitResult::Partial { retry, reason, .. }) => {
                if let Some(pending) = &mut state.session.pending_submit {
                    pending.mode = retry;
                }
                state.error_banner = Some(reason);
                state.dirty = true;
            }
            Err(message) => state.error_banner = Some(message),
        },
    }
    Vec::new()
}

fn set_error(state: &mut AppState, result: Result<(), String>) {
    if let Err(message) = result {
        state.error_banner = Some(message);
    }
}

fn envelope(
    state: &mut AppState,
    generation: Option<crate::domain::CommitOid>,
    effect: AppEffect,
) -> EffectEnvelope {
    let id = state.next_request_id;
    state.next_request_id += 1;
    state.busy_operations.insert(id);
    EffectEnvelope {
        id,
        generation,
        effect,
    }
}

fn move_index(current: usize, delta: i32, count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    if delta < 0 {
        current.saturating_sub(delta.unsigned_abs() as usize)
    } else {
        current.saturating_add(delta as usize).min(count - 1)
    }
}

fn move_index_wrapped(current: usize, delta: i32, count: usize) -> usize {
    if delta < 0 {
        (current + count - delta.unsigned_abs() as usize % count) % count
    } else {
        (current + delta as usize) % count
    }
}
