use serde_json::Value;
use std::time::Duration;

use crate::{
    domain::{
        ChangeRequestKey, CommitOid, DraftComment, DraftId, RepoPath, ReviewComment, ReviewOutcome,
        ReviewThread, SubmitMode, SubmitRequest, SubmitResult, ThreadId,
    },
    process::{CommandError, CommandRunner},
};

use super::{
    super::{DraftBody, NewDraftComment, ProviderError},
    GitLabProvider, api_args, encode, parse_json,
    position::{selection, write_position},
    wire::{DraftNote, MergeRequest},
};

impl<R> GitLabProvider<R>
where
    R: CommandRunner + 'static,
{
    pub(super) async fn create_draft_note(
        &self,
        key: &ChangeRequestKey,
        expected_head: &CommitOid,
        input: NewDraftComment,
    ) -> Result<DraftComment, ProviderError> {
        let merge_request = self.write_context(key).await?;
        ensure_head(expected_head, &merge_request)?;
        let body = input
            .suggestion
            .as_deref()
            .map(suggestion_body)
            .unwrap_or(input.body.0);
        let position = write_position(&input.selection, &merge_request.diff_refs)?;
        let endpoint = format!("{}/draft_notes", merge_request_root(key));
        let mut fields = vec![("note".into(), body)];
        fields.extend(position.form_fields());
        let bytes = self
            .write_form(key, "POST", &endpoint, fields, "create draft")
            .await?;
        let draft: DraftNote = parse_json(&bytes, "create draft")?;
        Ok(map_draft(draft, Some(input.selection)))
    }

    pub(super) async fn update_draft_note(
        &self,
        key: &ChangeRequestKey,
        id: &DraftId,
        body: DraftBody,
    ) -> Result<DraftComment, ProviderError> {
        let endpoint = format!("{}/draft_notes/{}", merge_request_root(key), id.0);
        let bytes = self
            .write_form(
                key,
                "PUT",
                &endpoint,
                vec![("note".into(), body.0)],
                "update draft",
            )
            .await?;
        let draft: DraftNote = parse_json(&bytes, "update draft")?;
        Ok(map_draft(draft, None))
    }

    pub(super) async fn delete_draft_note(
        &self,
        key: &ChangeRequestKey,
        id: &DraftId,
    ) -> Result<(), ProviderError> {
        let endpoint = format!("{}/draft_notes/{}", merge_request_root(key), id.0);
        self.write_form(key, "DELETE", &endpoint, Vec::new(), "delete draft")
            .await
            .map(|_| ())
    }

    pub(super) async fn reply_to_discussion(
        &self,
        key: &ChangeRequestKey,
        thread: &ThreadId,
        body: DraftBody,
    ) -> Result<ReviewThread, ProviderError> {
        let endpoint = format!("{}/draft_notes", merge_request_root(key));
        let bytes = self
            .write_form(
                key,
                "POST",
                &endpoint,
                vec![
                    ("note".into(), body.0),
                    ("in_reply_to_discussion_id".into(), thread.0.clone()),
                ],
                "reply",
            )
            .await?;
        let draft: DraftNote = parse_json(&bytes, "reply")?;
        Ok(ReviewThread {
            id: thread.clone(),
            path: draft
                .position
                .as_ref()
                .map(|position| RepoPath(position.new_path.clone()))
                .unwrap_or_else(|| RepoPath(String::new())),
            resolved: false,
            outdated: false,
            comments: vec![ReviewComment {
                id: draft.id.to_string(),
                author: "viewer".into(),
                body: draft.note,
                position: draft.position.as_ref().and_then(super::position::position),
                selection: draft.position.as_ref().and_then(super::position::selection),
                pending: true,
            }],
        })
    }

    pub(super) async fn change_discussion_resolution(
        &self,
        key: &ChangeRequestKey,
        thread: &ThreadId,
        resolved: bool,
    ) -> Result<(), ProviderError> {
        let endpoint = format!("{}/discussions/{}", merge_request_root(key), thread.0);
        self.write_form(
            key,
            "PUT",
            &endpoint,
            vec![("resolved".into(), resolved.to_string())],
            if resolved {
                "resolve discussion"
            } else {
                "unresolve discussion"
            },
        )
        .await
        .map(|_| ())
    }

    pub(super) async fn discard_draft_notes(
        &self,
        key: &ChangeRequestKey,
    ) -> Result<(), ProviderError> {
        let root = merge_request_root(key);
        let endpoint = format!("{root}/draft_notes");
        let drafts: Vec<DraftNote> = parse_json(
            &self
                .read_api(
                    &key.host,
                    api_args(&key.host, [endpoint.as_str()]),
                    "load draft notes",
                )
                .await?,
            "load draft notes",
        )?;
        for draft in drafts {
            self.write_form(
                key,
                "DELETE",
                &format!("{root}/draft_notes/{}", draft.id),
                Vec::new(),
                "discard draft",
            )
            .await?;
        }
        Ok(())
    }

    pub(super) async fn submit_gitlab_review(
        &self,
        key: &ChangeRequestKey,
        request: SubmitRequest,
    ) -> Result<SubmitResult, ProviderError> {
        let merge_request = self.write_context(key).await?;
        ensure_head(&request.expected_head, &merge_request)?;
        let root = merge_request_root(key);
        let mut published_drafts = 0;

        if request.mode == SubmitMode::Full {
            let reviewer_state = match request.outcome {
                ReviewOutcome::Comment => None,
                ReviewOutcome::Approve => Some("reviewed"),
                ReviewOutcome::RequestChanges => Some("requested_changes"),
            };
            let mut fields = vec![("note".into(), request.summary)];
            if let Some(state) = reviewer_state {
                fields.push(("reviewer_state".into(), state.into()));
            }
            let bytes = self
                .write_form(
                    key,
                    "POST",
                    &format!("{root}/draft_notes/bulk_publish"),
                    fields,
                    "publish drafts",
                )
                .await?;
            let value = if bytes.iter().all(|byte| byte.is_ascii_whitespace()) {
                Value::Null
            } else {
                parse_json(&bytes, "publish drafts")?
            };
            published_drafts = value["published_drafts"]
                .as_u64()
                .and_then(|count| u32::try_from(count).ok())
                .unwrap_or(0);
        }

        if request.outcome != ReviewOutcome::Approve {
            return Ok(SubmitResult::Complete);
        }
        let approval = self
            .write_form(
                key,
                "POST",
                &format!("{root}/approve"),
                vec![("sha".into(), merge_request.diff_refs.head_sha)],
                "approve merge request",
            )
            .await;
        match approval {
            Ok(_) => Ok(SubmitResult::Complete),
            Err(error) if request.mode == SubmitMode::Full => Ok(SubmitResult::Partial {
                published_drafts,
                retry: SubmitMode::OutcomeOnly,
                reason: error.to_string(),
            }),
            Err(error) => Err(error),
        }
    }

    async fn write_context(&self, key: &ChangeRequestKey) -> Result<MergeRequest, ProviderError> {
        let endpoint = merge_request_root(key);
        parse_json(
            &self
                .read_api(
                    &key.host,
                    api_args(&key.host, [endpoint.as_str()]),
                    "load merge request write context",
                )
                .await?,
            "load merge request write context",
        )
    }

    async fn write_form(
        &self,
        key: &ChangeRequestKey,
        method: &str,
        endpoint: &str,
        fields: Vec<(String, String)>,
        operation: &str,
    ) -> Result<Vec<u8>, ProviderError> {
        let mut args = vec![
            "api".to_owned(),
            "--hostname".to_owned(),
            key.host.clone(),
            "-X".to_owned(),
            method.to_owned(),
        ];
        for (key, value) in fields {
            args.extend(["--form".to_owned(), format!("{key}={value}")]);
        }
        args.push(endpoint.to_owned());
        match self
            .client
            .api(args, None, operation, Duration::from_secs(120))
            .await
        {
            Err(ProviderError::Command(CommandError::Timeout { .. })) => {
                Err(ProviderError::AmbiguousWrite {
                    operation: operation.into(),
                    guidance: "the provider may have accepted the write; refresh before retrying"
                        .into(),
                })
            }
            result => result,
        }
    }
}

fn ensure_head(expected: &CommitOid, merge_request: &MergeRequest) -> Result<(), ProviderError> {
    let actual = CommitOid(merge_request.diff_refs.head_sha.clone());
    if &actual == expected {
        Ok(())
    } else {
        Err(ProviderError::StaleHead {
            expected: expected.clone(),
            actual,
        })
    }
}

fn merge_request_root(key: &ChangeRequestKey) -> String {
    format!(
        "projects/{}/merge_requests/{}",
        encode(&key.repository),
        key.number
    )
}

fn map_draft(draft: DraftNote, fallback: Option<crate::domain::DiffSelection>) -> DraftComment {
    let mapped = draft.position.as_ref().and_then(selection).or(fallback);
    DraftComment {
        id: DraftId(draft.id.to_string()),
        body: draft.note,
        selection: mapped,
        thread_id: None,
    }
}

fn suggestion_body(replacement: &str) -> String {
    format!("```suggestion\n{}\n```", replacement.trim_end_matches('\n'))
}
