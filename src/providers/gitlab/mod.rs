mod client;
mod position;
mod wire;

use async_trait::async_trait;
use semver::Version;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use url::form_urlencoded::byte_serialize;

use crate::{
    context::DiscoveryInput,
    diff::MAX_PATCH_BYTES,
    domain::{
        ChangeRequestKey, ChangedFile, CommitOid, DraftComment, DraftId, FileStatus,
        PatchAvailability, ProviderCapabilities, ProviderKind, ProviderSnapshot, RepoPath,
        ReviewComment, ReviewThread, SubmitRequest, SubmitResult, Support, ThreadId,
    },
    process::CommandRunner,
};

use self::{
    client::GlabClient,
    position::{position, selection},
    wire::{Approvals, Blob, Diff, Discussion, DraftNote, MergeRequest, VersionInfo},
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
        let merge_request: MergeRequest = parse_json(
            &self
                .read_api(
                    &key.host,
                    api_args(&key.host, [root.as_str()]),
                    "load merge request",
                )
                .await?,
            "load merge request",
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

        let diffs_endpoint = format!("{root}/diffs?unidiff=true");
        let diffs: Vec<Diff> = parse_ndjson(
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
        )?;
        let drafts: Vec<DraftNote> = parse_json(
            &self
                .read_api(
                    &key.host,
                    api_args(&key.host, [format!("{root}/draft_notes").as_str()]),
                    "load draft notes",
                )
                .await?,
            "load draft notes",
        )?;
        let discussions: Vec<Discussion> = parse_ndjson(
            &self
                .read_api(
                    &key.host,
                    api_args(
                        &key.host,
                        [
                            "--paginate",
                            "--output",
                            "ndjson",
                            format!("{root}/discussions").as_str(),
                        ],
                    ),
                    "load discussions",
                )
                .await?,
            "load discussions",
        )?;
        let approvals: Approvals = parse_json(
            &self
                .read_api(
                    &key.host,
                    api_args(&key.host, [format!("{root}/approvals").as_str()]),
                    "load approvals",
                )
                .await?,
            "load approvals",
        )?;
        let version: VersionInfo = parse_json(
            &self
                .read_api(
                    &key.host,
                    api_args(&key.host, ["version"]),
                    "load GitLab version",
                )
                .await?,
            "load GitLab version",
        )?;

        let mut files = Vec::with_capacity(diffs.len());
        for diff in diffs {
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
            let blob = self
                .load_blob(&key.host, &project, blob_path, revision)
                .await?;
            files.push(ChangedFile {
                path: RepoPath(diff.new_path.clone()),
                previous_path: diff.renamed_file.then_some(RepoPath(diff.old_path)),
                status,
                additions: 0,
                deletions: 0,
                patch,
                base_blob: (status == FileStatus::Deleted).then_some(blob.clone()),
                head_blob: (status != FileStatus::Deleted).then_some(blob),
                remotely_reviewed: None,
            });
        }

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

    async fn create_draft(
        &self,
        _key: &ChangeRequestKey,
        _expected_head: &CommitOid,
        _input: NewDraftComment,
    ) -> Result<DraftComment, ProviderError> {
        Err(unsupported(
            "create draft",
            "GitLab writes are not connected yet",
        ))
    }

    async fn update_draft(
        &self,
        _key: &ChangeRequestKey,
        _id: &DraftId,
        _body: DraftBody,
    ) -> Result<DraftComment, ProviderError> {
        Err(unsupported(
            "update draft",
            "GitLab writes are not connected yet",
        ))
    }

    async fn delete_draft(
        &self,
        _key: &ChangeRequestKey,
        _id: &DraftId,
    ) -> Result<(), ProviderError> {
        Err(unsupported(
            "delete draft",
            "GitLab writes are not connected yet",
        ))
    }

    async fn reply(
        &self,
        _key: &ChangeRequestKey,
        _thread: &ThreadId,
        _body: DraftBody,
    ) -> Result<ReviewThread, ProviderError> {
        Err(unsupported("reply", "GitLab writes are not connected yet"))
    }

    async fn resolve_thread(
        &self,
        _key: &ChangeRequestKey,
        _thread: &ThreadId,
        _resolved: bool,
    ) -> Result<(), ProviderError> {
        Err(unsupported(
            "resolve thread",
            "GitLab writes are not connected yet",
        ))
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
        _key: &ChangeRequestKey,
        _request: SubmitRequest,
    ) -> Result<SubmitResult, ProviderError> {
        Err(unsupported(
            "submit review",
            "GitLab writes are not connected yet",
        ))
    }

    async fn discard_review(&self, _key: &ChangeRequestKey) -> Result<(), ProviderError> {
        Err(unsupported(
            "discard review",
            "GitLab writes are not connected yet",
        ))
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
