use serde::Serialize;

use crate::domain::{DiffPosition, DiffSelection, DiffSide, RepoPath};

use super::{
    super::ProviderError,
    wire::{DiffRefs, Position},
};

#[derive(Serialize)]
pub(super) struct GitLabPosition {
    position_type: &'static str,
    base_sha: String,
    start_sha: String,
    head_sha: String,
    old_path: String,
    new_path: String,
    old_line: Option<u32>,
    new_line: Option<u32>,
    line_range: Option<GitLabLineRange>,
}

#[derive(Serialize)]
struct GitLabLineRange {
    start: GitLabLineAnchor,
    end: GitLabLineAnchor,
}

#[derive(Serialize)]
struct GitLabLineAnchor {
    line_code: Option<String>,
    #[serde(rename = "type")]
    side: &'static str,
    old_line: Option<u32>,
    new_line: Option<u32>,
}

pub(super) fn write_position(
    selection: &DiffSelection,
    refs: &DiffRefs,
) -> Result<GitLabPosition, ProviderError> {
    if selection.start.side != selection.end.side || selection.start.path != selection.end.path {
        return Err(ProviderError::MalformedResponse {
            operation: "create draft".into(),
            message: "selection must stay on one path and side".into(),
        });
    }
    let side = selection.end.side;
    let anchor = |line| GitLabLineAnchor {
        line_code: None,
        side: side_name(side),
        old_line: (side == DiffSide::Left).then_some(line),
        new_line: (side == DiffSide::Right).then_some(line),
    };
    Ok(GitLabPosition {
        position_type: "text",
        base_sha: refs.base_sha.clone(),
        start_sha: refs.start_sha.clone(),
        head_sha: refs.head_sha.clone(),
        old_path: selection.end.path.0.clone(),
        new_path: selection.end.path.0.clone(),
        old_line: (side == DiffSide::Left).then_some(selection.end.line),
        new_line: (side == DiffSide::Right).then_some(selection.end.line),
        line_range: (selection.start != selection.end).then(|| GitLabLineRange {
            start: anchor(selection.start.line),
            end: anchor(selection.end.line),
        }),
    })
}

fn side_name(side: DiffSide) -> &'static str {
    match side {
        DiffSide::Left => "old",
        DiffSide::Right => "new",
    }
}

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
