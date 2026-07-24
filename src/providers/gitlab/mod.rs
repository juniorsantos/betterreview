mod client;
mod mutations;
mod position;
mod wire;

use async_trait::async_trait;
use futures_util::{StreamExt, TryStreamExt, stream};
use semver::Version;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use url::form_urlencoded::byte_serialize;

/// How many repository blob lookups may run at once.
const BLOB_CONCURRENCY: usize = 8;

use crate::{
    context::DiscoveryInput,
    diff::MAX_PATCH_BYTES,
    domain::{
        ChangeRequestKey, ChangeRequestSummary, ChangedFile, CommitOid, DraftComment, DraftId,
        FileStatus, PatchAvailability, ProviderCapabilities, ProviderKind, ProviderSnapshot,
        RepoPath, ReviewComment, ReviewThread, SubmitRequest, SubmitResult, Support, ThreadId,
    },
    process::CommandRunner,
};

use self::{
    client::GlabClient,
    position::{position, selection},
    wire::{
        Approvals, Blob, Diff, Discussion, DraftNote, MergeRequest, MergeRequestSummary,
        VersionInfo,
    },
};
use super::{DraftBody, NewDraftComment, ProviderError, ReviewProvider};

pub struct GitLabProvider<R> {
    client: GlabClient<R>,
}

