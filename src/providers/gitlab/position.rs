use crate::domain::{DiffPosition, DiffSelection, DiffSide, RepoPath};

use super::wire::Position;

pub(super) fn position(value: &Position) -> Option<DiffPosition> {
    match (value.old_line, value.new_line) {
        (_, Some(line)) => Some(DiffPosition {
            path: RepoPath(value.new_path.clone()),
            side: DiffSide::Right,
            line,
            hunk: 0,
        }),
        (Some(line), None) => Some(DiffPosition {
            path: RepoPath(value.old_path.clone()),
            side: DiffSide::Left,
            line,
            hunk: 0,
        }),
        (None, None) => None,
    }
}

pub(super) fn selection(value: &Position) -> Option<DiffSelection> {
    position(value).map(|position| DiffSelection {
        start: position.clone(),
        end: position,
    })
}
