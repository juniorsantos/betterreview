use serde_json::{Map, Value, json};

use crate::{
    domain::{
        ChangeRequestKey, CommitOid, DiffSelection, DiffSide, DraftComment, DraftId, RepoPath,
        ReviewComment, ReviewOutcome, ReviewThread, SubmitRequest, SubmitResult, ThreadId,
    },
    process::{CommandError, CommandRunner},
};

use super::{
    super::{DraftBody, NewDraftComment, ProviderError, ReviewProvider},
    GitHubProvider, parse_json,
};

// Pending reviews are only visible to their author, so the first PENDING
// review is the viewer's own pending review.
const REVIEW_CONTEXT_QUERY: &str = r#"
query ReviewContext($owner: String!, $name: String!, $number: Int!) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      id
      reviews(states: PENDING, first: 1) { nodes { id } }
    }
  }
}
"#;

const CREATE_REVIEW: &str = r#"
mutation CreateReview($input: AddPullRequestReviewInput!) {
  addPullRequestReview(input: $input) { pullRequestReview { id } }
}
"#;

const CREATE_DRAFT: &str = r#"
mutation CreateDraft($input: AddPullRequestReviewThreadInput!) {
  addPullRequestReviewThread(input: $input) {
    thread { id comments(first: 1) { nodes { id body } } }
  }
}
"#;

const UPDATE_DRAFT: &str = r#"
mutation UpdateDraft($input: UpdatePullRequestReviewCommentInput!) {
  updatePullRequestReviewComment(input: $input) { pullRequestReviewComment { id body } }
}
"#;

const DELETE_DRAFT: &str = r#"
mutation DeleteDraft($input: DeletePullRequestReviewCommentInput!) {
  deletePullRequestReviewComment(input: $input) { clientMutationId }
}
"#;

// The reply payload exposes only the comment (verified by introspection);
// the refreshed thread is fetched separately through `node(id:)`.
const REPLY: &str = r#"
mutation Reply($input: AddPullRequestReviewThreadReplyInput!) {
  addPullRequestReviewThreadReply(input: $input) {
    comment { id }
  }
}
"#;

const THREAD_QUERY: &str = r#"
query Thread($id: ID!) {
  node(id: $id) {
    ... on PullRequestReviewThread {
      id path isResolved isOutdated diffSide
      comments(first: 100) {
        nodes {
          id body line originalLine viewerDidAuthor
          author { login }
          pullRequestReview { state }
        }
      }
    }
  }
}
"#;

const RESOLVE_THREAD: &str = r#"
mutation ResolveThread($input: ResolveReviewThreadInput!) {
  resolveReviewThread(input: $input) { thread { id } }
}
"#;

const UNRESOLVE_THREAD: &str = r#"
mutation UnresolveThread($input: UnresolveReviewThreadInput!) {
  unresolveReviewThread(input: $input) { thread { id } }
}
"#;

const MARK_FILE: &str = r#"
mutation MarkFile($input: MarkFileAsViewedInput!) {
  markFileAsViewed(input: $input) { pullRequest { id } }
}
"#;

const UNMARK_FILE: &str = r#"
mutation UnmarkFile($input: UnmarkFileAsViewedInput!) {
  unmarkFileAsViewed(input: $input) { pullRequest { id } }
}
"#;

const SUBMIT_REVIEW: &str = r#"
mutation SubmitReview($input: SubmitPullRequestReviewInput!) {
  submitPullRequestReview(input: $input) { pullRequestReview { id } }
}
"#;

const DISCARD_REVIEW: &str = r#"
mutation DiscardReview($input: DeletePullRequestReviewInput!) {
  deletePullRequestReview(input: $input) { clientMutationId }
}
"#;

struct ReviewContext {
    pull_request_id: String,
    review_id: Option<String>,
}

