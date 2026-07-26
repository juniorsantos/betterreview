use crate::{
    diff::{ParsedFileDiff, RenderedDiff},
    domain::{
        ChangedFile, CommitOid, DraftComment, DraftId, ProviderSnapshot, RepoPath, ReviewThread,
        SubmitRequest, SubmitResult, ThreadId,
    },
    providers::{DraftBody, NewDraftComment},
    state::SessionSnapshot,
};

#[derive(Debug, Clone)]
pub struct EffectEnvelope {
    pub id: u64,
    pub generation: Option<CommitOid>,
    pub effect: AppEffect,
}

#[derive(Debug, Clone)]
pub enum AppEffect {
    SaveConfig { config: crate::state::AppConfig },
    RenderActiveFile { file: ChangedFile, width: u16 },
    SaveSession { snapshot: Box<SessionSnapshot> },
    CreateDraft { input: NewDraftComment },
    UpdateDraft { id: DraftId, body: DraftBody },
    DeleteDraft { id: DraftId },
    Reply { thread: ThreadId, body: DraftBody },
    ResolveThread { thread: ThreadId, resolved: bool },
    SetFileReviewed { path: RepoPath, reviewed: bool },
    RefreshSnapshot,
    SubmitReview { request: SubmitRequest },
    DiscardReview,
    LoadFileContext { path: RepoPath, revision: CommitOid },
}

#[derive(Debug)]
pub struct EffectResult {
    pub id: u64,
    pub generation: Option<CommitOid>,
    pub outcome: EffectOutcome,
}

#[derive(Debug, Clone)]
pub struct RenderedFile {
    pub parsed: ParsedFileDiff,
    pub rendered: RenderedDiff,
}

#[derive(Debug)]
pub enum EffectOutcome {
    Rendered(Result<RenderedFile, String>),
    Saved(Result<(), String>),
    DraftCreated(Result<DraftComment, String>),
    DraftUpdated(Result<DraftComment, String>),
    ThreadUpdated(Result<ReviewThread, String>),
    SnapshotRefreshed(Box<Result<ProviderSnapshot, String>>),
    ReviewSubmitted(Result<SubmitResult, String>),
    FileReviewed {
        path: RepoPath,
        reviewed: bool,
        result: Result<(), String>,
    },
    DraftDeleted {
        id: DraftId,
        result: Result<(), String>,
    },
    Completed(Result<(), String>),
    FileContextLoaded {
        path: RepoPath,
        result: Result<String, String>,
    },
}
