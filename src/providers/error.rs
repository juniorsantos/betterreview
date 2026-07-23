#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("provider command failed: {0}")]
    Command(#[from] crate::process::CommandError),
    #[error("provider authentication failed: {guidance}")]
    Authentication { guidance: String },
    #[error("provider denied {operation}: {message}")]
    Permission { operation: String, message: String },
    #[error("provider does not support {operation}: {reason}")]
    Unsupported { operation: String, reason: String },
    #[error("provider returned malformed data for {operation}: {message}")]
    MalformedResponse { operation: String, message: String },
    #[error("provider resource not found: {resource}")]
    NotFound { resource: String },
    #[error("change-request head changed from {expected:?} to {actual:?}")]
    StaleHead {
        expected: crate::domain::CommitOid,
        actual: crate::domain::CommitOid,
    },
    #[error("the result of {operation} is ambiguous: {guidance}")]
    AmbiguousWrite { operation: String, guidance: String },
    #[error("review was partially submitted after publishing {published_drafts} drafts: {reason}")]
    PartialFailure {
        published_drafts: u32,
        retry: crate::domain::SubmitMode,
        reason: String,
    },
}
