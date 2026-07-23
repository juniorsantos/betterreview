use async_trait::async_trait;
use betterreview::{
    domain::{
        ChangeRequestKey, CommitOid, DiffPosition, DiffSelection, DiffSide, ProviderKind,
        ReviewOutcome, SubmitMode, SubmitRequest, SubmitResult,
    },
    process::{CommandError, CommandOutput, CommandRunner, CommandSpec},
    providers::{DraftBody, GitLabProvider, NewDraftComment, ProviderError, ReviewProvider},
};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

struct RecordingRunner {
    responses: Mutex<VecDeque<CommandOutput>>,
    calls: Mutex<Vec<CommandSpec>>,
}

impl RecordingRunner {
    fn new(responses: Vec<CommandOutput>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
            calls: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl CommandRunner for RecordingRunner {
    async fn run(&self, spec: CommandSpec) -> Result<CommandOutput, CommandError> {
        self.calls.lock().unwrap().push(spec);
        Ok(self.responses.lock().unwrap().pop_front().unwrap())
    }
}

#[tokio::test]
async fn creates_multiline_draft_with_nested_position_json() {
    let runner = Arc::new(RecordingRunner::new(vec![
        merge_request("head-sha"),
        success(json!({
            "id": 31,
            "note": "body $(); café",
            "position": {
                "old_path": "src/lib.rs",
                "new_path": "src/lib.rs",
                "old_line": null,
                "new_line": 9
            }
        })),
    ]));
    let provider = GitLabProvider::new(runner.clone());

    let draft = provider
        .create_draft(
            &key(),
            &CommitOid("head-sha".into()),
            NewDraftComment {
                body: DraftBody("body $(); café".into()),
                selection: selection(DiffSide::Right, 7, 9),
                suggestion: None,
                operation_id: "operation-1".into(),
            },
        )
        .await
        .unwrap();

    assert_eq!(draft.id.0, "31");
    let calls = runner.calls.lock().unwrap();
    let mutation = calls.last().unwrap();
    let body = json_body(mutation);
    assert_eq!(body["note"], "body $(); café");
    assert_eq!(body["position"]["position_type"], "text");
    assert_eq!(body["position"]["base_sha"], "base-sha");
    assert_eq!(body["position"]["start_sha"], "start-sha");
    assert_eq!(body["position"]["head_sha"], "head-sha");
    assert_eq!(body["position"]["new_line"], 9);
    assert_eq!(body["position"]["line_range"]["start"]["type"], "new");
    assert_eq!(body["position"]["line_range"]["start"]["new_line"], 7);
    assert!(
        mutation
            .args
            .iter()
            .all(|argument| argument.to_string_lossy() != "body $(); café")
    );
}

#[tokio::test]
async fn stale_head_prevents_gitlab_write() {
    let runner = Arc::new(RecordingRunner::new(vec![merge_request("new-head")]));
    let provider = GitLabProvider::new(runner.clone());

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
async fn approval_failure_after_publication_returns_partial_retry() {
    let runner = Arc::new(RecordingRunner::new(vec![
        merge_request("head-sha"),
        success(json!({ "published_drafts": 2 })),
        CommandOutput {
            status: 1,
            stdout: Vec::new(),
            stderr: b"approval denied".to_vec(),
        },
    ]));
    let provider = GitLabProvider::new(runner.clone());

    let result = provider
        .submit_review(
            &key(),
            SubmitRequest {
                expected_head: CommitOid("head-sha".into()),
                summary: "summary".into(),
                outcome: ReviewOutcome::Approve,
                mode: SubmitMode::Full,
            },
        )
        .await
        .unwrap();

    assert!(matches!(
        result,
        SubmitResult::Partial {
            published_drafts: 2,
            retry: SubmitMode::OutcomeOnly,
            ..
        }
    ));
    let calls = runner.calls.lock().unwrap();
    let publish = json_body(&calls[1]);
    assert_eq!(publish["note"], "summary");
    assert_eq!(publish["reviewer_state"], "reviewed");
    let approve = json_body(&calls[2]);
    assert_eq!(approve["sha"], "head-sha");
}

#[tokio::test]
async fn outcome_only_retry_skips_bulk_publication() {
    let runner = Arc::new(RecordingRunner::new(vec![
        merge_request("head-sha"),
        success(json!({ "approved": true })),
    ]));
    let provider = GitLabProvider::new(runner.clone());

    let result = provider
        .submit_review(
            &key(),
            SubmitRequest {
                expected_head: CommitOid("head-sha".into()),
                summary: "summary".into(),
                outcome: ReviewOutcome::Approve,
                mode: SubmitMode::OutcomeOnly,
            },
        )
        .await
        .unwrap();

    assert_eq!(result, SubmitResult::Complete);
    let calls = runner.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert!(args(&calls[1]).iter().any(|arg| arg.ends_with("/approve")));
    assert!(
        !calls
            .iter()
            .flat_map(args)
            .any(|arg| arg.contains("bulk_publish"))
    );
}

#[tokio::test]
async fn request_changes_publishes_reviewer_state() {
    let runner = Arc::new(RecordingRunner::new(vec![
        merge_request("head-sha"),
        success(json!({ "published_drafts": 1 })),
    ]));
    let provider = GitLabProvider::new(runner.clone());

    provider
        .submit_review(
            &key(),
            SubmitRequest {
                expected_head: CommitOid("head-sha".into()),
                summary: "changes needed".into(),
                outcome: ReviewOutcome::RequestChanges,
                mode: SubmitMode::Full,
            },
        )
        .await
        .unwrap();

    let calls = runner.calls.lock().unwrap();
    assert_eq!(json_body(&calls[1])["reviewer_state"], "requested_changes");
}

fn key() -> ChangeRequestKey {
    ChangeRequestKey {
        provider: ProviderKind::GitLab,
        host: "git.acme.test".into(),
        repository: "group/api".into(),
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

fn merge_request(head: &str) -> CommandOutput {
    success(json!({
        "iid": 42,
        "title": "title",
        "web_url": "https://git.acme.test/group/api/-/merge_requests/42",
        "author": { "username": "alice" },
        "diff_refs": {
            "base_sha": "base-sha",
            "start_sha": "start-sha",
            "head_sha": head
        }
    }))
}

fn success(value: Value) -> CommandOutput {
    CommandOutput {
        status: 0,
        stdout: serde_json::to_vec(&value).unwrap(),
        stderr: Vec::new(),
    }
}

fn json_body(spec: &CommandSpec) -> Value {
    serde_json::from_slice(spec.stdin.as_ref().unwrap()).unwrap()
}

fn args(spec: &CommandSpec) -> Vec<String> {
    spec.args
        .iter()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect()
}
