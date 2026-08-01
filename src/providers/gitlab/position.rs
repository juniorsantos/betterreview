use serde::Serialize;
use sha1::{Digest, Sha1};

use crate::domain::{DiffPosition, DiffSelection, DiffSide, RepoPath};

use super::{
    super::ProviderError,
    wire::{DiffRefs, LineAnchor, Position},
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

impl GitLabPosition {
    pub(super) fn form_fields(&self) -> Vec<(String, String)> {
        let mut fields = vec![
            ("position[position_type]".into(), self.position_type.into()),
            ("position[base_sha]".into(), self.base_sha.clone()),
            ("position[start_sha]".into(), self.start_sha.clone()),
            ("position[head_sha]".into(), self.head_sha.clone()),
            ("position[old_path]".into(), self.old_path.clone()),
            ("position[new_path]".into(), self.new_path.clone()),
        ];
        add_optional(&mut fields, "position[old_line]", self.old_line);
        add_optional(&mut fields, "position[new_line]", self.new_line);
        if let Some(line_range) = &self.line_range {
            add_anchor(&mut fields, "start", &line_range.start);
            add_anchor(&mut fields, "end", &line_range.end);
        }
        fields
    }
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
    let anchor = |position: &DiffPosition| {
        let (old_line, new_line) = coordinates(position);
        GitLabLineAnchor {
            line_code: Some(line_code(&position.path, old_line, new_line)),
            side: side_name(position.side),
            old_line,
            new_line,
        }
    };
    let (old_line, new_line) = coordinates(&selection.end);
    Ok(GitLabPosition {
        position_type: "text",
        base_sha: refs.base_sha.clone(),
        start_sha: refs.start_sha.clone(),
        head_sha: refs.head_sha.clone(),
        old_path: selection.end.path.0.clone(),
        new_path: selection.end.path.0.clone(),
        old_line,
        new_line,
        line_range: (selection.start != selection.end).then(|| GitLabLineRange {
            start: anchor(&selection.start),
            end: anchor(&selection.end),
        }),
    })
}

fn coordinates(position: &DiffPosition) -> (Option<u32>, Option<u32>) {
    match (position.old_line, position.new_line) {
        (None, None) if position.side == DiffSide::Left => (Some(position.line), None),
        (None, None) => (None, Some(position.line)),
        coordinates => coordinates,
    }
}

fn line_code(path: &RepoPath, old_line: Option<u32>, new_line: Option<u32>) -> String {
    let digest = Sha1::digest(path.0.as_bytes());
    format!(
        "{digest:x}_{}_{}",
        old_line.unwrap_or_else(|| new_line.unwrap_or_default()),
        new_line.unwrap_or_else(|| old_line.unwrap_or_default())
    )
}

fn side_name(side: DiffSide) -> &'static str {
    match side {
        DiffSide::Left => "old",
        DiffSide::Right => "new",
    }
}

fn add_optional(fields: &mut Vec<(String, String)>, key: &str, value: Option<u32>) {
    if let Some(value) = value {
        fields.push((key.into(), value.to_string()));
    }
}

fn add_anchor(fields: &mut Vec<(String, String)>, name: &str, anchor: &GitLabLineAnchor) {
    let prefix = format!("position[line_range][{name}]");
    if let Some(line_code) = &anchor.line_code {
        fields.push((format!("{prefix}[line_code]"), line_code.clone()));
    }
    fields.push((format!("{prefix}[type]"), anchor.side.into()));
}

pub(super) fn position(value: &Position) -> Option<DiffPosition> {
    match (value.old_line, value.new_line) {
        (_, Some(line)) => Some(DiffPosition {
            path: RepoPath(value.new_path.clone()),
            side: DiffSide::Right,
            line,
            hunk: 0,
            old_line: value.old_line,
            new_line: value.new_line,
        }),
        (Some(line), None) => Some(DiffPosition {
            path: RepoPath(value.old_path.clone()),
            side: DiffSide::Left,
            line,
            hunk: 0,
            old_line: value.old_line,
            new_line: value.new_line,
        }),
        (None, None) => {
            let range = value.line_range.as_ref()?;
            let anchor = range.end.as_ref().or(range.start.as_ref())?;
            let side = match anchor.side.as_deref() {
                Some("old") => DiffSide::Left,
                Some("new") => DiffSide::Right,
                _ => return None,
            };
            let line = anchor_line(anchor, side)?;
            Some(DiffPosition {
                path: match side {
                    DiffSide::Left => RepoPath(value.old_path.clone()),
                    DiffSide::Right => RepoPath(value.new_path.clone()),
                },
                side,
                line,
                hunk: 0,
                old_line: anchor.old_line,
                new_line: anchor.new_line,
            })
        }
    }
}

pub(super) fn selection(value: &Position) -> Option<DiffSelection> {
    let end = position(value)?;
    let first = value
        .line_range
        .as_ref()
        .and_then(|range| range.start.as_ref())
        .and_then(|anchor| anchor_line(anchor, end.side));
    let start = match first {
        Some(line) if line < end.line => DiffPosition {
            line,
            ..end.clone()
        },
        _ => end.clone(),
    };
    Some(DiffSelection { start, end })
}

fn anchor_line(anchor: &LineAnchor, side: DiffSide) -> Option<u32> {
    let line = match side {
        DiffSide::Left => anchor.old_line,
        DiffSide::Right => anchor.new_line,
    };
    line.or_else(|| line_code_line(anchor.line_code.as_deref()?, side))
}

fn line_code_line(line_code: &str, side: DiffSide) -> Option<u32> {
    let (prefix, new_line) = line_code.rsplit_once('_')?;
    let (_, old_line) = prefix.rsplit_once('_')?;
    let value = match side {
        DiffSide::Left => old_line,
        DiffSide::Right => new_line,
    }
    .parse::<u32>()
    .ok()?;
    (value > 0).then_some(value)
}
