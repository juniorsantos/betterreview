use std::collections::BTreeSet;

use crate::{
    diff::{DiffCursor, validate_selection},
    domain::{
        DiffLayout, DiffPosition, DiffSelection, DiffSide, DraftId, ProviderKind, ReviewOutcome,
        SubmitMode, SubmitRequest, SubmitResult, Support, ThreadId,
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
        AppAction::FocusLeft => {
            state.focus = match state.focus {
                AppFocus::Files => AppFocus::Files,
                AppFocus::Diff => AppFocus::Files,
                AppFocus::Threads => AppFocus::Diff,
            };
            Vec::new()
        }
        AppAction::FocusRight => {
            state.focus = match state.focus {
                AppFocus::Files => AppFocus::Diff,
                AppFocus::Diff => AppFocus::Threads,
                AppFocus::Threads => AppFocus::Threads,
            };
            Vec::new()
        }
        AppAction::FocusFiles => {
            state.files_hidden = false;
            state.focus = AppFocus::Files;
            refresh_display_rows(state);
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
        AppAction::ActivateFile(index) => {
            if index >= state.provider.files.len() {
                return Vec::new();
            }
            if is_folded(state, index) && !is_directory_representative(state, index) {
                return Vec::new();
            }
            activate_file(state, index)
        }
        AppAction::ToggleFoldDir(dir) => {
            if !state.collapsed_dirs.remove(&dir) {
                state.collapsed_dirs.insert(dir);
            }
            Vec::new()
        }
        AppAction::JumpToDisplayRow(index) => jump_to_display_row(state, index),
        AppAction::ToggleReviewed => toggle_reviewed(state),
        AppAction::ToggleHunkReviewed => toggle_hunk_reviewed(state),
        AppAction::ToggleSelection => {
            let on_diff_row = state
                .display_rows
                .get(state.display_cursor)
                .and_then(DisplayRow::anchor_row)
                .is_some();
            if !on_diff_row && state.selection_anchor.is_none() {
                push_notice(state, "move to a code line");
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
                push_notice(state, format!("unavailable: {reason}"));
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
        AppAction::Quit => leave_review(state, false),
        AppAction::BackToPicker => leave_review(state, true),
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
                QuitChoice::Cancel => state.return_to_picker = false,
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
        AppAction::ToggleFilesVisible => {
            state.files_hidden = !state.files_hidden;
            if state.files_hidden && state.focus == AppFocus::Files {
                state.focus = AppFocus::Diff;
            }
            refresh_display_rows(state);
            vec![envelope(
                state,
                None,
                AppEffect::SaveConfig {
                    config: config_of(state),
                },
            )]
        }
        AppAction::ToggleFold => {
            // `z` is contextual: folders only from the Files panel (in the
            // diff it expands hidden-context gaps via the key dispatch).
            if state.focus != AppFocus::Files {
                push_notice(
                    state,
                    "z expands the diff's `· · ·` gaps; for folders, focus panel [2]",
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
        AppAction::ToggleDiffLayout => toggle_diff_layout(state),
        AppAction::CycleSplitSide => cycle_split_side(state),
        AppAction::ToggleWrap => toggle_wrap(state),
        AppAction::ToggleBlame => toggle_blame(state),
        AppAction::DismissBlocked => {
            state.blocked = None;
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
        push_notice(state, "no matches");
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
    if let Some(row) = state
        .display_rows
        .get(index)
        .and_then(DisplayRow::anchor_row)
    {
        state.session.cursor_row = row;
    }
    state.dirty = true;
}

/// Lands on `index` after clamping it into `display_rows` and snapping a
/// non-stop row back to the nearest preceding stop (see `snap_to_stop`) —
/// the mouse click can land on any row the diff panel drew, not just a
/// navigation stop.
fn jump_to_display_row(state: &mut AppState, index: usize) -> Vec<EffectEnvelope> {
    if state.display_rows.is_empty() {
        return Vec::new();
    }
    let clamped = index.min(state.display_rows.len() - 1);
    let target = snap_to_stop(&state.display_rows, clamped);
    land_on_display_row(state, target);
    Vec::new()
}

/// Walks backward from `index` to the nearest display-stop row. Comment
/// blocks are pushed contiguously (`Header`, `Body`*, `Footer`), so this
/// finds a block's header the same way search's `block_header_index` does —
/// but it also covers `FileHeader`/`OrphanHeader`, falling back to `index`
/// itself when nothing behind it is a stop.
fn snap_to_stop(rows: &[DisplayRow], index: usize) -> usize {
    let mut cursor = index;
    loop {
        if is_display_stop(&rows[cursor]) {
            return cursor;
        }
        match cursor.checked_sub(1) {
            Some(previous) => cursor = previous,
            None => return index,
        }
    }
}

fn is_display_stop(row: &DisplayRow) -> bool {
    matches!(
        row,
        DisplayRow::Diff { .. }
            | DisplayRow::SplitDiff { .. }
            | DisplayRow::Comment {
                kind: CommentRowKind::Header,
                ..
            }
            | DisplayRow::Gap { .. }
            | DisplayRow::Context { .. }
            | DisplayRow::HunkHeader { .. }
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
    if state.parsed_diff.is_none() {
        push_notice(state, "diff is still loading");
        return Vec::new();
    }
    let target = find_display_row(&state.display_rows, state.display_cursor, step, |row| {
        matches!(row, DisplayRow::HunkHeader { .. })
    });
    match target {
        Some(index) => land_on_display_row(state, index),
        None => push_notice(
            state,
            if step > 0 {
                "no next hunk"
            } else {
                "no previous hunk"
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
                "no next comment"
            } else {
                "no previous comment"
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
    if state.folder_selected {
        state.folder_selected = false;
        if step > 0 {
            return Vec::new();
        }
    } else if step < 0
        && is_directory_representative(state, state.active_file_index)
        && !is_folded(state, state.active_file_index)
    {
        state.folder_selected = true;
        return Vec::new();
    }
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
    state.folder_selected = false;
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

fn toggle_diff_layout(state: &mut AppState) -> Vec<EffectEnvelope> {
    state.diff_layout = match state.diff_layout {
        DiffLayout::Unified => DiffLayout::Split,
        DiffLayout::Split => DiffLayout::Auto,
        DiffLayout::Auto => DiffLayout::Unified,
    };
    refresh_display_rows(state);
    if state.diff_layout == DiffLayout::Split
        && crate::app::diff_panel_width(state) < crate::app::SPLIT_MIN_DIFF_WIDTH
    {
        push_notice(
            state,
            "side-by-side needs a wider diff panel; try f to hide the files panel",
        );
    }
    vec![envelope(
        state,
        None,
        AppEffect::SaveConfig {
            config: config_of(state),
        },
    )]
}

fn config_of(state: &AppState) -> crate::state::AppConfig {
    crate::state::AppConfig {
        diff_layout: state.diff_layout,
        files_hidden: state.files_hidden,
        wrap_lines: state.wrap_lines,
        tab_width: state.tab_width,
        transparent: state.transparent,
    }
}

fn toggle_blame(state: &mut AppState) -> Vec<EffectEnvelope> {
    state.blame_visible = !state.blame_visible;
    let Some(file) = state.provider.files.get(state.active_file_index) else {
        return Vec::new();
    };
    if !state.blame_visible || state.blame.contains_key(&file.path) {
        return Vec::new();
    }
    let path = file.path.clone();
    let base = state.provider.base.clone();
    vec![envelope(
        state,
        Some(state.provider.head.clone()),
        AppEffect::LoadBlame {
            path,
            revision: base,
        },
    )]
}

fn toggle_wrap(state: &mut AppState) -> Vec<EffectEnvelope> {
    state.wrap_lines = !state.wrap_lines;
    vec![envelope(
        state,
        None,
        AppEffect::SaveConfig {
            config: config_of(state),
        },
    )]
}

fn cycle_split_side(state: &mut AppState) -> Vec<EffectEnvelope> {
    if crate::app::effective_layout(state) != DiffLayout::Split {
        push_notice(state, "expanding a side needs the split layout (\\)");
        return Vec::new();
    }
    state.split_focus = match state.split_focus {
        None => Some(crate::tui::SplitSide::New),
        Some(crate::tui::SplitSide::New) => Some(crate::tui::SplitSide::Old),
        Some(crate::tui::SplitSide::Old) => None,
    };
    Vec::new()
}

fn leave_review(state: &mut AppState, to_picker: bool) -> Vec<EffectEnvelope> {
    state.return_to_picker = to_picker;
    if state.session.editor.is_none() {
        state.quit_requested = true;
    } else {
        state.quit_dialog = true;
        state.quit_selected = 0;
    }
    Vec::new()
}

fn active_path(state: &AppState) -> Option<crate::domain::RepoPath> {
    state
        .provider
        .files
        .get(state.active_file_index)
        .map(|file| file.path.clone())
}

fn toggle_reviewed(state: &mut AppState) -> Vec<EffectEnvelope> {
    let Some(path) = active_path(state) else {
        return Vec::new();
    };
    let Some(progress) = state.session.files.get(&path) else {
        return Vec::new();
    };
    let reviewed = !progress.reviewed;
    let hunks = if reviewed {
        (0..state.hunk_total(&path)).collect()
    } else {
        BTreeSet::new()
    };
    let effects = set_file_reviewed(state, &path, reviewed, Some(hunks));
    push_notice(
        state,
        if reviewed {
            "✓ file marked as reviewed"
        } else {
            "file unmarked"
        },
    );
    effects
}

fn set_file_reviewed(
    state: &mut AppState,
    path: &crate::domain::RepoPath,
    reviewed: bool,
    hunks: Option<BTreeSet<u32>>,
) -> Vec<EffectEnvelope> {
    let remote_supported = matches!(
        state.provider.capabilities.mark_file_reviewed,
        Support::Supported
    );
    let sync = match (state.provider.key.provider, remote_supported) {
        (ProviderKind::GitHub, true) => ReviewSync::Pending { desired: reviewed },
        _ => ReviewSync::LocalOnly,
    };
    let Some(progress) = state.session.files.get_mut(path) else {
        return Vec::new();
    };
    progress.reviewed = reviewed;
    if let Some(hunks) = hunks {
        progress.reviewed_hunks = hunks;
    }
    progress.sync = sync;
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
    effects.push(save_session(state));
    effects
}

fn save_session(state: &mut AppState) -> EffectEnvelope {
    envelope(
        state,
        None,
        AppEffect::SaveSession {
            snapshot: Box::new(state.session.clone()),
        },
    )
}

fn hunk_at_cursor(state: &AppState) -> Option<u32> {
    let display_row = state.display_rows.get(state.display_cursor)?;
    if let DisplayRow::HunkHeader { hunk } = display_row {
        return Some(*hunk);
    }
    let row = display_row.anchor_row()?;
    state
        .parsed_diff
        .as_ref()?
        .hunks
        .iter()
        .find(|hunk| hunk.row_range.contains(&row))
        .map(|hunk| hunk.id)
}

fn toggle_hunk_reviewed(state: &mut AppState) -> Vec<EffectEnvelope> {
    let Some(hunk) = hunk_at_cursor(state) else {
        push_notice(state, "no hunk under the cursor");
        return Vec::new();
    };
    let Some(path) = active_path(state) else {
        return Vec::new();
    };
    let total = state.hunk_total(&path);
    let Some(progress) = state.session.files.get_mut(&path) else {
        return Vec::new();
    };
    let marked = !progress.reviewed_hunks.remove(&hunk);
    if marked {
        progress.reviewed_hunks.insert(hunk);
    }
    let done = progress.reviewed_hunks.len() as u32;
    let complete = total > 0 && done == total;
    let was_reviewed = progress.reviewed;

    let effects = if complete == was_reviewed {
        vec![save_session(state)]
    } else {
        set_file_reviewed(state, &path, complete, None)
    };
    let message = if complete {
        format!("✓ file reviewed ({done}/{total} hunks)")
    } else if marked {
        format!("✓ hunk {} reviewed ({done}/{total})", hunk + 1)
    } else {
        format!("hunk {} unmarked ({done}/{total})", hunk + 1)
    };
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
        push_notice(state, "stale operation ignored (head changed)");
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
        EffectOutcome::Saved(result) => set_error(state, result),
        EffectOutcome::Completed(result) => {
            if result.is_ok() {
                return vec![envelope(
                    state,
                    Some(state.provider.head.clone()),
                    AppEffect::RefreshSnapshot,
                )];
            }
            set_error(state, result);
        }
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
                state.refresh_hunk_totals();
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
                state.dirty = true;
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
                state.dirty = true;
                refresh_display_rows(state);
                return vec![envelope(
                    state,
                    Some(state.provider.head.clone()),
                    AppEffect::RefreshSnapshot,
                )];
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
                state.dirty = true;
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
        EffectOutcome::BlameLoaded { path, result } => match result {
            Ok(lines) => {
                state.blame.insert(path, lines);
            }
            Err(reason) => {
                state.blame_visible = false;
                state.blocked = Some(crate::app::Blocked {
                    title: "Blame unavailable".into(),
                    guidance: crate::blame::guidance(&reason),
                    reason,
                });
            }
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
        Some(
            DisplayRow::Comment { .. }
                | DisplayRow::Gap { .. }
                | DisplayRow::Context { .. }
                | DisplayRow::HunkHeader { .. }
        )
    );
    if blocks_editor {
        push_notice(state, "move to a code line");
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
            "you have an unsaved comment; save it (c → ↵) or discard it before editing",
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
            "you have an unsaved comment; save it (c → ↵) or discard it before replying",
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
        AppEffect::CreateDraft { .. } => Some("saving comment…"),
        AppEffect::UpdateDraft { .. } => Some("updating comment…"),
        AppEffect::DeleteDraft { .. } => Some("deleting comment…"),
        AppEffect::Reply { .. } => Some("replying…"),
        AppEffect::SubmitReview { .. } => Some("submitting review…"),
        AppEffect::RefreshSnapshot => Some("refreshing…"),
        AppEffect::LoadFileContext { .. } => Some("loading context…"),
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
