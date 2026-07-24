use crate::{
    diff::{DiffCursor, validate_selection},
    domain::{
        DiffPosition, DiffSelection, DiffSide, DraftId, ProviderKind, ReviewOutcome, SubmitMode,
        SubmitRequest, SubmitResult, Support, ThreadId,
    },
    state::{EditorSnapshot, PendingSubmit, ReviewSync},
};

use super::{
    AppAction, AppEffect, AppEvent, AppFocus, AppState, DisplayRow, EffectEnvelope, EffectOutcome,
    EffectResult, QuitChoice, SubmissionModal, generated::is_generated, refresh_display_rows,
};

/// Pushes a user-facing notice and arms its on-screen lifetime. Every refusal
/// or informational message the reducer surfaces must go through this so the
/// status bar has something to read (see `widgets::status::render`) — a bare
/// `state.notices.push(..)` is invisible, ~3s at the 250ms tick rate.
pub(crate) fn push_notice(state: &mut AppState, message: impl Into<String>) {
    state.notices.push(message.into());
    state.notice_ttl = 12;
}

pub fn update(state: &mut AppState, event: AppEvent) -> Vec<EffectEnvelope> {
    match event {
        AppEvent::Action(action) => action_update(state, action),
        AppEvent::Terminal(_) => Vec::new(),
        AppEvent::Tick => {
            if !state.pending_labels.is_empty() {
                state.spinner_frame = state.spinner_frame.wrapping_add(1);
            }
            state.notice_ttl = state.notice_ttl.saturating_sub(1);
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
        AppAction::MoveCursor(delta) => match state.focus {
            AppFocus::Files => navigate_by(state, delta.signum()),
            AppFocus::Diff => move_display_cursor(state, delta),
            AppFocus::Threads => Vec::new(),
        },
        AppAction::ToggleReviewed => toggle_reviewed(state),
        AppAction::ToggleSelection => {
            let on_diff_row = matches!(
                state.display_rows.get(state.display_cursor),
                Some(DisplayRow::Diff { .. })
            );
            if !on_diff_row && state.selection_anchor.is_none() {
                push_notice(state, "mova para uma linha de código");
                return Vec::new();
            }
            state.selection_anchor = match state.selection_anchor {
                Some(_) => None,
                None => Some(state.session.cursor_row),
            };
            state.dirty = true;
            Vec::new()
        }
        AppAction::OpenComment => {
            open_editor(state, false);
            Vec::new()
        }
        AppAction::OpenSuggestion => {
            open_editor(state, true);
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
        AppAction::EditComment(id) => {
            edit_comment(state, id);
            Vec::new()
        }
        AppAction::DeleteComment(id) => {
            state.delete_dialog = Some(id);
            state.delete_selected = 0;
            Vec::new()
        }
        AppAction::ConfirmDeleteChoice(confirm) => {
            let dialog_id = state.delete_dialog.take();
            state.delete_selected = 0;
            match (confirm, dialog_id) {
                (true, Some(id)) => vec![envelope(
                    state,
                    Some(state.provider.head.clone()),
                    AppEffect::DeleteDraft { id },
                )],
                _ => Vec::new(),
            }
        }
        AppAction::ReplyComment(thread) => {
            reply_comment(state, thread);
            Vec::new()
        }
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
            state.quit_selected = 0;
            Vec::new()
        }
        AppAction::ConfirmQuit(choice) => {
            match choice {
                QuitChoice::KeepSession => {
                    // `editing_draft`/`replying_thread` are in-memory only;
                    // the persisted `session.editor` cannot carry that
                    // identity across resume. Reopening it there would let
                    // Enter re-create the draft (duplicating an edit) or
                    // flatten a reply into a top-level comment. Mode editors
                    // are trivially recreatable, so discard rather than risk
                    // that.
                    if state.editing_draft.is_some() || state.replying_thread.is_some() {
                        state.session.editor = None;
                        state.editor_open = false;
                        state.editing_draft = None;
                        state.replying_thread = None;
                        state.dirty = true;
                    }
                    state.quit_requested = true;
                }
                QuitChoice::DiscardEditor => {
                    state.session.editor = None;
                    state.editing_draft = None;
                    state.replying_thread = None;
                    state.editor_open = false;
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
        AppAction::ToggleFilesPanel => {
            state.files_expanded = !state.files_expanded;
            Vec::new()
        }
        AppAction::ToggleFold => {
            if let Some(file) = state.provider.files.get(state.active_file_index) {
                let dir = directory_of(&file.path.0).to_owned();
                if !state.collapsed_dirs.remove(&dir) {
                    state.collapsed_dirs.insert(dir);
                }
            }
            Vec::new()
        }
        AppAction::ToggleComments => {
            state.comments_hidden = !state.comments_hidden;
            refresh_display_rows(state);
            Vec::new()
        }
    }
}

/// Moves `display_cursor` through the display rows, stopping only on rows
/// where a comment could be opened or a code line selected: `Diff` rows and
/// the first row of a `Comment` block. Continuation rows and the orphan
/// header are skipped over. Landing on a `Diff` row keeps `session.cursor_row`
/// in sync with it (the diff widget still scrolls by that value); landing on
/// a comment leaves `session.cursor_row` at whatever it last was.
fn move_display_cursor(state: &mut AppState, delta: i32) -> Vec<EffectEnvelope> {
    if state.display_rows.is_empty() {
        return Vec::new();
    }
    let mut index = state.display_cursor.min(state.display_rows.len() - 1);
    let step = delta.signum();
    for _ in 0..delta.unsigned_abs() {
        match next_stop(&state.display_rows, index, step) {
            Some(next) => index = next,
            None => break,
        }
    }
    state.display_cursor = index;
    if let Some(DisplayRow::Diff { row }) = state.display_rows.get(index) {
        state.session.cursor_row = *row;
    }
    state.dirty = true;
    Vec::new()
}

fn is_display_stop(row: &DisplayRow) -> bool {
    matches!(
        row,
        DisplayRow::Diff { .. }
            | DisplayRow::Comment {
                block_start: true,
                ..
            }
    )
}

fn next_stop(rows: &[DisplayRow], from: usize, step: i32) -> Option<usize> {
    if step == 0 {
        return None;
    }
    let mut cursor = from as i64 + step as i64;
    while cursor >= 0 && (cursor as usize) < rows.len() {
        let index = cursor as usize;
        if is_display_stop(&rows[index]) {
            return Some(index);
        }
        cursor += step as i64;
    }
    None
}

pub fn directory_of(path: &str) -> &str {
    path.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("")
}

fn is_folded(state: &AppState, index: usize) -> bool {
    state
        .provider
        .files
        .get(index)
        .is_some_and(|file| state.collapsed_dirs.contains(directory_of(&file.path.0)))
}

fn navigate_by(state: &mut AppState, delta: i32) -> Vec<EffectEnvelope> {
    let count = state.provider.files.len();
    if count == 0 {
        return Vec::new();
    }
    // Step over files hidden inside collapsed directories.
    let step = delta.signum();
    let mut index = state.active_file_index;
    loop {
        let next = move_index(index, step, count);
        if next == index {
            return Vec::new();
        }
        index = next;
        if !is_folded(state, index) {
            break;
        }
    }
    if index == state.active_file_index {
        return Vec::new();
    }
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
        let reviewed = state
            .session
            .files
            .get(path)
            .is_some_and(|progress| progress.reviewed);
        if !reviewed && !is_generated(&path.0) {
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
    refresh_display_rows(state);
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
    state.pending_labels.remove(&result.id);
    if result
        .generation
        .as_ref()
        .is_some_and(|generation| generation != &state.provider.head)
    {
        push_notice(state, "ignored stale operation from an older head");
        return Vec::new();
    }
    match result.outcome {
        EffectOutcome::Rendered(result) => match result {
            Ok(result) => {
                state.parsed_diff = Some(result.parsed);
                state.rendered_diff = Some(result.rendered);
                state.error_banner = None;
                refresh_display_rows(state);
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
            Ok(snapshot) => {
                state.provider = snapshot;
                refresh_display_rows(state);
            }
            Err(message) => state.error_banner = Some(message),
        },
        EffectOutcome::DraftCreated(result) | EffectOutcome::DraftUpdated(result) => match result {
            Ok(draft) => {
                state.provider.drafts.retain(|item| item.id != draft.id);
                state.provider.drafts.push(draft);
                state.session.editor = None;
                state.editor_open = false;
                state.editing_draft = None;
                refresh_display_rows(state);
            }
            Err(message) => state.error_banner = Some(message),
        },
        EffectOutcome::ThreadUpdated(result) => match result {
            Ok(thread) => {
                state.provider.threads.retain(|item| item.id != thread.id);
                state.provider.threads.push(thread);
                state.session.editor = None;
                state.editor_open = false;
                state.replying_thread = None;
                refresh_display_rows(state);
            }
            Err(message) => state.error_banner = Some(message),
        },
        EffectOutcome::DraftDeleted { id, result } => match result {
            Ok(()) => {
                state.provider.drafts.retain(|draft| draft.id != id);
                refresh_display_rows(state);
            }
            Err(message) => state.error_banner = Some(message),
        },
        EffectOutcome::ReviewSubmitted(result) => match result {
            Ok(SubmitResult::Complete) => {
                state.session.pending_submit = None;
                // Published drafts are still sitting in provider.drafts as
                // interactive blocks; refresh the snapshot so they turn into
                // read-only submitted comments.
                return vec![envelope(
                    state,
                    Some(state.provider.head.clone()),
                    AppEffect::RefreshSnapshot,
                )];
            }
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

fn open_editor(state: &mut AppState, suggestion: bool) {
    let on_comment_row = matches!(
        state.display_rows.get(state.display_cursor),
        Some(DisplayRow::Comment { .. })
    );
    if on_comment_row {
        push_notice(state, "mova para uma linha de código");
        return;
    }
    if let Some(editor) = &state.session.editor {
        if !editor.stale {
            state.editor_open = true;
            state.editor_suggestion = suggestion;
            return;
        }
        // A stale editor can never be submitted; replace it instead of
        // trapping every new comment in the read-only popup.
        state.session.editor = None;
        state.editor_open = false;
        state.dirty = true;
        push_notice(state, "discarded stale draft from a previous session");
    }
    let Some(diff) = state.parsed_diff.as_ref() else {
        state.error_banner = Some("diff is still loading".into());
        return;
    };
    let start_row = state.selection_anchor.unwrap_or(state.session.cursor_row);
    let end_row = state.session.cursor_row;
    let side = diff
        .rows
        .get(end_row)
        .and_then(|row| {
            row.right
                .as_ref()
                .map(|_| DiffSide::Right)
                .or_else(|| row.left.as_ref().map(|_| DiffSide::Left))
        })
        .unwrap_or(DiffSide::Right);
    match validate_selection(
        diff,
        DiffCursor {
            row: start_row,
            side,
        },
        DiffCursor { row: end_row, side },
    ) {
        Ok(selection) => {
            let path = selection.end.path.clone();
            state.session.editor = Some(EditorSnapshot {
                lines: vec![String::new()],
                cursor_row: 0,
                grapheme_col: 0,
                original_head: state.provider.head.clone(),
                path,
                selection,
                stale: false,
            });
            state.editor_open = true;
            state.editor_suggestion = suggestion;
            state.dirty = true;
        }
        Err(error) => state.error_banner = Some(error.to_string()),
    }
}

/// Opens the editor pre-filled with an existing draft's body so it can be
/// edited in place. `UpdateDraft` only ever sends `id` and the new `body` to
/// the provider (see `AppEffect::UpdateDraft`), so the selection carried on
/// the resulting `EditorSnapshot` is never transmitted — when the draft has
/// none recorded, a placeholder anchored at the active file's first line is
/// harmless.
fn edit_comment(state: &mut AppState, id: DraftId) {
    if state.session.editor.is_some()
        && state.editing_draft.is_none()
        && state.replying_thread.is_none()
    {
        push_notice(
            state,
            "você tem um comentário não salvo; salve (c → Enter) ou descarte antes de editar",
        );
        return;
    }
    let Some(draft) = state.provider.drafts.iter().find(|draft| draft.id == id) else {
        state.error_banner = Some("draft comment not found".into());
        return;
    };
    let lines = if draft.body.is_empty() {
        vec![String::new()]
    } else {
        draft.body.lines().map(str::to_owned).collect()
    };
    let selection = draft
        .selection
        .clone()
        .unwrap_or_else(|| placeholder_selection(state));
    let path = selection.end.path.clone();
    state.session.editor = Some(EditorSnapshot {
        lines,
        cursor_row: 0,
        grapheme_col: 0,
        original_head: state.provider.head.clone(),
        path,
        selection,
        stale: false,
    });
    state.editing_draft = Some(id);
    state.editor_open = true;
    state.editor_suggestion = false;
    state.dirty = true;
}

/// Opens an empty editor in reply mode. `Reply` only ever sends `thread` and
/// the new `body` to the provider (see `AppEffect::Reply`), so — exactly as
/// in [`edit_comment`] — the selection on the resulting `EditorSnapshot` is
/// never transmitted and a placeholder is harmless.
fn reply_comment(state: &mut AppState, thread: ThreadId) {
    if state.session.editor.is_some()
        && state.editing_draft.is_none()
        && state.replying_thread.is_none()
    {
        push_notice(
            state,
            "você tem um comentário não salvo; salve (c → Enter) ou descarte antes de responder",
        );
        return;
    }
    let Some(thread_ref) = state.provider.threads.iter().find(|item| item.id == thread) else {
        state.error_banner = Some("thread not found".into());
        return;
    };
    let selection = thread_ref
        .comments
        .iter()
        .find_map(|comment| comment.position.clone())
        .map(|position| DiffSelection {
            start: position.clone(),
            end: position,
        })
        .unwrap_or_else(|| placeholder_selection(state));
    let path = selection.end.path.clone();
    state.session.editor = Some(EditorSnapshot {
        lines: vec![String::new()],
        cursor_row: 0,
        grapheme_col: 0,
        original_head: state.provider.head.clone(),
        path,
        selection,
        stale: false,
    });
    state.replying_thread = Some(thread);
    state.editor_open = true;
    state.editor_suggestion = false;
    state.dirty = true;
}

fn placeholder_selection(state: &AppState) -> DiffSelection {
    let path = state
        .provider
        .files
        .get(state.active_file_index)
        .map(|file| file.path.clone())
        .unwrap_or_else(|| crate::domain::RepoPath(String::new()));
    let position = DiffPosition {
        path,
        side: DiffSide::Right,
        line: 1,
        hunk: 0,
    };
    DiffSelection {
        start: position.clone(),
        end: position,
    }
}

/// Human-facing label for effects whose progress the status bar reports.
fn effect_label(effect: &AppEffect) -> Option<&'static str> {
    match effect {
        AppEffect::CreateDraft { .. } => Some("salvando comentário…"),
        AppEffect::UpdateDraft { .. } => Some("atualizando comentário…"),
        AppEffect::DeleteDraft { .. } => Some("excluindo comentário…"),
        AppEffect::Reply { .. } => Some("respondendo…"),
        AppEffect::SubmitReview { .. } => Some("enviando revisão…"),
        AppEffect::RefreshSnapshot => Some("atualizando…"),
        _ => None,
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
    if let Some(label) = effect_label(&effect) {
        state.pending_labels.insert(id, label);
    }
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
