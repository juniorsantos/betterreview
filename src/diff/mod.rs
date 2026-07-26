mod ansi;
mod delta;
mod parser;
mod selection;
mod split;

use std::ops::Range;

use crate::domain::DiffPosition;

pub use ansi::sanitize_ansi;
pub use delta::{DeltaError, DeltaRenderer, DiffRenderer, RenderedDiff, RenderedRow, RowBinding};
pub use parser::{count_hunks, parse_file_patch};
pub use selection::{DiffCursor, SelectionError, validate_selection};
pub use split::{SplitPair, pair_rows};

pub const MAX_PATCH_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffRowKind {
    Header,
    HunkHeader,
    Context,
    Added,
    Removed,
    Metadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffRow {
    pub raw: String,
    pub kind: DiffRowKind,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub left: Option<DiffPosition>,
    pub right: Option<DiffPosition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    pub id: u32,
    pub old_start: u32,
    pub old_count: u32,
    pub new_start: u32,
    pub new_count: u32,
    pub row_range: Range<usize>,
    pub section: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFileDiff {
    pub path: crate::domain::RepoPath,
    pub head: crate::domain::CommitOid,
    pub rows: Vec<DiffRow>,
    pub hunks: Vec<DiffHunk>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DiffError {
    #[error("the patch is unavailable: {reason}")]
    PatchUnavailable { reason: String },
    #[error("patch payload is {size} bytes; maximum is {maximum}")]
    PatchTooLarge { size: usize, maximum: usize },
    #[error("malformed hunk header: {line}")]
    MalformedHunk { line: String },
    #[error(
        "hunk {hunk} count mismatch: expected -{expected_old}/+{expected_new}, consumed -{actual_old}/+{actual_new}"
    )]
    HunkCountMismatch {
        hunk: u32,
        expected_old: u32,
        expected_new: u32,
        actual_old: u32,
        actual_new: u32,
    },
    #[error("diff content appeared outside a hunk: {line}")]
    ContentOutsideHunk { line: String },
    #[error("diff line counter overflowed")]
    LineOverflow,
}
