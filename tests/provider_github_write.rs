use async_trait::async_trait;
use betterreview::{
    domain::{
        ChangeRequestKey, CommitOid, DiffPosition, DiffSelection, DiffSide, ProviderKind,
        ReviewOutcome, SubmitMode, SubmitRequest,
    },
    process::{CommandError, CommandOutput, CommandRunner, CommandSpec},
    providers::{DraftBody, GitHubProvider, NewDraftComment, ProviderError, ReviewProvider},
};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

struct RecordingRunner {
    responses: Mutex<VecDeque<Result<CommandOutput, ResponseError>>>,
    calls: Mutex<Vec<CommandSpec>>,
}

enum ResponseError {
    Timeout,
}

impl RecordingRunner {
    fn new(responses: Vec<Result<CommandOutput, ResponseError>>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
            calls: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl CommandRunner for RecordingRunner {
    async fn run(&self, spec: CommandSpec) -> Result<CommandOutput, CommandError> {
        self.calls.lock().unwrap().push(spec.clone());
        match self.responses.lock().unwrap().pop_front().unwrap() {
            Ok(output) => Ok(output),
            Err(ResponseError::Timeout) => Err(CommandError::Timeout {
                timeout: spec.timeout,
            }),
        }
    }
}

#[tokio::test]
async fn creates_multiline_draft_with_body_only_in_json_stdin() {
    let runner = Arc::new(RecordingRunner::new(vec![
        Ok(json_output(json!({ "head": { "sha": "head-oid" } }))),
        Ok(json_output(review_context())),
        Ok(json_output(json!({
            "data": {
                "addPullRequestReviewThread": {
                    "thread": {
                        "id": "thread-new",
                        "comments": { "nodes": [{ "id": "draft-new", "body": "quoted \"body\"\n$(); café" }] }
                    }
                }
            }
        }))),
    ]));
    let provider = GitHubProvider::new(runner.clone());
    let body = "quoted \"body\"\n$(); café";

    let draft = provider
        .create_draft(
            &key(),
            &CommitOid("head-oid".into()),
            NewDraftComment {
                body: DraftBody(body.into()),
                selection: selection(DiffSide::Right, 7, 9),
                suggestion: None,
                operation_id: "operation-1".into(),
            },
        )
        .await
        .unwrap();

    assert_eq!(draft.id.0, "draft-new");
    let calls = runner.calls.lock().unwrap();
    let context_query = graphql_body(&calls[1]);
    let query = context_query["query"].as_str().unwrap();
    assert!(
        !query.contains("viewerPendingReview"),
        "viewerPendingReview does not exist in the GitHub schema"
    );
    assert!(query.contains("states: PENDING"));
    let mutation = graphql_body(calls.last().unwrap());
    assert_eq!(mutation["variables"]["input"]["body"], body);
    assert_eq!(mutation["variables"]["input"]["startLine"], 7);
    assert_eq!(mutation["variables"]["input"]["line"], 9);
    assert_eq!(mutation["variables"]["input"]["startSide"], "RIGHT");
    assert!(calls.iter().all(|call| {
        call.args
            .iter()
            .all(|argument| argument.to_string_lossy() != body)
    }));
}

#[tokio::test]
async fn formats_suggestion_body() {
    let runner = Arc::new(RecordingRunner::new(vec![
        Ok(json_output(json!({ "head": { "sha": "head-oid" } }))),
        Ok(json_output(review_context())),
        Ok(json_output(json!({
            "data": {
                "addPullRequestReviewThread": {
                    "thread": {
                        "id": "thread-new",
                        "comments": { "nodes": [{ "id": "draft-new", "body": "```suggestion\nreplacement\n```" }] }
                    }
                }
            }
        }))),
    ]));
    let provider = GitHubProvider::new(runner.clone());

    provider
        .create_draft(
            &key(),
            &CommitOid("head-oid".into()),
            NewDraftComment {
                body: DraftBody("ignored".into()),
                selection: selection(DiffSide::Left, 4, 4),
                suggestion: Some("replacement\n".into()),
                operation_id: "operation-2".into(),
            },
        )
        .await
        .unwrap();

    let calls = runner.calls.lock().unwrap();
    let mutation = graphql_body(calls.last().unwrap());
    assert_eq!(
        mutation["variables"]["input"]["body"],
        "```suggestion\nreplacement\n```"
    );
    assert!(mutation["variables"]["input"].get("startLine").is_none());
    assert_eq!(mutation["variables"]["input"]["side"], "LEFT");
}

#[tokio::test]
async fn submits_approve_after_head_revalidation() {
    let runner = Arc::new(RecordingRunner::new(vec![
        Ok(json_output(json!({ "head": { "sha": "head-oid" } }))),
        Ok(json_output(review_context())),
        Ok(json_output(json!({
            "data": { "submitPullRequestReview": { "pullRequestReview": { "id": "review-id" } } }
        }))),
    ]));
    let provider = GitHubProvider::new(runner.clone());

    provider
        .submit_review(
            &key(),
            SubmitRequest {
                expected_head: CommitOid("head-oid".into()),
                summary: "summary".into(),
                outcome: ReviewOutcome::Approve,
                mode: SubmitMode::Full,
            },
        )
        .await
        .unwrap();

    let calls = runner.calls.lock().unwrap();
    let mutation = graphql_body(calls.last().unwrap());
    assert_eq!(mutation["variables"]["input"]["event"], "APPROVE");
    assert_eq!(mutation["variables"]["input"]["body"], "summary");
}

#[tokio::test]
async fn stale_head_prevents_mutation_calls() {
    let runner = Arc::new(RecordingRunner::new(vec![Ok(json_output(json!({
        "head": { "sha": "new-head" }
    })))]));
    let provider = GitHubProvider::new(runner.clone());

    let error = provider
        .submit_review(
            &key(),
            SubmitRequest {
                expected_head: CommitOid("old-head".into()),
                summary: "summary".into(),
                outcome: ReviewOutcome::Comment,
                mode: SubmitMode::Full,
            },
        )
        .await
        .unwrap_err();

    assert!(matches!(error, ProviderError::StaleHead { .. }));
    assert_eq!(runner.calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn timeout_is_reported_as_ambiguous_without_retry() {
    let runner = Arc::new(RecordingRunner::new(vec![
        Ok(json_output(json!({ "head": { "sha": "head-oid" } }))),
        Ok(json_output(review_context())),
        Err(ResponseError::Timeout),
    ]));
    let provider = GitHubProvider::new(runner.clone());

    let error = provider
        .create_draft(
            &key(),
            &CommitOid("head-oid".into()),
            NewDraftComment {
                body: DraftBody("body".into()),
                selection: selection(DiffSide::Right, 3, 3),
                suggestion: None,
                operation_id: "operation-timeout".into(),
            },
        )
        .await
        .unwrap_err();

    assert!(matches!(error, ProviderError::AmbiguousWrite { .. }));
    assert_eq!(runner.calls.lock().unwrap().len(), 3);
}

fn key() -> ChangeRequestKey {
    ChangeRequestKey {
        provider: ProviderKind::GitHub,
        host: "ghe.acme.test".into(),
        repository: "acme/api".into(),
        number: 42,
    }
}

fn selection(side: DiffSide, start: u32, end: u32) -> DiffSelection {
    let position = |line| DiffPosition {
        path: betterreview::domain::RepoPath("src/lib.rs".into()),
        side,
        line,
        hunk: 0,
    };
    DiffSelection {
        start: position(start),
        end: position(end),
    }
}

// Pending reviews are only visible to their author, so the first PENDING
// review returned by the API is the viewer's own.
fn review_context() -> Value {
    json!({
        "data": {
            "repository": {
                "pullRequest": {
                    "id": "pr-id",
                    "reviews": { "nodes": [ { "id": "review-id" } ] }
                }
            }
        }
    })
}

fn json_output(value: Value) -> CommandOutput {
    CommandOutput {
        status: 0,
        stdout: serde_json::to_vec(&value).unwrap(),
        stderr: Vec::new(),
    }
}

fn graphql_body(spec: &CommandSpec) -> Value {
    serde_json::from_slice(spec.stdin.as_ref().unwrap()).unwrap()
}

#[tokio::test]
async fn delete_draft_sends_the_node_id_field() {
    let runner = Arc::new(RecordingRunner::new(vec![Ok(json_output(json!({
        "data": { "deletePullRequestReviewComment": { "clientMutationId": null } }
    })))]));
    let provider = GitHubProvider::new(runner.clone());

    provider
        .delete_draft(
            &key(),
            &betterreview::domain::DraftId("draft-node-id".into()),
        )
        .await
        .unwrap();

    let calls = runner.calls.lock().unwrap();
    let body = graphql_body(calls.last().unwrap());
    // DeletePullRequestReviewCommentInput takes `id` (verified by introspection).
    assert_eq!(body["variables"]["input"]["id"], "draft-node-id");
    assert!(
        body["variables"]["input"]
            .get("pullRequestReviewCommentId")
            .is_none()
    );
}

#[tokio::test]
async fn reply_refetches_the_full_thread() {
    use betterreview::domain::ThreadId;
    let runner = Arc::new(RecordingRunner::new(vec![
        Ok(json_output(json!({
            "data": { "addPullRequestReviewThreadReply": { "comment": { "id": "reply-1" } } }
        }))),
        Ok(json_output(json!({
            "data": {
                "node": {
                    "id": "thread-1",
                    "path": "src/lib.rs",
                    "isResolved": false,
                    "isOutdated": false,
                    "diffSide": "RIGHT",
                    "comments": { "nodes": [
                        {
                            "id": "c1",
                            "body": "primeiro",
                            "line": 3,
                            "originalLine": 3,
                            "viewerDidAuthor": false,
                            "author": { "login": "alice" },
                            "pullRequestReview": { "state": "SUBMITTED" }
                        },
                        {
                            "id": "reply-1",
                            "body": "resposta",
                            "line": 3,
                            "originalLine": 3,
                            "viewerDidAuthor": true,
                            "author": { "login": "you" },
                            "pullRequestReview": { "state": "SUBMITTED" }
                        }
                    ] }
                }
            }
        }))),
    ]));
    let provider = GitHubProvider::new(runner.clone());

    let thread = provider
        .reply(
            &key(),
            &ThreadId("thread-1".into()),
            DraftBody("resposta".into()),
        )
        .await
        .unwrap();

    assert_eq!(
        thread.comments.len(),
        2,
        "prior comments must survive a reply"
    );
    assert_eq!(thread.comments[1].body, "resposta");
    let calls = runner.calls.lock().unwrap();
    let mutation = graphql_body(&calls[0]);
    assert!(
        !mutation["query"]
            .as_str()
            .unwrap()
            .contains("pullRequestReviewThread"),
        "the reply payload has no thread field in the schema"
    );
    let refetch = graphql_body(&calls[1]);
    assert_eq!(refetch["variables"]["id"], "thread-1");
}

fn review_context_without_pending() -> Value {
    json!({
        "data": {
            "repository": {
                "pullRequest": { "id": "pr-id", "reviews": { "nodes": [] } }
            }
        }
    })
}

#[tokio::test]
async fn submitting_without_any_draft_creates_the_review_carrying_the_verdict() {
    let runner = Arc::new(RecordingRunner::new(vec![
        Ok(json_output(json!({ "head": { "sha": "head-oid" } }))),
        Ok(json_output(review_context_without_pending())),
        Ok(json_output(json!({
            "data": { "addPullRequestReview": { "pullRequestReview": { "id": "fresh-review" } } }
        }))),
    ]));
    let provider = GitHubProvider::new(runner.clone());

    provider
        .submit_review(
            &key(),
            SubmitRequest {
                expected_head: CommitOid("head-oid".into()),
                summary: "tudo certo".into(),
                outcome: ReviewOutcome::Approve,
                mode: SubmitMode::Full,
            },
        )
        .await
        .unwrap();

    let calls = runner.calls.lock().unwrap();
    assert_eq!(calls.len(), 3, "head, review context, single create+submit");
    let mutation = graphql_body(calls.last().unwrap());
    assert_eq!(mutation["variables"]["input"]["pullRequestId"], "pr-id");
    assert_eq!(mutation["variables"]["input"]["event"], "APPROVE");
    assert_eq!(mutation["variables"]["input"]["body"], "tudo certo");
}

#[tokio::test]
async fn discarding_without_a_pending_review_is_a_no_op() {
    let runner = Arc::new(RecordingRunner::new(vec![Ok(json_output(
        review_context_without_pending(),
    ))]));
    let provider = GitHubProvider::new(runner.clone());

    provider.discard_review(&key()).await.unwrap();

    assert_eq!(runner.calls.lock().unwrap().len(), 1);
}
