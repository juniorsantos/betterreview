use serde::{Deserialize, Serialize};

use super::{
    ChangeRequestKey, ChangedFile, CommitOid, DiffPosition, DiffSelection, DraftId,
    ProviderCapabilities, RepoPath, ThreadId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewOutcome {
    Comment,
    Approve,
    RequestChanges,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftComment {
    pub id: DraftId,
    pub body: String,
    pub selection: Option<DiffSelection>,
    pub thread_id: Option<ThreadId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewComment {
    pub id: String,
    pub author: String,
    pub body: String,
    pub position: Option<DiffPosition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<DiffSelection>,
    pub pending: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewThread {
    pub id: ThreadId,
    pub path: RepoPath,
    pub resolved: bool,
    pub outdated: bool,
    pub comments: Vec<ReviewComment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeRequestSummary {
    pub number: u64,
    pub title: String,
    pub author: String,
    pub source_branch: String,
    pub updated_at: time::OffsetDateTime,
    pub draft: bool,
    pub web_url: String,
    pub description: String,
    pub head: CommitOid,
    pub reviewed_head: Option<CommitOid>,
}

impl ChangeRequestSummary {
    pub fn reviewed_current_head(&self) -> bool {
        self.reviewed_head.as_ref() == Some(&self.head)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderSnapshot {
    pub key: ChangeRequestKey,
    pub title: String,
    pub author: String,
    pub web_url: String,
    pub base: CommitOid,
    pub head: CommitOid,
    pub files: Vec<ChangedFile>,
    pub threads: Vec<ReviewThread>,
    pub drafts: Vec<DraftComment>,
    pub capabilities: ProviderCapabilities,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubmitMode {
    Full,
    OutcomeOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitRequest {
    pub expected_head: CommitOid,
    pub summary: String,
    pub outcome: ReviewOutcome,
    pub mode: SubmitMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SubmitResult {
    Complete,
    Partial {
        published_drafts: u32,
        retry: SubmitMode,
        reason: String,
    },
}
