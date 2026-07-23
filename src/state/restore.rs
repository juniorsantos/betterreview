use std::collections::BTreeMap;

use crate::domain::{
    ChangedFile, CommitOid, PatchAvailability, ProviderKind, ProviderSnapshot, RepoPath,
};

use super::{ContentIdentity, FileProgress, ReviewSync, SessionSnapshot};

pub struct SessionRestorer;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoredSession {
    pub snapshot: SessionSnapshot,
    pub notices: Vec<RestoreNotice>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreNotice {
    HeadChanged { old: CommitOid, new: CommitOid },
    EditorBecameStale,
    FileReset { path: RepoPath },
    ReviewedSyncPending { path: RepoPath },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewedReconciliation {
    Restored,
    Remote,
    Pending,
    Reset,
}

impl SessionRestorer {
    pub fn restore(mut saved: SessionSnapshot, fresh: &ProviderSnapshot) -> RestoredSession {
        let same_head = saved.head == fresh.head;
        let mut notices = Vec::new();

        if same_head {
            reconcile_same_head(&mut saved, fresh, &mut notices);
            clamp_position(&mut saved, fresh);
        } else {
            notices.push(RestoreNotice::HeadChanged {
                old: saved.head.clone(),
                new: fresh.head.clone(),
            });
            reconcile_changed_head(&mut saved, fresh, &mut notices);
            saved.cursor_row = 0;
            saved.scroll_row = 0;
            if let Some(editor) = &mut saved.editor {
                editor.stale = true;
                notices.push(RestoreNotice::EditorBecameStale);
            }
        }

        saved.key = fresh.key.clone();
        saved.base = fresh.base.clone();
        saved.head = fresh.head.clone();
        ensure_active_file(&mut saved, fresh);

        RestoredSession {
            snapshot: saved,
            notices,
        }
    }
}

fn reconcile_same_head(
    saved: &mut SessionSnapshot,
    fresh: &ProviderSnapshot,
    notices: &mut Vec<RestoreNotice>,
) {
    let mut files = BTreeMap::new();
    for file in &fresh.files {
        let path = file.path.clone();
        let mut progress = saved
            .files
            .get(&path)
            .cloned()
            .unwrap_or_else(|| reset_progress(file, fresh.key.provider));
        progress.identity = identity(file);
        reconcile_remote(&path, &mut progress, file, fresh.key.provider, notices);
        files.insert(path, progress);
    }
    saved.files = files;
}

fn reconcile_changed_head(
    saved: &mut SessionSnapshot,
    fresh: &ProviderSnapshot,
    notices: &mut Vec<RestoreNotice>,
) {
    let mut files = BTreeMap::new();
    for file in &fresh.files {
        let path = file.path.clone();
        let previous = saved.files.get(&path);
        let matches = previous
            .map(|progress| identities_match(&progress.identity, file))
            .unwrap_or(false);
        let mut progress = if matches {
            previous.cloned().expect("matched progress exists")
        } else {
            notices.push(RestoreNotice::FileReset { path: path.clone() });
            reset_progress(file, fresh.key.provider)
        };
        progress.identity = identity(file);
        if matches {
            reconcile_remote(&path, &mut progress, file, fresh.key.provider, notices);
        }
        files.insert(path, progress);
    }
    saved.files = files;
}

fn reconcile_remote(
    path: &RepoPath,
    progress: &mut FileProgress,
    file: &ChangedFile,
    provider: ProviderKind,
    notices: &mut Vec<RestoreNotice>,
) -> ReviewedReconciliation {
    if provider == ProviderKind::GitLab && progress.sync == ReviewSync::LocalOnly {
        return ReviewedReconciliation::Restored;
    }
    match &progress.sync {
        ReviewSync::Synced => {
            if let Some(reviewed) = file.remotely_reviewed {
                progress.reviewed = reviewed;
                ReviewedReconciliation::Remote
            } else {
                ReviewedReconciliation::Restored
            }
        }
        ReviewSync::Pending { desired } => {
            progress.reviewed = *desired;
            notices.push(RestoreNotice::ReviewedSyncPending { path: path.clone() });
            ReviewedReconciliation::Pending
        }
        ReviewSync::Failed { desired, .. } => {
            progress.reviewed = *desired;
            ReviewedReconciliation::Pending
        }
        ReviewSync::LocalOnly => ReviewedReconciliation::Restored,
    }
}

fn identities_match(saved: &ContentIdentity, fresh: &ChangedFile) -> bool {
    // Providers know the blob on one side only (head, or base for deletions);
    // match on whatever evidence exists, never on its absence alone.
    saved.path == fresh.path
        && (fresh.base_blob.is_some() || fresh.head_blob.is_some())
        && saved.base_blob == fresh.base_blob
        && saved.head_blob == fresh.head_blob
}

fn reset_progress(file: &ChangedFile, provider: ProviderKind) -> FileProgress {
    FileProgress {
        identity: identity(file),
        reviewed: false,
        sync: match provider {
            ProviderKind::GitHub => ReviewSync::Synced,
            ProviderKind::GitLab => ReviewSync::LocalOnly,
        },
    }
}

fn identity(file: &ChangedFile) -> ContentIdentity {
    ContentIdentity {
        path: file.path.clone(),
        base_blob: file.base_blob.clone(),
        head_blob: file.head_blob.clone(),
    }
}

fn clamp_position(saved: &mut SessionSnapshot, fresh: &ProviderSnapshot) {
    let Some(active) = &saved.active_file else {
        saved.cursor_row = 0;
        saved.scroll_row = 0;
        return;
    };
    let Some(file) = fresh.files.iter().find(|file| &file.path == active) else {
        saved.cursor_row = 0;
        saved.scroll_row = 0;
        return;
    };
    let last_row = match &file.patch {
        PatchAvailability::Available(patch) => patch.lines().count().saturating_sub(1),
        _ => 0,
    };
    saved.cursor_row = saved.cursor_row.min(last_row);
    saved.scroll_row = saved.scroll_row.min(last_row);
}

fn ensure_active_file(saved: &mut SessionSnapshot, fresh: &ProviderSnapshot) {
    if saved
        .active_file
        .as_ref()
        .is_some_and(|path| saved.files.contains_key(path))
    {
        return;
    }
    saved.active_file = fresh.files.first().map(|file| file.path.clone());
}
