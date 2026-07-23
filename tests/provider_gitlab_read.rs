use async_trait::async_trait;
use betterreview::{
    domain::{ChangeRequestKey, ProviderKind, ReviewOutcome, Support},
    process::{CommandError, CommandOutput, CommandRunner, CommandSpec},
    providers::{GitLabProvider, ProviderError, ReviewProvider},
};
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
async fn loads_gitlab_snapshot_and_capabilities_with_encoded_namespace() {
    let runner = Arc::new(RecordingRunner::new(vec![
        fixture("merge-request.json"),
        fixture("diffs.ndjson"),
        fixture("draft-notes.json"),
        fixture("discussions.ndjson"),
        fixture("approvals.json"),
        fixture("version.json"),
        blob("blob-head-1"),
        blob("blob-head-2"),
        blob("blob-base-3"),
    ]));
    let provider = GitLabProvider::new(runner.clone());

    let snapshot = provider.load(&key()).await.unwrap();

    assert_eq!(snapshot.key.number, 42);
    assert_eq!(snapshot.base.as_ref(), "base-sha");
    assert_eq!(snapshot.head.as_ref(), "head-sha");
    assert_eq!(snapshot.files.len(), 3);
    assert_eq!(snapshot.threads.len(), 1);
    assert_eq!(snapshot.drafts.len(), 1);
    assert_eq!(snapshot.files[0].head_blob.as_deref(), Some("blob-head-1"));
    assert!(matches!(
        snapshot
            .capabilities
            .for_outcome(ReviewOutcome::RequestChanges),
        Support::Supported
    ));
    assert!(matches!(
        snapshot.capabilities.mark_file_reviewed,
        Support::Unsupported { ref reason }
            if reason == "This GitLab instance has no file-viewed API; progress is stored locally"
    ));

    let calls = runner.calls.lock().unwrap();
    for expected in [
        vec![
            "api",
            "--hostname",
            "git.acme.test",
            "projects/group%2Fapi/merge_requests/42",
        ],
        vec![
            "api",
            "--hostname",
            "git.acme.test",
            "--paginate",
            "--output",
            "ndjson",
            "projects/group%2Fapi/merge_requests/42/diffs?unidiff=true",
        ],
        vec![
            "api",
            "--hostname",
            "git.acme.test",
            "projects/group%2Fapi/merge_requests/42/draft_notes",
        ],
        vec![
            "api",
            "--hostname",
            "git.acme.test",
            "--paginate",
            "--output",
            "ndjson",
            "projects/group%2Fapi/merge_requests/42/discussions",
        ],
        vec![
            "api",
            "--hostname",
            "git.acme.test",
            "projects/group%2Fapi/merge_requests/42/approvals",
        ],
        vec!["api", "--hostname", "git.acme.test", "version"],
    ] {
        assert!(calls.iter().any(|spec| args(spec) == expected));
    }
}

#[tokio::test]
async fn disables_request_changes_before_gitlab_17_3() {
    let runner = Arc::new(RecordingRunner::new(vec![
        fixture("merge-request.json"),
        fixture("diffs.ndjson"),
        fixture("draft-notes.json"),
        fixture("discussions.ndjson"),
        fixture("approvals.json"),
        output(br#"{"version":"17.2.9","tier":"premium"}"#.to_vec()),
        blob("blob-head-1"),
        blob("blob-head-2"),
        blob("blob-base-3"),
    ]));
    let provider = GitLabProvider::new(runner);

    let snapshot = provider.load(&key()).await.unwrap();

    assert!(matches!(
        snapshot
            .capabilities
            .for_outcome(ReviewOutcome::RequestChanges),
        Support::Unsupported { reason } if reason.contains("17.3")
    ));
}

#[tokio::test]
async fn maps_malformed_ndjson_and_authentication() {
    let malformed = Arc::new(RecordingRunner::new(vec![
        fixture("merge-request.json"),
        output(b"not-json\n".to_vec()),
    ]));
    let provider = GitLabProvider::new(malformed);
    assert!(matches!(
        provider.load(&key()).await,
        Err(ProviderError::MalformedResponse { .. })
    ));

    let auth = Arc::new(RecordingRunner::new(vec![CommandOutput {
        status: 1,
        stdout: Vec::new(),
        stderr: b"authentication required; run glab auth login".to_vec(),
    }]));
    let provider = GitLabProvider::new(auth);
    assert!(matches!(
        provider.probe("git.acme.test").await,
        Err(ProviderError::Authentication { .. })
    ));
}

fn key() -> ChangeRequestKey {
    ChangeRequestKey {
        provider: ProviderKind::GitLab,
        host: "git.acme.test".into(),
        repository: "group/api".into(),
        number: 42,
    }
}

fn fixture(name: &str) -> CommandOutput {
    output(std::fs::read(format!("tests/fixtures/gitlab/{name}")).unwrap())
}

fn blob(id: &str) -> CommandOutput {
    output(serde_json::to_vec(&serde_json::json!({ "blob_id": id })).unwrap())
}

fn output(stdout: Vec<u8>) -> CommandOutput {
    CommandOutput {
        status: 0,
        stdout,
        stderr: Vec::new(),
    }
}

fn args(spec: &CommandSpec) -> Vec<&str> {
    spec.args
        .iter()
        .map(|argument| argument.to_str().unwrap())
        .collect()
}
