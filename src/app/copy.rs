use std::ops::Range;

use crate::{
    diff::{DiffCursor, DiffHunk, DiffRowKind, ParsedFileDiff, validate_selection},
    domain::{DiffPosition, DiffSelection, DiffSide, RepoPath},
    tui::SplitSide,
};

use super::{AppState, DisplayRow};

pub(super) enum CopyTarget {
    LineOrSelection,
    Hunk,
    PatchHunk,
    AllComments,
}

pub(super) struct PreparedCopy {
    pub content: String,
    pub notice: &'static str,
}

pub(super) fn prepare(state: &AppState, target: CopyTarget) -> Result<PreparedCopy, String> {
    match target {
        CopyTarget::LineOrSelection => line_or_selection(state),
        CopyTarget::Hunk => hunk(state),
        CopyTarget::PatchHunk => patch_hunk(state),
        CopyTarget::AllComments => all_comments(state),
    }
}

fn line_or_selection(state: &AppState) -> Result<PreparedCopy, String> {
    let diff = state
        .parsed_diff
        .as_ref()
        .ok_or_else(|| "diff is still loading".to_owned())?;
    let row_index = display_code_row(state).ok_or_else(|| "move to a code line".to_owned())?;
    let side = copy_side(state, row_index).ok_or_else(|| "move to a code line".to_owned())?;
    let selected = state.selection_anchor.is_some();
    let start = state.selection_anchor.unwrap_or(row_index);
    let (first, last) = if start <= row_index {
        (start, row_index)
    } else {
        (row_index, start)
    };
    validate_selection(
        diff,
        DiffCursor { row: first, side },
        DiffCursor { row: last, side },
    )
    .map_err(|error| error.to_string())?;
    let content = clean_rows(diff, first..last.saturating_add(1), side);
    if content.is_empty() {
        return Err("move to a code line".into());
    }
    Ok(PreparedCopy {
        content,
        notice: if selected {
            "copied selection"
        } else {
            "copied line"
        },
    })
}

fn hunk(state: &AppState) -> Result<PreparedCopy, String> {
    let diff = state
        .parsed_diff
        .as_ref()
        .ok_or_else(|| "diff is still loading".to_owned())?;
    let row_index = display_code_row(state);
    let hunk = hunk_at_cursor(state, diff).ok_or_else(|| "move to a hunk".to_owned())?;
    let side = row_index
        .and_then(|index| copy_side(state, index))
        .or_else(|| hunk_side(state, diff, hunk.row_range.clone()))
        .ok_or_else(|| "move to a hunk".to_owned())?;
    let content = clean_rows(diff, hunk.row_range.clone(), side);
    if content.is_empty() {
        return Err("move to a hunk".into());
    }
    Ok(PreparedCopy {
        content,
        notice: "copied hunk",
    })
}

fn patch_hunk(state: &AppState) -> Result<PreparedCopy, String> {
    let diff = state
        .parsed_diff
        .as_ref()
        .ok_or_else(|| "diff is still loading".to_owned())?;
    let hunk = hunk_at_cursor(state, diff).ok_or_else(|| "move to a hunk".to_owned())?;
    let header = hunk
        .row_range
        .start
        .checked_sub(1)
        .filter(|&index| {
            diff.rows
                .get(index)
                .is_some_and(|row| row.kind == DiffRowKind::HunkHeader)
        })
        .ok_or_else(|| "move to a hunk".to_owned())?;
    let content = diff.rows[header..hunk.row_range.end]
        .iter()
        .map(|row| row.raw.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    Ok(PreparedCopy {
        content,
        notice: "copied patch hunk",
    })
}

fn all_comments(state: &AppState) -> Result<PreparedCopy, String> {
    let mut comments = Vec::new();
    for thread in &state.provider.threads {
        let anchor = thread
            .comments
            .iter()
            .find_map(|comment| comment.position.as_ref());
        for comment in &thread.comments {
            comments.push(markdown_comment(
                location(&thread.path, comment.position.as_ref().or(anchor)),
                &format!("@{}", comment.author),
                &comment.body,
            ));
        }
    }
    for draft in &state.provider.drafts {
        let location = draft
            .selection
            .as_ref()
            .map(selection_location)
            .or_else(|| draft_thread_location(state, draft.thread_id.as_ref()))
            .unwrap_or_else(|| "review (unanchored)".into());
        comments.push(markdown_comment(location, "you · draft", &draft.body));
    }
    if comments.is_empty() {
        return Err("no comments to copy".into());
    }
    Ok(PreparedCopy {
        content: comments.join("\n\n"),
        notice: "copied all comments",
    })
}

fn hunk_at_cursor<'a>(state: &AppState, diff: &'a ParsedFileDiff) -> Option<&'a DiffHunk> {
    let hunk_id = match state.display_rows.get(state.display_cursor) {
        Some(DisplayRow::HunkHeader { hunk }) => Some(*hunk),
        _ => display_code_row(state).and_then(|index| {
            diff.rows
                .get(index)
                .and_then(|row| row.right.as_ref().or(row.left.as_ref()))
                .map(|position| position.hunk)
        }),
    }?;
    diff.hunks.iter().find(|hunk| hunk.id == hunk_id)
}

