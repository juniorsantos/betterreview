use crate::{
    diff::{DiffCursor, DiffRowKind, validate_selection},
    domain::{
        DiffPosition, DiffSelection, DiffSide, DraftId, ProviderKind, ReviewOutcome, SubmitMode,
        SubmitRequest, SubmitResult, Support, ThreadId,
    },
    state::{EditorSnapshot, PendingSubmit, ReviewSync},
};

use super::{
    AppAction, AppEffect, AppEvent, AppFocus, AppState, CommentRowKind, DisplayRow, EffectEnvelope,
    EffectOutcome, EffectResult, QuitChoice, SubmissionModal, generated::is_generated,
    refresh_display_rows,
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
        AppAction::FocusFiles => {
            state.focus = AppFocus::Files;
            Vec::new()
        }
        AppAction::FocusDiff => {
            state.focus = AppFocus::Diff;
            Vec::new()
        }
        AppAction::NextFile => navigate_by(state, 1),
        AppAction::PreviousFile => navigate_by(state, -1),
        AppAction::NextUnreviewed => navigate_unreviewed(state, 1),
        AppAction::PreviousUnreviewed => navigate_unreviewed(state, -1),
        AppAction::NextHunk => jump_hunk(state, 1),
        AppAction::PreviousHunk => jump_hunk(state, -1),
        AppAction::NextComment => jump_comment(state, 1),
        AppAction::PreviousComment => jump_comment(state, -1),
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
            if let Support::Unsupported { reason } =
                state.provider.capabilities.for_outcome(outcome)
            {
                push_notice(state, format!("indisponível: {reason}"));
                return Vec::new();
            }
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
            // Without an unsaved editor draft both dialog choices are
            // identical (the session is always persisted) — just leave.
            if state.session.editor.is_none() {
                state.quit_requested = true;
                return Vec::new();
            }
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
            // `z` is contextual: folders only from the Files panel (in the
            // diff it expands hidden-context gaps via the key dispatch).
            if state.focus != AppFocus::Files {
                push_notice(
                    state,
                    "z expande as lacunas `· · ·` do diff; para pastas, foque o painel [2]",
                );
                return Vec::new();
            }
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
        AppAction::ExpandGap => expand_gap(state),
        AppAction::ConfirmSearch => confirm_search(state),
        AppAction::SearchNext => search_step(state, 1),
        AppAction::SearchPrevious => search_step(state, -1),
        AppAction::CancelSearch => {
            state.search_input = None;
            state.search_query = None;
            Vec::new()
        }
    }
}

/// Fixes the typed query as the active search and jumps to the first match
/// at or after the cursor — an empty query (typed nothing, just hit `Enter`)
/// clears the search instead of fixing an empty one.
fn confirm_search(state: &mut AppState) -> Vec<EffectEnvelope> {
    let typed = state.search_input.take().unwrap_or_default();
    let trimmed = typed.trim();
    if trimmed.is_empty() {
        state.search_query = None;
        return Vec::new();
    }
    state.search_query = Some(trimmed.to_owned());
    jump_to_match(state, state.display_cursor, 1, true)
}

/// Steps to the next (`step = 1`) or previous (`step = -1`) match, wrapping
/// around the ends of the match list. A no-op when there is no active query.
fn search_step(state: &mut AppState, step: i32) -> Vec<EffectEnvelope> {
    if state.search_query.is_none() {
        return Vec::new();
    }
    jump_to_match(state, state.display_cursor, step, false)
}

/// Lands on the nearest match to `from` in `step`'s direction. `inclusive`
/// lets a match sitting exactly on `from` count (used by `confirm_search`,
/// which searches "from the cursor" onward); `n`/`N` always look strictly
/// past the cursor so repeated presses keep advancing. Wraps to the first
/// (or last) match when nothing is found in that direction; a notice is
/// left when there are no matches at all.
fn jump_to_match(
    state: &mut AppState,
    from: usize,
    step: i32,
    inclusive: bool,
) -> Vec<EffectEnvelope> {
    let matches = crate::app::search_matches(state);
    let Some(&target) = (if step >= 0 {
        matches
            .iter()
            .find(|&&index| {
                if inclusive {
                    index >= from
                } else {
                    index > from
                }
            })
            .or_else(|| matches.first())
    } else {
        matches
            .iter()
            .rev()
            .find(|&&index| {
                if inclusive {
                    index <= from
                } else {
                    index < from
                }
            })
            .or_else(|| matches.last())
    }) else {
        push_notice(state, "sem resultados");
        return Vec::new();
    };
    land_on_display_row(state, target);
    Vec::new()
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
        match find_display_row(&state.display_rows, index, step, is_display_stop) {
            Some(next) => index = next,
            None => break,
        }
    }
    if index == state.display_cursor {
        // Hit the edge of the file: flow into the neighbor to keep the
        // review moving — forward lands at its top, backward at its end
        // (positioned once its diff renders).
        if step > 0 {
            return navigate_by(state, 1);
        }
        if step < 0 {
            let effects = navigate_by(state, -1);
            if !effects.is_empty() {
                state.enter_file_at_end = true;
            }
            return effects;
        }
    }
    land_on_display_row(state, index);
    Vec::new()
}

