use serde::{Deserialize, Serialize};

use super::ReviewOutcome;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Support {
    Supported,
    Unsupported { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    pub create_draft: Support,
    pub edit_draft: Support,
    pub delete_draft: Support,
    pub reply: Support,
    pub resolve_thread: Support,
    pub suggestion: Support,
    pub mark_file_reviewed: Support,
    pub comment: Support,
    pub approve: Support,
    pub request_changes: Support,
}

impl ProviderCapabilities {
    pub fn all_supported() -> Self {
        Self {
            create_draft: Support::Supported,
            edit_draft: Support::Supported,
            delete_draft: Support::Supported,
            reply: Support::Supported,
            resolve_thread: Support::Supported,
            suggestion: Support::Supported,
            mark_file_reviewed: Support::Supported,
            comment: Support::Supported,
            approve: Support::Supported,
            request_changes: Support::Supported,
        }
    }

    pub fn with_request_changes(mut self, support: Support) -> Self {
        self.request_changes = support;
        self
    }

    pub fn for_outcome(&self, outcome: ReviewOutcome) -> &Support {
        match outcome {
            ReviewOutcome::Comment => &self.comment,
            ReviewOutcome::Approve => &self.approve,
            ReviewOutcome::RequestChanges => &self.request_changes,
        }
    }
}