fn draft_thread_location(
    state: &AppState,
    thread_id: Option<&crate::domain::ThreadId>,
) -> Option<String> {
    let thread = state
        .provider
        .threads
        .iter()
        .find(|thread| Some(&thread.id) == thread_id)?;
    let position = thread
        .comments
        .iter()
        .find_map(|comment| comment.position.as_ref());
    Some(location(&thread.path, position))
}

fn selection_location(selection: &DiffSelection) -> String {
    let path = &selection.end.path.0;
    let first = selection.start.line.min(selection.end.line);
    let last = selection.start.line.max(selection.end.line);
    if first == last {
        format!("{path}:{last}")
    } else {
        format!("{path}:{first}-{last}")
    }
}

fn location(path: &RepoPath, position: Option<&DiffPosition>) -> String {
    match position {
        Some(position) => format!("{}:{}", position.path.0, position.line),
        None if !path.0.is_empty() => format!("{} (unanchored)", path.0),
        None => "review (unanchored)".into(),
    }
}

fn markdown_comment(location: String, author: &str, body: &str) -> String {
    format!("### `{location}`\n\n**{author}**\n\n{}", body.trim())
}

fn display_code_row(state: &AppState) -> Option<usize> {
    match state.display_rows.get(state.display_cursor)? {
        DisplayRow::Diff { row } => Some(*row),
        DisplayRow::SplitDiff { left, right } => match state.split_focus {
            Some(SplitSide::Old) => *left,
            Some(SplitSide::New) => *right,
            None => right.or(*left),
        },
        _ => None,
    }
}

fn copy_side(state: &AppState, row_index: usize) -> Option<DiffSide> {
    let row = state.parsed_diff.as_ref()?.rows.get(row_index)?;
    match state.split_focus {
        Some(SplitSide::Old) if row.left.is_some() => Some(DiffSide::Left),
        Some(SplitSide::New) if row.right.is_some() => Some(DiffSide::Right),
        _ if row.right.is_some() => Some(DiffSide::Right),
        _ if row.left.is_some() => Some(DiffSide::Left),
        _ => None,
    }
}

fn hunk_side(state: &AppState, diff: &ParsedFileDiff, mut rows: Range<usize>) -> Option<DiffSide> {
    let has_left = rows
        .clone()
        .any(|index| diff.rows.get(index).is_some_and(|row| row.left.is_some()));
    let has_right = rows.any(|index| diff.rows.get(index).is_some_and(|row| row.right.is_some()));
    match state.split_focus {
        Some(SplitSide::Old) if has_left => Some(DiffSide::Left),
        Some(SplitSide::New) if has_right => Some(DiffSide::Right),
        _ if has_right => Some(DiffSide::Right),
        _ if has_left => Some(DiffSide::Left),
        _ => None,
    }
}

fn clean_rows(diff: &ParsedFileDiff, rows: Range<usize>, side: DiffSide) -> String {
    rows.filter_map(|index| {
        let row = diff.rows.get(index)?;
        let belongs = match side {
            DiffSide::Left => row.left.is_some(),
            DiffSide::Right => row.right.is_some(),
        };
        belongs
            .then(|| row.raw.strip_prefix([' ', '+', '-']).unwrap_or(&row.raw))
            .map(str::to_owned)
    })
    .collect::<Vec<_>>()
    .join("\n")
}