/// Lands the display cursor on `index`, keeping `session.cursor_row` in sync
/// when that row is a `Diff` row (a comment row leaves it untouched — see
/// `move_display_cursor`'s doc comment above for why). Every action that
/// jumps the cursor around the diff — arrow movement, hunk/comment jumps,
/// search — funnels through here so the landing semantics stay identical.
fn land_on_display_row(state: &mut AppState, index: usize) {
    state.display_cursor = index;
    if let Some(DisplayRow::Diff { row }) = state.display_rows.get(index) {
        state.session.cursor_row = *row;
    }
    state.dirty = true;
}

fn is_display_stop(row: &DisplayRow) -> bool {
    matches!(
        row,
        DisplayRow::Diff { .. }
            | DisplayRow::Comment {
                kind: CommentRowKind::Header,
                ..
            }
            | DisplayRow::Gap { .. }
            | DisplayRow::Context { .. }
    )
}

/// Scans the display rows from `from` (exclusive) in `step`'s direction
/// (`1` forward, `-1` backward) for the first row matching `predicate`, with
/// no wraparound. Shared by cursor movement, hunk jumps and comment jumps.
fn find_display_row(
    rows: &[DisplayRow],
    from: usize,
    step: i32,
    predicate: impl Fn(&DisplayRow) -> bool,
) -> Option<usize> {
    if step == 0 {
        return None;
    }
    let mut cursor = from as i64 + step as i64;
    while cursor >= 0 && (cursor as usize) < rows.len() {
        let index = cursor as usize;
        if predicate(&rows[index]) {
            return Some(index);
        }
        cursor += step as i64;
    }
    None
}

/// Jumps to the next/previous hunk header (`]h`/`[h`). Clamps at the first or
/// last hunk — no wraparound — and leaves a notice when there is nowhere
/// left to go, or when the diff has not parsed yet.
fn jump_hunk(state: &mut AppState, step: i32) -> Vec<EffectEnvelope> {
    let Some(diff) = state.parsed_diff.as_ref() else {
        push_notice(state, "diff ainda carregando");
        return Vec::new();
    };
    // The raw `@@` rows are hidden from the display; a hunk's landing spot
    // is its first code row (the one right after the parsed HunkHeader).
    let target = find_display_row(&state.display_rows, state.display_cursor, step, |row| {
        matches!(row, DisplayRow::Diff { row } if *row > 0
                && diff.rows.get(*row - 1).is_some_and(|prev| prev.kind == DiffRowKind::HunkHeader))
    });
    match target {
        Some(index) => land_on_display_row(state, index),
        None => push_notice(
            state,
            if step > 0 {
                "não há próximo hunk"
            } else {
                "não há hunk anterior"
            },
        ),
    }
    Vec::new()
}

/// Jumps to the next/previous comment block (`]c`/`[c`). Same clamp-and-notice
/// semantics as [`jump_hunk`].
fn jump_comment(state: &mut AppState, step: i32) -> Vec<EffectEnvelope> {
    let target = find_display_row(&state.display_rows, state.display_cursor, step, |row| {
        matches!(
            row,
            DisplayRow::Comment {
                kind: CommentRowKind::Header,
                ..
            }
        )
    });
    match target {
        Some(index) => land_on_display_row(state, index),
        None => push_notice(
            state,
            if step > 0 {
                "não há próximo comentário"
            } else {
                "não há comentário anterior"
            },
        ),
    }
    Vec::new()
}

pub fn directory_of(path: &str) -> &str {
    path.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("")
}

