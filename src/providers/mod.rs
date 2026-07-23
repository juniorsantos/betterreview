mod error;
mod github;
mod registry;

use async_trait::async_trait;

pub use error::ProviderError;
pub use github::GitHubProvider;
pub use registry::ProviderRegistry;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewDraftComment {
    pub body: DraftBody,
    pub selection: crate::domain::DiffSelection,
    pub suggestion: Option<String>,
    pub operation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftBody(pub String);

#[async_trait]
pub trait ReviewProvider: Send + Sync {
    fn kind(&self) -> crate::domain::ProviderKind;
    async fn probe(&self, host: &str) -> Result<(), ProviderError>;
    async fn discover(
        &self,
        input: &crate::context::DiscoveryInput,
    ) -> Result<crate::domain::ChangeRequestKey, ProviderError>;
    async fn load(
        &self,
        key: &crate::domain::ChangeRequestKey,
    ) -> Result<crate::domain::ProviderSnapshot, ProviderError>;
    async fn read_head(
        &self,
        key: &crate::domain::ChangeRequestKey,
    ) -> Result<crate::domain::CommitOid, ProviderError>;
    async fn create_draft(
        &self,
        key: &crate::domain::ChangeRequestKey,
        expected_head: &crate::domain::CommitOid,
        input: NewDraftComment,
    ) -> Result<crate::domain::DraftComment, ProviderError>;
    async fn update_draft(
        &self,
        key: &crate::domain::ChangeRequestKey,
        id: &crate::domain::DraftId,
        body: DraftBody,
    ) -> Result<crate::domain::DraftComment, ProviderError>;
    async fn delete_draft(
        &self,
        key: &crate::domain::ChangeRequestKey,
        id: &crate::domain::DraftId,
    ) -> Result<(), ProviderError>;
    async fn reply(
        &self,
        key: &crate::domain::ChangeRequestKey,
        thread: &crate::domain::ThreadId,
        body: DraftBody,
    ) -> Result<crate::domain::ReviewThread, ProviderError>;
    async fn resolve_thread(
        &self,
        key: &crate::domain::ChangeRequestKey,
        thread: &crate::domain::ThreadId,
        resolved: bool,
    ) -> Result<(), ProviderError>;
    async fn set_file_reviewed(
        &self,
        key: &crate::domain::ChangeRequestKey,
        path: &crate::domain::RepoPath,
        reviewed: bool,
    ) -> Result<(), ProviderError>;
    async fn submit_review(
        &self,
        key: &crate::domain::ChangeRequestKey,
        request: crate::domain::SubmitRequest,
    ) -> Result<crate::domain::SubmitResult, ProviderError>;
    async fn discard_review(
        &self,
        key: &crate::domain::ChangeRequestKey,
    ) -> Result<(), ProviderError>;
}