impl<R> GitLabProvider<R>
where
    R: CommandRunner,
{
    pub fn new(runner: Arc<R>) -> Self {
        Self {
            client: GlabClient::new(runner),
        }
    }

    async fn read_api(
        &self,
        host: &str,
        args: Vec<String>,
        operation: &str,
    ) -> Result<Vec<u8>, ProviderError> {
        self.client
            .api(args, None, operation, Duration::from_secs(60))
            .await
            .map_err(|error| match error {
                ProviderError::Authentication { guidance } => ProviderError::Authentication {
                    guidance: if guidance.is_empty() {
                        format!("Run: glab auth login --hostname {host}")
                    } else {
                        guidance
                    },
                },
                error => error,
            })
    }

    async fn load_snapshot(
        &self,
        key: &ChangeRequestKey,
    ) -> Result<ProviderSnapshot, ProviderError> {
        let project = encode(&key.repository);
        let root = format!("projects/{project}/merge_requests/{}", key.number);
        let diffs_endpoint = format!("{root}/diffs?unidiff=true&per_page=100");
        let drafts_endpoint = format!("{root}/draft_notes");
        let discussions_endpoint = format!("{root}/discussions?per_page=100");
        let approvals_endpoint = format!("{root}/approvals");
        let (merge_request, diffs, drafts, discussions, approvals, version) = tokio::try_join!(
            async {
                parse_json::<MergeRequest>(
                    &self
                        .read_api(
                            &key.host,
                            api_args(&key.host, [root.as_str()]),
                            "load merge request",
                        )
                        .await?,
                    "load merge request",
                )
            },
            async {
                parse_ndjson::<Diff>(
                    &self
                        .read_api(
                            &key.host,
                            api_args(
                                &key.host,
                                ["--paginate", "--output", "ndjson", diffs_endpoint.as_str()],
                            ),
                            "load merge request diffs",
                        )
                        .await?,
                    "load merge request diffs",
                )
            },
            async {
                parse_json::<Vec<DraftNote>>(
                    &self
                        .read_api(
                            &key.host,
                            api_args(&key.host, [drafts_endpoint.as_str()]),
                            "load draft notes",
                        )
                        .await?,
                    "load draft notes",
                )
            },
            async {
                parse_ndjson::<Discussion>(
                    &self
                        .read_api(
                            &key.host,
                            api_args(
                                &key.host,
                                [
                                    "--paginate",
                                    "--output",
                                    "ndjson",
                                    discussions_endpoint.as_str(),
                                ],
                            ),
                            "load discussions",
                        )
                        .await?,
                    "load discussions",
                )
            },
            async {
                parse_json::<Approvals>(
                    &self
                        .read_api(
                            &key.host,
                            api_args(&key.host, [approvals_endpoint.as_str()]),
                            "load approvals",
                        )
                        .await?,
                    "load approvals",
                )
            },
            async {
                parse_json::<VersionInfo>(
                    &self
                        .read_api(
                            &key.host,
                            api_args(&key.host, ["version"]),
                            "load GitLab version",
                        )
                        .await?,
                    "load GitLab version",
                )
            },
        )?;
        if merge_request.iid != key.number {
            return Err(malformed(
                "load merge request",
                "response iid did not match request",
            ));
        }
        if merge_request.diff_refs.start_sha.is_empty() {
            return Err(malformed(
                "load merge request",
                "diff_refs.start_sha was empty",
            ));
        }

        let files: Vec<ChangedFile> = stream::iter(
            diffs
                .into_iter()
                .map(|diff| self.changed_file(&key.host, &project, &merge_request, diff)),
        )
        .buffered(BLOB_CONCURRENCY)
        .try_collect()
        .await?;

        let draft_comments = drafts
            .into_iter()
            .map(|draft| DraftComment {
                id: DraftId(draft.id.to_string()),
                body: draft.note,
                selection: draft.position.as_ref().and_then(selection),
                thread_id: None,
            })
            .collect();
        let threads = discussions.into_iter().map(map_discussion).collect();
        let capabilities = capabilities(&version, &approvals)?;

        Ok(ProviderSnapshot {
            key: key.clone(),
            title: merge_request.title,
            author: merge_request.author.username,
            web_url: merge_request.web_url,
            base: CommitOid(merge_request.diff_refs.base_sha),
            head: CommitOid(merge_request.diff_refs.head_sha),
            files,
            threads,
            drafts: draft_comments,
            capabilities,
        })
    }

    async fn changed_file(
        &self,
        host: &str,
        project: &str,
        merge_request: &MergeRequest,
        diff: Diff,
    ) -> Result<ChangedFile, ProviderError> {
        let status = if diff.new_file {
            FileStatus::Added
        } else if diff.deleted_file {
            FileStatus::Deleted
        } else if diff.renamed_file {
            FileStatus::Renamed
        } else {
            FileStatus::Modified
        };
        let patch = if diff.too_large {
            PatchAvailability::TooLarge
        } else if diff.collapsed {
            PatchAvailability::Collapsed
        } else {
            match diff.diff {
                Some(patch) if patch.len() <= MAX_PATCH_BYTES => {
                    PatchAvailability::Available(patch)
                }
                Some(_) => PatchAvailability::TooLarge,
                None => PatchAvailability::Truncated {
                    reason: "GitLab omitted this file diff".into(),
                },
            }
        };
        let blob_path = if status == FileStatus::Deleted {
            &diff.old_path
        } else {
            &diff.new_path
        };
        let revision = if status == FileStatus::Deleted {
            &merge_request.diff_refs.base_sha
        } else {
            &merge_request.diff_refs.head_sha
        };
        let blob = self.load_blob(host, project, blob_path, revision).await?;
        Ok(ChangedFile {
            path: RepoPath(diff.new_path.clone()),
            previous_path: diff.renamed_file.then_some(RepoPath(diff.old_path)),
            status,
            additions: 0,
            deletions: 0,
            patch,
            base_blob: (status == FileStatus::Deleted).then_some(blob.clone()),
            head_blob: (status != FileStatus::Deleted).then_some(blob),
            remotely_reviewed: None,
        })
    }

    async fn load_blob(
        &self,
        host: &str,
        project: &str,
        path: &str,
        revision: &str,
    ) -> Result<String, ProviderError> {
        let endpoint = format!(
            "projects/{project}/repository/files/{}?ref={}",
            encode(path),
            encode(revision)
        );
        let blob: Blob = parse_json(
            &self
                .read_api(
                    host,
                    api_args(host, [endpoint.as_str()]),
                    "load repository file",
                )
                .await?,
            "load repository file",
        )?;
        Ok(blob.blob_id)
    }
}

