use crate::domain::{DiffPosition, DiffSelection, DiffSide};

use super::{DiffRow, ParsedFileDiff};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffCursor {
    pub row: usize,
    pub side: DiffSide,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SelectionError {
    #[error("the selected row is not commentable")]
    NotCommentable,
    #[error("a range must stay on one diff side")]
    DifferentSides,
    #[error("a seleção precisa ficar dentro de um único hunk")]
    DifferentHunks,
    #[error("the selected range contains a row with no position on this side")]
    MissingSidePosition,
}

pub fn validate_selection(
    diff: &ParsedFileDiff,
    start: DiffCursor,
    end: DiffCursor,
) -> Result<DiffSelection, SelectionError> {
    if start.side != end.side {
        return Err(SelectionError::DifferentSides);
    }
    let (mut first_index, mut last_index) = if start.row <= end.row {
        (start.row, end.row)
    } else {
        (end.row, start.row)
    };
    // Trim edges that carry no position on the selected side (hunk headers
    // and metadata): landing the cursor on an `@@` boundary must shrink the
    // range, not reject the whole selection.
    while first_index < last_index
        && diff
            .rows
            .get(first_index)
            .is_some_and(|row| position(row, start.side).is_none())
    {
        first_index += 1;
    }
    while last_index > first_index
        && diff
            .rows
            .get(last_index)
            .is_some_and(|row| position(row, start.side).is_none())
    {
        last_index -= 1;
    }
    let first_row = diff
        .rows
        .get(first_index)
        .ok_or(SelectionError::NotCommentable)?;
    let last_row = diff
        .rows
        .get(last_index)
        .ok_or(SelectionError::NotCommentable)?;
    let first = position(first_row, start.side).ok_or(SelectionError::NotCommentable)?;
    let last = position(last_row, start.side).ok_or(SelectionError::NotCommentable)?;

    if first.hunk != last.hunk {
        return Err(SelectionError::DifferentHunks);
    }
    if diff.rows[first_index..=last_index]
        .iter()
        .any(|row| position(row, start.side).is_none())
    {
        return Err(SelectionError::MissingSidePosition);
    }

    Ok(DiffSelection {
        start: first.clone(),
        end: last.clone(),
    })
}

fn position(row: &DiffRow, side: DiffSide) -> Option<&DiffPosition> {
    match side {
        DiffSide::Left => row.left.as_ref(),
        DiffSide::Right => row.right.as_ref(),
    }
}
