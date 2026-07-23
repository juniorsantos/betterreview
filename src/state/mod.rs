mod json_store;
mod model;
mod paths;
mod restore;

pub use json_store::{JsonSessionStore, SessionAccess, SessionHandle, SessionStore, StateError};
pub use model::{
    ContentIdentity, EditorSnapshot, FileProgress, PendingSubmit, ReviewSync,
    SESSION_SCHEMA_VERSION, SessionSnapshot, SessionSummary,
};
pub use paths::StatePaths;
pub use restore::{RestoreNotice, RestoredSession, ReviewedReconciliation, SessionRestorer};