#[async_trait]
impl<R> ReviewProvider for GitLabProvider<R>
where
    R: CommandRunner + 'static,
{
    fn kind(&self) -> ProviderKind {
        ProviderKind::GitLab
    }

    async fn probe(&self, host: &str) -> Result<(), ProviderError> {
        self.read_api(host, api_args(host, ["user"]), "probe GitLab")
            .await
            .map(|_| ())
    }

    async fn discover(&self, input: &DiscoveryInput) -> Result<ChangeRequestKey, ProviderError> {
        match input {
            DiscoveryInput::Exact(key) if key.provider == ProviderKind::GitLab => Ok(key.clone()),
            DiscoveryInput::CurrentBranch {
                provider: ProviderKind::GitLab,
                host,
                repository,
                branch,
            } => {
                let endpoint = format!(
                    "projects/{}/merge_requests?state=opened&source_branch={}",
                    encode(repository),
                    encode(branch)
                );
                let value: Value = parse_json(
                    &self
                        .read_api(
                            host,
                            api_args(host, [endpoint.as_str()]),
                            "discover merge request",
                        )
                        .await?,
                    "discover merge request",
                )?;
                let number = value
                    .as_array()
                    .and_then(|items| items.first())
                    .and_then(|item| item["iid"].as_u64())
                    .ok_or_else(|| ProviderError::NotFound {
                        resource: format!("merge request for {branch}"),
                    })?;
                Ok(ChangeRequestKey {
                    provider: ProviderKind::GitLab,
                    host: host.clone(),
                    repository: repository.clone(),
                    number,
                })
            }
            _ => Err(unsupported("discover", "input belongs to another provider")),
        }
    }

    async fn list_open(
        &self,
        host: &str,
        repository: &str,
    ) -> Result<Vec<ChangeRequestSummary>, ProviderError> {
        let project = encode(repository);
        let endpoint = format!(
            "projects/{project}/merge_requests?state=opened&order_by=updated_at&sort=desc&per_page=50"
        );
        let summaries: Vec<MergeRequestSummary> = parse_json(
            &self
                .read_api(
                    host,
                    api_args(host, [endpoint.as_str()]),
                    "list open merge requests",
                )
                .await?,
            "list open merge requests",
        )?;
        summaries
            .into_iter()
            .map(|summary| {
                Ok(ChangeRequestSummary {
                    number: summary.iid,
                    title: summary.title,
                    author: summary.author.username,
                    source_branch: summary.source_branch,
                    updated_at: time::OffsetDateTime::parse(
                        &summary.updated_at,
                        &time::format_description::well_known::Rfc3339,
                    )
                    .map_err(|error| malformed("list open merge requests", &error.to_string()))?,
                    draft: summary.draft,
                    web_url: summary.web_url,
                    description: summary.description.unwrap_or_default(),
                })
            })
            .collect()
    }

    async fn load(&self, key: &ChangeRequestKey) -> Result<ProviderSnapshot, ProviderError> {
        self.load_snapshot(key).await
    }

    async fn read_head(&self, key: &ChangeRequestKey) -> Result<CommitOid, ProviderError> {
        let endpoint = format!(
            "projects/{}/merge_requests/{}",
            encode(&key.repository),
            key.number
        );
        let merge_request: MergeRequest = parse_json(
            &self
                .read_api(
                    &key.host,
                    api_args(&key.host, [endpoint.as_str()]),
                    "read merge request head",
                )
                .await?,
            "read merge request head",
        )?;
        Ok(CommitOid(merge_request.diff_refs.head_sha))
    }

    async fn read_file(
        &self,
        key: &ChangeRequestKey,
        path: &RepoPath,
        revision: &CommitOid,
    ) -> Result<String, ProviderError> {
        let project = encode(&key.repository);
        let endpoint = format!(
            "projects/{project}/repository/files/{}/raw?ref={}",
            encode(&path.0),
            encode(&revision.0)
        );
        let bytes = self
            .read_api(
                &key.host,
                api_args(&key.host, [endpoint.as_str()]),
                "read file contents",
            )
            .await?;
        String::from_utf8(bytes)
            .map_err(|error| malformed("read file contents", &error.to_string()))
    }

    async fn create_draft(
        &self,
        key: &ChangeRequestKey,
        expected_head: &CommitOid,
        input: NewDraftComment,
    ) -> Result<DraftComment, ProviderError> {
        self.create_draft_note(key, expected_head, input).await
    }

    async fn update_draft(
        &self,
        key: &ChangeRequestKey,
        id: &DraftId,
        body: DraftBody,
    ) -> Result<DraftComment, ProviderError> {
        self.update_draft_note(key, id, body).await
    }

    async fn delete_draft(
        &self,
        key: &ChangeRequestKey,
        id: &DraftId,
    ) -> Result<(), ProviderError> {
        self.delete_draft_note(key, id).await
    }

    async fn reply(
        &self,
        key: &ChangeRequestKey,
        thread: &ThreadId,
        body: DraftBody,
    ) -> Result<ReviewThread, ProviderError> {
        self.reply_to_discussion(key, thread, body).await
    }

    async fn resolve_thread(
        &self,
        key: &ChangeRequestKey,
        thread: &ThreadId,
        resolved: bool,
    ) -> Result<(), ProviderError> {
        self.change_discussion_resolution(key, thread, resolved)
            .await
    }

    async fn set_file_reviewed(
        &self,
        _key: &ChangeRequestKey,
        _path: &RepoPath,
        _reviewed: bool,
    ) -> Result<(), ProviderError> {
        Err(unsupported(
            "set file reviewed",
            "This GitLab instance has no file-viewed API; progress is stored locally",
        ))
    }

    async fn submit_review(
        &self,
        key: &ChangeRequestKey,
        request: SubmitRequest,
    ) -> Result<SubmitResult, ProviderError> {
        self.submit_gitlab_review(key, request).await
    }

    async fn discard_review(&self, key: &ChangeRequestKey) -> Result<(), ProviderError> {
        self.discard_draft_notes(key).await
    }
}

