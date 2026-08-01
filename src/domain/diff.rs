use serde::{Deserialize, Serialize};

use super::RepoPath;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffLayout {
    #[default]
    Auto,
    Unified,
    Split,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffSide {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffPosition {
    pub path: RepoPath,
    pub side: DiffSide,
    pub line: u32,
    pub hunk: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_line: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_line: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffSelection {
    pub start: DiffPosition,
    pub end: DiffPosition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangedFile {
    pub path: RepoPath,
    pub previous_path: Option<RepoPath>,
    pub status: FileStatus,
    pub additions: u32,
    pub deletions: u32,
    pub patch: PatchAvailability,
    pub base_blob: Option<String>,
    pub head_blob: Option<String>,
    pub remotely_reviewed: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "value", rename_all = "snake_case")]
pub enum PatchAvailability {
    Available(String),
    Binary,
    TooLarge,
    Collapsed,
    Truncated { reason: String },
}
