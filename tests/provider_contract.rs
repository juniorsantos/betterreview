use async_trait::async_trait;
use betterreview::{
    context::DiscoveryInput,
    domain::{
        ChangeRequestKey, CommitOid, DraftComment, DraftId, ProviderKind, ProviderSnapshot,
        RepoPath, ReviewThread, SubmitRequest, SubmitResult, ThreadId,
    },
    providers::{DraftBody, NewDraftComment, ProviderError, ProviderRegistry, ReviewProvider},
};
use std::sync::Arc;

struct FakeProvider {
    kind: ProviderKind,
}

impl FakeProvider {
    fn new(kind: ProviderKind) -> Self {
        Self { kind }
    }
}

#[async_trait]
impl ReviewProvider for FakeProvider {
    fn kind(&self) -> ProviderKind {
        self.kind
    }

    async fn probe(&self, _host: &str) -> Result<(), ProviderError> {
        unimplemented!()
    }

    async fn discover(&self, _input: &DiscoveryInput) -> Result<ChangeRequestKey, ProviderError> {
        unimplemented!()
    }

    async fn load(&self, _key: &ChangeRequestKey) -> Result<ProviderSnapshot, ProviderError> {
        unimplemented!()
    }

    async fn read_head(&self, _key: &ChangeRequestKey) -> Result<CommitOid, ProviderError> {
        unimplemented!()
    }

    async fn create_draft(
        &self,
        _key: &ChangeRequestKey,
        _expected_head: &CommitOid,
        _input: NewDraftComment,
    ) -> Result<DraftComment, ProviderError> {
        unimplemented!()
    }

    async fn update_draft(
        &self,
        _key: &ChangeRequestKey,
        _id: &DraftId,
        _body: DraftBody,
    ) -> Result<DraftComment, ProviderError> {
        unimplemented!()
    }

    async fn delete_draft(
        &self,
        _key: &ChangeRequestKey,
        _id: &DraftId,
    ) -> Result<(), ProviderError> {
        unimplemented!()
    }

    async fn reply(
        &self,
        _key: &ChangeRequestKey,
        _thread: &ThreadId,
        _body: DraftBody,
    ) -> Result<ReviewThread, ProviderError> {
        unimplemented!()
    }

    async fn resolve_thread(
        &self,
        _key: &ChangeRequestKey,
        _thread: &ThreadId,
        _resolved: bool,
    ) -> Result<(), ProviderError> {
        unimplemented!()
    }

    async fn set_file_reviewed(
        &self,
        _key: &ChangeRequestKey,
        _path: &RepoPath,
        _reviewed: bool,
    ) -> Result<(), ProviderError> {
        unimplemented!()
    }

    async fn submit_review(
        &self,
        _key: &ChangeRequestKey,
        _request: SubmitRequest,
    ) -> Result<SubmitResult, ProviderError> {
        unimplemented!()
    }

    async fn discard_review(&self, _key: &ChangeRequestKey) -> Result<(), ProviderError> {
        unimplemented!()
    }
}

fn accepts_trait_object(_provider: Arc<dyn ReviewProvider>) {}

#[test]
fn registry_returns_the_requested_provider() {
    let github: Arc<dyn ReviewProvider> = Arc::new(FakeProvider::new(ProviderKind::GitHub));
    let gitlab: Arc<dyn ReviewProvider> = Arc::new(FakeProvider::new(ProviderKind::GitLab));
    accepts_trait_object(github.clone());
    let registry = ProviderRegistry::new(github, gitlab);
    assert_eq!(
        registry.get(ProviderKind::GitLab).kind(),
        ProviderKind::GitLab
    );
}