/// The first file of its directory: the row the fold highlight collapses
/// onto, and therefore the navigation stop that keeps a folded directory
/// reachable.
fn is_directory_representative(state: &AppState, index: usize) -> bool {
    let Some(file) = state.provider.files.get(index) else {
        return false;
    };
    let dir = directory_of(&file.path.0);
    state
        .provider
        .files
        .iter()
        .position(|candidate| directory_of(&candidate.path.0) == dir)
        == Some(index)
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
    // Step over files hidden inside collapsed directories — but a folded
    // directory stays reachable through its FIRST file (shown highlighted
    // on the directory header), so `z` can always unfold it again.
    let step = delta.signum();
    let mut index = state.active_file_index;
    loop {
        let next = move_index(index, step, count);
        if next == index {
            return Vec::new();
        }
        index = next;
        if !is_folded(state, index) || is_directory_representative(state, index) {
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
    state.enter_file_at_end = false;
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
    state.expanded_gaps.clear();
    state.pending_gap = None;
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
    let message = if reviewed {
        "✓ arquivo marcado como revisado"
    } else {
        "arquivo desmarcado"
    };
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
    push_notice(state, message);
    effects
}

/// `z` on a `Gap` row: expands it in place when the active file's contents
/// are already cached, or schedules `LoadFileContext` and parks the gap's
/// key in `pending_gap` so the response can expand it once it lands. A no-op
/// when the cursor isn't on a `Gap` row.
fn expand_gap(state: &mut AppState) -> Vec<EffectEnvelope> {
    let Some(DisplayRow::Gap { after_new_line, .. }) =
        state.display_rows.get(state.display_cursor).cloned()
    else {
        return Vec::new();
    };
    let Some(active_path) = state
        .provider
        .files
        .get(state.active_file_index)
        .map(|file| file.path.clone())
    else {
        return Vec::new();
    };
    if state.file_contexts.contains_key(&active_path) {
        if !state.expanded_gaps.remove(&after_new_line) {
            state.expanded_gaps.insert(after_new_line);
        }
        refresh_display_rows(state);
        return Vec::new();
    }
    state.pending_gap = Some(after_new_line);
    vec![envelope(
        state,
        Some(state.provider.head.clone()),
        AppEffect::LoadFileContext {
            path: active_path,
            revision: state.provider.head.clone(),
        },
    )]
}

fn finish_effect(state: &mut AppState, result: EffectResult) -> Vec<EffectEnvelope> {
    state.busy_operations.remove(&result.id);
    state.pending_labels.remove(&result.id);
    if result
        .generation
        .as_ref()
        .is_some_and(|generation| generation != &state.provider.head)
    {
        push_notice(state, "operação antiga ignorada (head mudou)");
        return Vec::new();
    }
    match result.outcome {
        EffectOutcome::Rendered(result) => match result {
            Ok(result) => {
                state.parsed_diff = Some(result.parsed);
                state.rendered_diff = Some(result.rendered);
                state.error_banner = None;
                refresh_display_rows(state);
                if state.enter_file_at_end {
                    state.enter_file_at_end = false;
                    if let Some(last) = state.display_rows.iter().rposition(is_display_stop) {
                        land_on_display_row(state, last);
                    }
                }
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
                state.enter_file_at_end = false;
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
            Ok(mut draft) => {
                // Update responses omit position data; keep the previous
                // anchor so the card stays attached to its line.
                if let Some(previous) = state
                    .provider
                    .drafts
                    .iter()
                    .find(|item| item.id == draft.id)
                {
                    if draft.selection.is_none() {
                        draft.selection = previous.selection.clone();
                    }
                    if draft.thread_id.is_none() {
                        draft.thread_id = previous.thread_id.clone();
                    }
                }
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
        EffectOutcome::FileContextLoaded { path, result } => match result {
            Ok(content) => {
                let lines: Vec<String> = content.split('\n').map(str::to_owned).collect();
                state.file_contexts.insert(path, lines);
                if let Some(gap) = state.pending_gap.take() {
                    state.expanded_gaps.insert(gap);
                }
                refresh_display_rows(state);
            }
            Err(message) => {
                state.pending_gap = None;
                state.error_banner = Some(message);
            }
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
    let blocks_editor = matches!(
        state.display_rows.get(state.display_cursor),
        Some(DisplayRow::Comment { .. } | DisplayRow::Gap { .. } | DisplayRow::Context { .. })
    );
    if blocks_editor {
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
        AppEffect::LoadFileContext { .. } => Some("carregando contexto…"),
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
