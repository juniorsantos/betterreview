use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::domain::{
    ChangeRequestKey, CommitOid, DiffSelection, RepoPath, ReviewOutcome, SubmitMode,
};

pub const SESSION_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub schema_version: u32,
    pub key: ChangeRequestKey,
    pub base: CommitOid,
    pub head: CommitOid,
    pub active_file: Option<RepoPath>,
    pub cursor_row: usize,
    pub scroll_row: usize,
    pub files: BTreeMap<RepoPath, FileProgress>,
    pub editor: Option<EditorSnapshot>,
    pub pending_submit: Option<PendingSubmit>,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSummary {
    pub key: ChangeRequestKey,
    pub head: CommitOid,
    pub updated_at: OffsetDateTime,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentIdentity {
    pub path: RepoPath,
    pub base_blob: Option<String>,
    pub head_blob: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileProgress {
    pub identity: ContentIdentity,
    pub reviewed: bool,
    #[serde(default)]
    pub reviewed_hunks: BTreeSet<u32>,
    pub sync: ReviewSync,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ReviewSync {
    Synced,
    Pending { desired: bool },
    LocalOnly,
    Failed { desired: bool, message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorSnapshot {
    pub lines: Vec<String>,
    pub cursor_row: usize,
    pub grapheme_col: usize,
    pub original_head: CommitOid,
    pub path: RepoPath,
    pub selection: DiffSelection,
    pub stale: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingSubmit {
    pub summary: String,
    pub outcome: ReviewOutcome,
    pub mode: SubmitMode,
}
