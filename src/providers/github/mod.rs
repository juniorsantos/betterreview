mod client;
mod graphql;
mod mutations;
mod wire;

use async_trait::async_trait;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};

use crate::{
    context::DiscoveryInput,
    diff::MAX_PATCH_BYTES,
    domain::{
        ChangeRequestKey, ChangeRequestSummary, ChangedFile, CommitOid, DiffPosition,
        DiffSelection, DiffSide, DraftComment, DraftId, FileStatus, PatchAvailability,
        ProviderCapabilities, ProviderKind, ProviderSnapshot, RepoPath, ReviewComment,
        ReviewThread, SubmitRequest, SubmitResult, ThreadId,
    },
    process::CommandRunner,
};

use self::{
    client::GhClient,
    graphql::{DISCOVER_QUERY, LIST_OPEN_QUERY, SNAPSHOT_QUERY},
    wire::{
        GraphQlEnvelope, ListData, PullRequest, RestFile, ReviewThread as WireThread, SnapshotData,
    },
};
use super::{DraftBody, NewDraftComment, ProviderError, ReviewProvider};

pub struct GitHubProvider<R> {
    client: GhClient<R>,
}

impl<R> GitHubProvider<R>
where
    R: CommandRunner,
{
    pub fn new(runner: Arc<R>) -> Self {
        Self {
            client: GhClient::new(runner),
        }
    }

    async fn load_snapshot(
        &self,
        key: &ChangeRequestKey,
    ) -> Result<ProviderSnapshot, ProviderError> {
        repository_parts(&key.repository)?;
        // GitHub refuses the whole-PR raw diff above 20k lines (HTTP 406);
        // fall back to the per-file patches the files endpoint already carries.
        let ((metadata, wire_threads, viewed), rest_files, raw_diff) = tokio::try_join!(
            self.load_graphql_snapshot(key),
            self.load_rest_files(key),
            async { Ok(self.load_raw_diff(key).await.ok()) },
        )?;

        let patches = match &raw_diff {
            Some(raw) => split_patches(raw)?,
            None => BTreeMap::new(),
        };
        let mut files = Vec::with_capacity(rest_files.len());
        for rest in rest_files {
            let status = file_status(&rest.status)?;
            let from_raw = patches.get(&rest.filename).cloned();
            let candidate = match (from_raw, raw_diff.is_some()) {
                (Some(patch), _) => Some(patch),
                (None, false) => rest.patch.clone(),
                (None, true) => None,
            };
            let patch = candidate
                .map(|patch| {
                    if patch.len() > MAX_PATCH_BYTES {
                        PatchAvailability::TooLarge
                    } else {
                        PatchAvailability::Available(patch)
                    }
                })
                .unwrap_or_else(|| {
                    if rest.patch.is_none() {
                        PatchAvailability::Truncated {
                            reason: "GitHub omitted this file patch".into(),
                        }
                    } else {
                        PatchAvailability::Truncated {
                            reason: "raw diff did not contain this file".into(),
                        }
                    }
                });
            let path = RepoPath(rest.filename.clone());
            // GitHub's files endpoint reports the blob sha at head, except for
            // deleted files where it is the blob that was removed (the base).
            let (base_blob, head_blob) = if status == FileStatus::Deleted {
                (rest.sha, None)
            } else {
                (None, rest.sha)
            };
            files.push(ChangedFile {
                path,
                previous_path: rest.previous_filename.map(RepoPath),
                status,
                additions: rest.additions,
                deletions: rest.deletions,
                patch,
                base_blob,
                head_blob,
                remotely_reviewed: viewed.get(&rest.filename).copied().flatten(),
            });
        }

        let (threads, drafts) = map_threads(wire_threads);
        Ok(ProviderSnapshot {
            key: key.clone(),
            title: metadata.title,
            author: metadata
                .author
                .map_or_else(|| "unknown".into(), |author| author.login),
            web_url: metadata.url,
            base: CommitOid(metadata.base_ref_oid),
            head: CommitOid(metadata.head_ref_oid),
            files,
            threads,
            drafts,
            capabilities: ProviderCapabilities::all_supported(),
        })
    }

    async fn load_graphql_snapshot(
        &self,
        key: &ChangeRequestKey,
    ) -> Result<(PullRequest, Vec<WireThread>, BTreeMap<String, Option<bool>>), ProviderError> {
        let (owner, name) = repository_parts(&key.repository)?;
        let mut cursor: Option<String> = None;
        let mut metadata: Option<PullRequest> = None;
        let mut wire_threads = Vec::new();
        let mut viewed = BTreeMap::new();

        loop {
            let bytes = self
                .client
                .graphql(
                    &key.host,
                    SNAPSHOT_QUERY,
                    json!({
                        "owner": owner,
                        "name": name,
                        "number": key.number,
                        "cursor": cursor,
                    }),
                    "load pull request",
                )
                .await?;
            let envelope: GraphQlEnvelope<SnapshotData> = parse_json(&bytes, "load pull request")?;
            ensure_graphql(&envelope, "load pull request")?;
            let pull_request = envelope
                .data
                .and_then(|data| data.repository)
                .and_then(|repository| repository.pull_request)
                .ok_or_else(|| ProviderError::NotFound {
                    resource: format!("{}/pull/{}", key.repository, key.number),
                })?;
            for file in &pull_request.files.nodes {
                viewed.insert(
                    file.path.clone(),
                    match file.viewer_viewed_state.as_deref() {
                        Some("VIEWED") => Some(true),
                        Some("UNVIEWED") => Some(false),
                        _ => None,
                    },
                );
            }
            wire_threads.extend(pull_request.review_threads.nodes);
            let page_info = pull_request.review_threads.page_info;
            if metadata.is_none() {
                metadata = Some(PullRequest {
                    review_threads: Default::default(),
                    ..pull_request
                });
            }
            if !page_info.has_next_page {
                break;
            }
            cursor = page_info.end_cursor;
            if cursor.is_none() {
                return Err(malformed("load pull request", "missing pagination cursor"));
            }
        }

        let metadata =
            metadata.ok_or_else(|| malformed("load pull request", "missing metadata"))?;
        if metadata.number != key.number {
            return Err(malformed(
                "load pull request",
                "response number did not match the requested pull request",
            ));
        }
        Ok((metadata, wire_threads, viewed))
    }

    async fn load_rest_files(
        &self,
        key: &ChangeRequestKey,
    ) -> Result<Vec<RestFile>, ProviderError> {
        let endpoint = format!(
            "repos/{}/pulls/{}/files?per_page=100",
            key.repository, key.number
        );
        let file_bytes = self
            .client
            .api(
                &key.host,
                [
                    "api",
                    "--hostname",
                    key.host.as_str(),
                    "--paginate",
                    "--slurp",
                    endpoint.as_str(),
                ],
                "load pull request files",
            )
            .await?;
        let pages: Vec<Vec<RestFile>> = parse_json(&file_bytes, "load pull request files")?;
        Ok(pages.into_iter().flatten().collect())
    }

    async fn load_raw_diff(&self, key: &ChangeRequestKey) -> Result<Vec<u8>, ProviderError> {
        let diff_endpoint = format!("repos/{}/pulls/{}", key.repository, key.number);
        self.client
            .api(
                &key.host,
                [
                    "api",
                    "--hostname",
                    key.host.as_str(),
                    "-H",
                    "Accept:application/vnd.github.v3.diff",
                    diff_endpoint.as_str(),
                ],
                "load pull request diff",
            )
            .await
    }
}