fn api_args<'a, I>(host: &'a str, tail: I) -> Vec<String>
where
    I: IntoIterator<Item = &'a str>,
{
    ["api", "--hostname", host]
        .into_iter()
        .chain(tail)
        .map(str::to_owned)
        .collect()
}

fn encode(value: &str) -> String {
    byte_serialize(value.as_bytes()).collect()
}

fn parse_json<T: DeserializeOwned>(bytes: &[u8], operation: &str) -> Result<T, ProviderError> {
    serde_json::from_slice(bytes).map_err(|error| malformed(operation, &error.to_string()))
}

fn parse_ndjson<T: DeserializeOwned>(
    bytes: &[u8],
    operation: &str,
) -> Result<Vec<T>, ProviderError> {
    let text =
        std::str::from_utf8(bytes).map_err(|error| malformed(operation, &error.to_string()))?;
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line).map_err(|error| malformed(operation, &error.to_string()))
        })
        .collect()
}

fn map_discussion(discussion: Discussion) -> ReviewThread {
    let resolved = discussion.resolved || discussion.notes.iter().any(|note| note.resolved);
    let path = discussion
        .notes
        .iter()
        .find_map(|note| note.position.as_ref())
        .map(|position| {
            if position.new_line.is_some() {
                RepoPath(position.new_path.clone())
            } else {
                RepoPath(position.old_path.clone())
            }
        })
        .unwrap_or_else(|| RepoPath(String::new()));
    ReviewThread {
        id: ThreadId(discussion.id),
        path,
        resolved,
        outdated: false,
        comments: discussion
            .notes
            .into_iter()
            .map(|note| ReviewComment {
                id: note.id.to_string(),
                author: note.author.username,
                body: note.body,
                position: note.position.as_ref().and_then(position),
                pending: false,
            })
            .collect(),
    }
}

fn capabilities(
    version: &VersionInfo,
    _approvals: &Approvals,
) -> Result<ProviderCapabilities, ProviderError> {
    let parsed = Version::parse(&version.version)
        .map_err(|error| malformed("load GitLab version", &error.to_string()))?;
    let request_changes = if parsed < Version::new(17, 3, 0) {
        Support::Unsupported {
            reason: "GitLab request changes requires version 17.3 or newer".into(),
        }
    } else if version
        .tier
        .as_deref()
        .is_some_and(|tier| tier.eq_ignore_ascii_case("free"))
    {
        Support::Unsupported {
            reason: "GitLab request changes requires Premium or Ultimate".into(),
        }
    } else {
        Support::Supported
    };
    Ok(ProviderCapabilities {
        create_draft: Support::Supported,
        edit_draft: Support::Supported,
        delete_draft: Support::Supported,
        reply: Support::Supported,
        resolve_thread: Support::Supported,
        suggestion: Support::Supported,
        mark_file_reviewed: Support::Unsupported {
            reason: "This GitLab instance has no file-viewed API; progress is stored locally"
                .into(),
        },
        comment: Support::Supported,
        approve: Support::Supported,
        request_changes,
    })
}

fn malformed(operation: &str, message: &str) -> ProviderError {
    ProviderError::MalformedResponse {
        operation: operation.into(),
        message: message.into(),
    }
}

fn unsupported(operation: &str, reason: &str) -> ProviderError {
    ProviderError::Unsupported {
        operation: operation.into(),
        reason: reason.into(),
    }
}
