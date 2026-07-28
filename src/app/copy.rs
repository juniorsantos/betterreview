use std::ops::Range;

use crate::{
    diff::{DiffCursor, ParsedFileDiff, validate_selection},
    domain::DiffSide,
    tui::SplitSide,
};

use super::{AppState, DisplayRow};

pub(super) enum CopyTarget {
    LineOrSelection,
    Hunk,
}

pub(super) struct PreparedCopy {
    pub content: String,
    pub notice: &'static str,
}

pub(super) fn prepare(state: &AppState, target: CopyTarget) -> Result<PreparedCopy, String> {
    match target {
        CopyTarget::LineOrSelection => line_or_selection(state),
        CopyTarget::Hunk => hunk(state),
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
    let display_row = state.display_rows.get(state.display_cursor);
    let row_index = display_code_row(state);
    let hunk_id = match display_row {
        Some(DisplayRow::HunkHeader { hunk }) => Some(*hunk),
        _ => row_index.and_then(|index| {
            diff.rows
                .get(index)
                .and_then(|row| row.right.as_ref().or(row.left.as_ref()))
                .map(|position| position.hunk)
        }),
    };
    let hunk = hunk_id
        .and_then(|id| diff.hunks.iter().find(|hunk| hunk.id == id))
        .ok_or_else(|| "move to a hunk".to_owned())?;
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