#[async_trait]
impl<R> ReviewProvider for GitHubProvider<R>
where
    R: CommandRunner + 'static,
{
    fn kind(&self) -> ProviderKind {
        ProviderKind::GitHub
    }

    async fn probe(&self, host: &str) -> Result<(), ProviderError> {
        self.client
            .api(host, ["api", "--hostname", host, "user"], "probe GitHub")
            .await
            .map(|_| ())
    }

    async fn discover(&self, input: &DiscoveryInput) -> Result<ChangeRequestKey, ProviderError> {
        match input {
            DiscoveryInput::Exact(key) if key.provider == ProviderKind::GitHub => Ok(key.clone()),
            DiscoveryInput::CurrentBranch {
                provider: ProviderKind::GitHub,
                host,
                repository,
                branch,
            } => {
                let (owner, name) = repository_parts(repository)?;
                let bytes = self
                    .client
                    .graphql(
                        host,
                        DISCOVER_QUERY,
                        json!({ "owner": owner, "name": name, "head": branch }),
                        "discover pull request",
                    )
                    .await?;
                let value: Value = parse_json(&bytes, "discover pull request")?;
                let number = value["data"]["repository"]["pullRequests"]["nodes"]
                    .as_array()
                    .and_then(|nodes| nodes.first())
                    .and_then(|node| node["number"].as_u64())
                    .ok_or_else(|| ProviderError::NotFound {
                        resource: format!("pull request for {branch}"),
                    })?;
                Ok(ChangeRequestKey {
                    provider: ProviderKind::GitHub,
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
        let (owner, name) = repository_parts(repository)?;
        let bytes = self
            .client
            .graphql(
                host,
                LIST_OPEN_QUERY,
                json!({ "owner": owner, "name": name }),
                "list open pull requests",
            )
            .await?;
        let envelope: GraphQlEnvelope<ListData> = parse_json(&bytes, "list open pull requests")?;
        ensure_graphql(&envelope, "list open pull requests")?;
        let nodes = envelope
            .data
            .and_then(|data| data.repository)
            .map(|repository| repository.pull_requests.nodes)
            .unwrap_or_default();
        nodes
            .into_iter()
            .map(|node| {
                Ok(ChangeRequestSummary {
                    number: node.number,
                    title: node.title,
                    author: node
                        .author
                        .map_or_else(|| "unknown".into(), |author| author.login),
                    source_branch: node.head_ref_name,
                    updated_at: time::OffsetDateTime::parse(
                        &node.updated_at,
                        &time::format_description::well_known::Rfc3339,
                    )
                    .map_err(|error| malformed("list open pull requests", &error.to_string()))?,
                    draft: node.is_draft,
                    web_url: node.url,
                    description: node.body.unwrap_or_default(),
                })
            })
            .collect()
    }

    async fn load(&self, key: &ChangeRequestKey) -> Result<ProviderSnapshot, ProviderError> {
        self.load_snapshot(key).await
    }

    async fn read_head(&self, key: &ChangeRequestKey) -> Result<CommitOid, ProviderError> {
        let endpoint = format!("repos/{}/pulls/{}", key.repository, key.number);
        let bytes = self
            .client
            .api(
                &key.host,
                ["api", "--hostname", key.host.as_str(), endpoint.as_str()],
                "read pull request head",
            )
            .await?;
        let value: Value = parse_json(&bytes, "read pull request head")?;
        value["head"]["sha"]
            .as_str()
            .map(|sha| CommitOid(sha.into()))
            .ok_or_else(|| malformed("read pull request head", "missing head sha"))
    }

    async fn read_file(
        &self,
        key: &ChangeRequestKey,
        path: &RepoPath,
        revision: &CommitOid,
    ) -> Result<String, ProviderError> {
        let endpoint = format!(
            "repos/{}/contents/{}?ref={}",
            key.repository,
            encode_path(&path.0),
            utf8_percent_encode(&revision.0, PATH_SEGMENT)
        );
        let bytes = self
            .client
            .api(
                &key.host,
                [
                    "api",
                    "--hostname",
                    key.host.as_str(),
                    "-H",
                    "Accept:application/vnd.github.raw",
                    endpoint.as_str(),
                ],
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
        self.create_draft_comment(key, expected_head, input).await
    }

    async fn update_draft(
        &self,
        key: &ChangeRequestKey,
        id: &DraftId,
        body: DraftBody,
    ) -> Result<DraftComment, ProviderError> {
        self.update_draft_comment(key, id, body).await
    }

    async fn delete_draft(
        &self,
        key: &ChangeRequestKey,
        id: &DraftId,
    ) -> Result<(), ProviderError> {
        self.delete_draft_comment(key, id).await
    }

    async fn reply(
        &self,
        key: &ChangeRequestKey,
        thread: &ThreadId,
        body: DraftBody,
    ) -> Result<ReviewThread, ProviderError> {
        self.reply_to_thread(key, thread, body).await
    }

    async fn resolve_thread(
        &self,
        key: &ChangeRequestKey,
        thread: &ThreadId,
        resolved: bool,
    ) -> Result<(), ProviderError> {
        self.change_thread_resolution(key, thread, resolved).await
    }

    async fn set_file_reviewed(
        &self,
        key: &ChangeRequestKey,
        path: &RepoPath,
        reviewed: bool,
    ) -> Result<(), ProviderError> {
        self.change_file_reviewed(key, path, reviewed).await
    }

    async fn submit_review(
        &self,
        key: &ChangeRequestKey,
        request: SubmitRequest,
    ) -> Result<SubmitResult, ProviderError> {
        self.submit_pending_review(key, request).await
    }

    async fn discard_review(&self, key: &ChangeRequestKey) -> Result<(), ProviderError> {
        self.discard_pending_review(key).await
    }
}

/// Unreserved characters (RFC 3986) left unescaped so encoded paths stay
/// readable; everything else, including `/`, is percent-encoded.
const PATH_SEGMENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

/// Percent-encodes each `/`-separated segment of a repository path while
/// keeping the separators themselves literal, so the contents API receives a
/// valid nested path rather than one giant encoded blob.
fn encode_path(path: &str) -> String {
    path.split('/')
        .map(|segment| utf8_percent_encode(segment, PATH_SEGMENT).to_string())
        .collect::<Vec<_>>()
        .join("/")
}

fn repository_parts(repository: &str) -> Result<(&str, &str), ProviderError> {
    repository
        .split_once('/')
        .filter(|(owner, name)| !owner.is_empty() && !name.is_empty())
        .ok_or_else(|| malformed("repository", "expected owner/name"))
}

fn parse_json<T: DeserializeOwned>(bytes: &[u8], operation: &str) -> Result<T, ProviderError> {
    serde_json::from_slice(bytes).map_err(|error| malformed(operation, &error.to_string()))
}

fn ensure_graphql<T>(envelope: &GraphQlEnvelope<T>, operation: &str) -> Result<(), ProviderError> {
    if envelope.errors.is_empty() {
        Ok(())
    } else {
        Err(malformed(
            operation,
            &envelope
                .errors
                .iter()
                .map(|error| error.message.as_str())
                .collect::<Vec<_>>()
                .join("; "),
        ))
    }
}

fn split_patches(raw: &[u8]) -> Result<BTreeMap<String, String>, ProviderError> {
    let text = std::str::from_utf8(raw)
        .map_err(|error| malformed("load pull request diff", &error.to_string()))?;
    let mut patches = BTreeMap::new();
    let mut current_path = None;
    let mut current = String::new();
    for line in text.split_inclusive('\n') {
        if line.starts_with("diff --git ") {
            if let Some(path) = current_path.take() {
                patches.insert(path, std::mem::take(&mut current));
            }
            current_path = line
                .split_whitespace()
                .nth(3)
                .and_then(|path| path.strip_prefix("b/"))
                .map(str::to_owned);
        }
        current.push_str(line);
    }
    if let Some(path) = current_path {
        patches.insert(path, current);
    }
    Ok(patches)
}

fn file_status(status: &str) -> Result<FileStatus, ProviderError> {
    match status {
        "added" => Ok(FileStatus::Added),
        "modified" | "changed" => Ok(FileStatus::Modified),
        "removed" => Ok(FileStatus::Deleted),
        "renamed" => Ok(FileStatus::Renamed),
        "copied" => Ok(FileStatus::Copied),
        value => Err(malformed(
            "load pull request files",
            &format!("unknown status {value}"),
        )),
    }
}

fn map_threads(wire_threads: Vec<WireThread>) -> (Vec<ReviewThread>, Vec<DraftComment>) {
    let mut threads = Vec::with_capacity(wire_threads.len());
    let mut drafts = Vec::new();
    for thread in wire_threads {
        let path = RepoPath(thread.path.clone());
        let diff_side = thread.diff_side.clone();
        let mut comments = Vec::new();
        for comment in thread.comments.nodes {
            let position = comment_position(&path, &diff_side, &comment);
            let pending = comment
                .pull_request_review
                .as_ref()
                .is_some_and(|review| review.state == "PENDING")
                && comment.viewer_did_author;
            if pending {
                drafts.push(DraftComment {
                    id: DraftId(comment.id),
                    body: comment.body,
                    selection: position.clone().map(|position| DiffSelection {
                        start: position.clone(),
                        end: position,
                    }),
                    thread_id: Some(ThreadId(thread.id.clone())),
                });
            } else {
                comments.push(ReviewComment {
                    id: comment.id,
                    author: comment
                        .author
                        .map_or_else(|| "unknown".into(), |author| author.login),
                    body: comment.body,
                    position,
                    pending: false,
                });
            }
        }
        threads.push(ReviewThread {
            id: ThreadId(thread.id),
            path,
            resolved: thread.is_resolved,
            outdated: thread.is_outdated,
            comments,
        });
    }
    (threads, drafts)
}

fn comment_position(
    path: &RepoPath,
    diff_side: &str,
    comment: &wire::ReviewComment,
) -> Option<DiffPosition> {
    let side = match diff_side {
        "LEFT" => DiffSide::Left,
        "RIGHT" => DiffSide::Right,
        _ => return None,
    };
    let line = match side {
        DiffSide::Left => comment.original_line.or(comment.line)?,
        DiffSide::Right => comment.line.or(comment.original_line)?,
    };
    Some(DiffPosition {
        path: path.clone(),
        side,
        line,
        hunk: 0,
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