impl<R> GitHubProvider<R>
where
    R: CommandRunner + 'static,
{
    pub(super) async fn create_draft_comment(
        &self,
        key: &ChangeRequestKey,
        expected_head: &CommitOid,
        input: NewDraftComment,
    ) -> Result<DraftComment, ProviderError> {
        self.ensure_head(key, expected_head).await?;
        let context = self.review_context(key).await?;
        let review_id = match context.review_id {
            Some(id) => id,
            None => self.create_review(key, &context.pull_request_id).await?,
        };
        let body = input
            .suggestion
            .as_deref()
            .map(suggestion_body)
            .unwrap_or(input.body.0);
        let mut mutation_input = selection_input(&input.selection, &body);
        mutation_input.insert("pullRequestId".into(), json!(context.pull_request_id));
        mutation_input.insert("pullRequestReviewId".into(), json!(review_id));
        mutation_input.insert("clientMutationId".into(), json!(input.operation_id));
        let bytes = self
            .write_graphql(
                key,
                CREATE_DRAFT,
                json!({ "input": mutation_input }),
                "create draft",
            )
            .await?;
        let value: Value = parse_json(&bytes, "create draft")?;
        let thread = &value["data"]["addPullRequestReviewThread"]["thread"];
        let thread_id = string_at(&thread["id"], "create draft", "thread id")?;
        let comment = thread["comments"]["nodes"]
            .as_array()
            .and_then(|nodes| nodes.first())
            .ok_or_else(|| malformed("create draft", "missing draft comment"))?;
        Ok(DraftComment {
            id: DraftId(string_at(&comment["id"], "create draft", "draft id")?),
            body: string_at(&comment["body"], "create draft", "body")?,
            selection: Some(input.selection),
            thread_id: Some(ThreadId(thread_id)),
        })
    }

    pub(super) async fn update_draft_comment(
        &self,
        key: &ChangeRequestKey,
        id: &DraftId,
        body: DraftBody,
    ) -> Result<DraftComment, ProviderError> {
        let bytes = self
            .write_graphql(
                key,
                UPDATE_DRAFT,
                json!({ "input": { "pullRequestReviewCommentId": id.0, "body": body.0 } }),
                "update draft",
            )
            .await?;
        let value: Value = parse_json(&bytes, "update draft")?;
        let comment = &value["data"]["updatePullRequestReviewComment"]["pullRequestReviewComment"];
        Ok(DraftComment {
            id: DraftId(string_at(&comment["id"], "update draft", "draft id")?),
            body: string_at(&comment["body"], "update draft", "body")?,
            selection: None,
            thread_id: None,
        })
    }

    pub(super) async fn delete_draft_comment(
        &self,
        key: &ChangeRequestKey,
        id: &DraftId,
    ) -> Result<(), ProviderError> {
        // DeletePullRequestReviewCommentInput takes `id`, unlike the update
        // input's `pullRequestReviewCommentId` (verified by introspection).
        self.write_graphql(
            key,
            DELETE_DRAFT,
            json!({ "input": { "id": id.0 } }),
            "delete draft",
        )
        .await
        .map(|_| ())
    }

    pub(super) async fn reply_to_thread(
        &self,
        key: &ChangeRequestKey,
        thread: &ThreadId,
        body: DraftBody,
    ) -> Result<ReviewThread, ProviderError> {
        let bytes = self
            .write_graphql(
                key,
                REPLY,
                json!({ "input": { "pullRequestReviewThreadId": thread.0, "body": body.0 } }),
                "reply",
            )
            .await?;
        let value: Value = parse_json(&bytes, "reply")?;
        string_at(
            &value["data"]["addPullRequestReviewThreadReply"]["comment"]["id"],
            "reply",
            "comment id",
        )?;
        // Refetch the whole thread so earlier comments survive the update.
        let bytes = self
            .client
            .graphql(
                &key.host,
                THREAD_QUERY,
                json!({ "id": thread.0 }),
                "reload thread",
            )
            .await?;
        let value: Value = parse_json(&bytes, "reload thread")?;
        let wire: super::wire::ReviewThread = serde_json::from_value(value["data"]["node"].clone())
            .map_err(|error| malformed("reload thread", &error.to_string()))?;
        let (mut threads, _) = super::map_threads(vec![wire]);
        threads
            .pop()
            .ok_or_else(|| malformed("reload thread", "missing thread"))
    }

    pub(super) async fn change_thread_resolution(
        &self,
        key: &ChangeRequestKey,
        thread: &ThreadId,
        resolved: bool,
    ) -> Result<(), ProviderError> {
        self.write_graphql(
            key,
            if resolved {
                RESOLVE_THREAD
            } else {
                UNRESOLVE_THREAD
            },
            json!({ "input": { "threadId": thread.0 } }),
            if resolved {
                "resolve thread"
            } else {
                "unresolve thread"
            },
        )
        .await
        .map(|_| ())
    }

    pub(super) async fn change_file_reviewed(
        &self,
        key: &ChangeRequestKey,
        path: &RepoPath,
        reviewed: bool,
    ) -> Result<(), ProviderError> {
        let context = self.review_context(key).await?;
        self.write_graphql(
            key,
            if reviewed { MARK_FILE } else { UNMARK_FILE },
            json!({ "input": { "pullRequestId": context.pull_request_id, "path": path.0 } }),
            if reviewed {
                "mark file viewed"
            } else {
                "unmark file viewed"
            },
        )
        .await
        .map(|_| ())
    }

    pub(super) async fn submit_pending_review(
        &self,
        key: &ChangeRequestKey,
        request: SubmitRequest,
    ) -> Result<SubmitResult, ProviderError> {
        self.ensure_head(key, &request.expected_head).await?;
        let context = self.review_context(key).await?;
        let review_id = context.review_id.ok_or_else(|| ProviderError::NotFound {
            resource: "pending GitHub review".into(),
        })?;
        let event = match request.outcome {
            ReviewOutcome::Comment => "COMMENT",
            ReviewOutcome::Approve => "APPROVE",
            ReviewOutcome::RequestChanges => "REQUEST_CHANGES",
        };
        self.write_graphql(
            key,
            SUBMIT_REVIEW,
            json!({ "input": {
                "pullRequestReviewId": review_id,
                "event": event,
                "body": request.summary,
            }}),
            "submit review",
        )
        .await?;
        Ok(SubmitResult::Complete)
    }

    pub(super) async fn discard_pending_review(
        &self,
        key: &ChangeRequestKey,
    ) -> Result<(), ProviderError> {
        let context = self.review_context(key).await?;
        let review_id = context.review_id.ok_or_else(|| ProviderError::NotFound {
            resource: "pending GitHub review".into(),
        })?;
        self.write_graphql(
            key,
            DISCARD_REVIEW,
            json!({ "input": { "pullRequestReviewId": review_id } }),
            "discard review",
        )
        .await
        .map(|_| ())
    }

    async fn ensure_head(
        &self,
        key: &ChangeRequestKey,
        expected: &CommitOid,
    ) -> Result<(), ProviderError> {
        let actual = <Self as ReviewProvider>::read_head(self, key).await?;
        if &actual == expected {
            Ok(())
        } else {
            Err(ProviderError::StaleHead {
                expected: expected.clone(),
                actual,
            })
        }
    }

    async fn review_context(&self, key: &ChangeRequestKey) -> Result<ReviewContext, ProviderError> {
        let (owner, name) = super::repository_parts(&key.repository)?;
        let bytes = self
            .client
            .graphql(
                &key.host,
                REVIEW_CONTEXT_QUERY,
                json!({ "owner": owner, "name": name, "number": key.number }),
                "load review context",
            )
            .await?;
        let value: Value = parse_json(&bytes, "load review context")?;
        let pull_request = &value["data"]["repository"]["pullRequest"];
        Ok(ReviewContext {
            pull_request_id: string_at(
                &pull_request["id"],
                "load review context",
                "pull request id",
            )?,
            review_id: pull_request["reviews"]["nodes"][0]["id"]
                .as_str()
                .map(str::to_owned),
        })
    }

    async fn create_review(
        &self,
        key: &ChangeRequestKey,
        pull_request_id: &str,
    ) -> Result<String, ProviderError> {
        let bytes = self
            .write_graphql(
                key,
                CREATE_REVIEW,
                json!({ "input": { "pullRequestId": pull_request_id } }),
                "create pending review",
            )
            .await?;
        let value: Value = parse_json(&bytes, "create pending review")?;
        string_at(
            &value["data"]["addPullRequestReview"]["pullRequestReview"]["id"],
            "create pending review",
            "review id",
        )
    }

    async fn write_graphql(
        &self,
        key: &ChangeRequestKey,
        query: &str,
        variables: Value,
        operation: &str,
    ) -> Result<Vec<u8>, ProviderError> {
        match self
            .client
            .graphql(&key.host, query, variables, operation)
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

fn selection_input(selection: &DiffSelection, body: &str) -> Map<String, Value> {
    let mut input = Map::new();
    input.insert("body".into(), json!(body));
    input.insert("path".into(), json!(selection.end.path.0));
    input.insert("line".into(), json!(selection.end.line));
    input.insert("side".into(), json!(side_name(selection.end.side)));
    if selection.start != selection.end {
        input.insert("startLine".into(), json!(selection.start.line));
        input.insert("startSide".into(), json!(side_name(selection.start.side)));
    }
    input
}

fn side_name(side: DiffSide) -> &'static str {
    match side {
        DiffSide::Left => "LEFT",
        DiffSide::Right => "RIGHT",
    }
}

fn suggestion_body(replacement: &str) -> String {
    format!("```suggestion\n{}\n```", replacement.trim_end_matches('\n'))
}

fn string_at(value: &Value, operation: &str, field: &str) -> Result<String, ProviderError> {
    value
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| malformed(operation, &format!("missing {field}")))
}

fn malformed(operation: &str, message: &str) -> ProviderError {
    ProviderError::MalformedResponse {
        operation: operation.into(),
        message: message.into(),
    }
}
