use async_trait::async_trait;
use betterreview::{
    domain::{ChangeRequestKey, CommitOid, ProviderKind, RepoPath, ReviewOutcome, Support},
    process::{CommandError, CommandOutput, CommandRunner, CommandSpec},
    providers::{GitLabProvider, ProviderError, ReviewProvider},
};
use std::{
    collections::BTreeMap,
    io,
    sync::{Arc, Mutex},
    time::Duration,
};

/// Routes responses by the endpoint (last argument), so calls may arrive in
/// any order or concurrently.
struct RoutingRunner {
    responses: BTreeMap<String, CommandOutput>,
    calls: Mutex<Vec<CommandSpec>>,
    delay: Option<Duration>,
}

impl RoutingRunner {
    fn new(responses: Vec<(&str, CommandOutput)>) -> Self {
        Self {
            responses: responses
                .into_iter()
                .map(|(endpoint, output)| (endpoint.to_owned(), output))
                .collect(),
            calls: Mutex::new(Vec::new()),
            delay: None,
        }
    }

    fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }
}

#[async_trait]
impl CommandRunner for RoutingRunner {
    async fn run(&self, spec: CommandSpec) -> Result<CommandOutput, CommandError> {
        if let Some(delay) = self.delay {
            tokio::time::sleep(delay).await;
        }
        let endpoint = spec
            .args
            .last()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.calls.lock().unwrap().push(spec.clone());
        match self.responses.get(&endpoint) {
            Some(output) => Ok(output.clone()),
            None => Err(CommandError::Spawn {
                program: spec.program,
                source: io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("no fixture for endpoint {endpoint}"),
                ),
            }),
        }
    }
}

fn snapshot_responses() -> Vec<(&'static str, CommandOutput)> {
    vec![
        (
            "projects/group%2Fapi/merge_requests/42",
            fixture("merge-request.json"),
        ),
        (
            "projects/group%2Fapi/merge_requests/42/diffs?unidiff=true&per_page=100",
            fixture("diffs.ndjson"),
        ),
        (
            "projects/group%2Fapi/merge_requests/42/draft_notes",
            fixture("draft-notes.json"),
        ),
        (
            "projects/group%2Fapi/merge_requests/42/discussions?per_page=100",
            fixture("discussions.ndjson"),
        ),
        (
            "projects/group%2Fapi/merge_requests/42/approvals",
            fixture("approvals.json"),
        ),
        ("version", fixture("version.json")),
        (
            "projects/group%2Fapi/repository/files/src%2Fone.rs?ref=head-sha",
            blob("blob-head-1"),
        ),
        (
            "projects/group%2Fapi/repository/files/src%2Fnew.rs?ref=head-sha",
            blob("blob-head-2"),
        ),
        (
            "projects/group%2Fapi/repository/files/src%2Fdeleted.rs?ref=base-sha",
            blob("blob-base-3"),
        ),
    ]
}

#[tokio::test]
async fn loads_gitlab_snapshot_and_capabilities_with_encoded_namespace() {
    let runner = Arc::new(RoutingRunner::new(snapshot_responses()));
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
            "projects/group%2Fapi/merge_requests/42/diffs?unidiff=true&per_page=100",
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
            "projects/group%2Fapi/merge_requests/42/discussions?per_page=100",
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
async fn snapshot_calls_overlap_instead_of_running_sequentially() {
    let delay = Duration::from_millis(50);
    let runner = Arc::new(RoutingRunner::new(snapshot_responses()).with_delay(delay));
    let provider = GitLabProvider::new(runner);

    let started = std::time::Instant::now();
    provider.load(&key()).await.unwrap();
    let elapsed = started.elapsed();

    // 9 sequential calls would take >= 450ms; concurrent metadata + blob
    // batches should finish in roughly two delay windows.
    assert!(
        elapsed < Duration::from_millis(300),
        "load took {elapsed:?}, calls are not overlapping"
    );
}

#[tokio::test]
async fn disables_request_changes_before_gitlab_17_3() {
    let mut responses = snapshot_responses();
    responses.retain(|(endpoint, _)| *endpoint != "version");
    responses.push((
        "version",
        output(br#"{"version":"17.2.9","tier":"premium"}"#.to_vec()),
    ));
    let runner = Arc::new(RoutingRunner::new(responses));
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
    let mut responses = snapshot_responses();
    responses.retain(|(endpoint, _)| !endpoint.ends_with("diffs?unidiff=true"));
    responses.push((
        "projects/group%2Fapi/merge_requests/42/diffs?unidiff=true&per_page=100",
        output(b"not-json\n".to_vec()),
    ));
    let malformed = Arc::new(RoutingRunner::new(responses));
    let provider = GitLabProvider::new(malformed);
    assert!(matches!(
        provider.load(&key()).await,
        Err(ProviderError::MalformedResponse { .. })
    ));

    let auth = Arc::new(RoutingRunner::new(vec![(
        "user",
        CommandOutput {
            status: 1,
            stdout: Vec::new(),
            stderr: b"authentication required; run glab auth login".to_vec(),
        },
    )]));
    let provider = GitLabProvider::new(auth);
    assert!(matches!(
        provider.probe("git.acme.test").await,
        Err(ProviderError::Authentication { .. })
    ));
}

#[tokio::test]
async fn lists_open_merge_requests_in_one_call() {
    let runner = Arc::new(RoutingRunner::new(vec![(
        "projects/group%2Fapi/merge_requests?state=opened&order_by=updated_at&sort=desc&per_page=50",
        fixture("merge-requests-list.json"),
    )]));
    let provider = GitLabProvider::new(runner.clone());

    let list = provider
        .list_open("git.acme.test", "group/api")
        .await
        .unwrap();

    assert_eq!(list.len(), 2);
    assert_eq!(list[0].number, 12);
    assert_eq!(list[0].source_branch, "feature/picker");
    assert_eq!(list[0].description, "Adds the review picker prefetch flow.");
    assert!(list[1].draft);
    assert_eq!(list[1].description, "");
    assert_eq!(runner.calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn reads_file_contents_at_a_revision() {
    let runner = Arc::new(RoutingRunner::new(vec![(
        "projects/group%2Fapi/repository/files/src%2Fnested%2Ffile.rs/raw?ref=deadbeef",
        output(b"fn example() {}\n".to_vec()),
    )]));
    let provider = GitLabProvider::new(runner.clone());

    let contents = provider
        .read_file(
            &key(),
            &RepoPath("src/nested/file.rs".into()),
            &CommitOid("deadbeef".into()),
        )
        .await
        .unwrap();

    assert_eq!(contents, "fn example() {}\n");
    let calls = runner.calls.lock().unwrap();
    assert!(calls.iter().any(|spec| args(spec)
        == vec![
            "api",
            "--hostname",
            "git.acme.test",
            "projects/group%2Fapi/repository/files/src%2Fnested%2Ffile.rs/raw?ref=deadbeef",
        ]));
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

fn server_error() -> CommandOutput {
    CommandOutput {
        status: 1,
        stdout: Vec::new(),
        stderr: b"glab: 500 Internal Server Error (HTTP 500)".to_vec(),
    }
}

#[tokio::test]
async fn falls_back_to_changes_when_the_diffs_endpoint_errors() {
    let mut responses = snapshot_responses();
    responses.retain(|(endpoint, _)| !endpoint.contains("/diffs"));
    responses.push((
        "projects/group%2Fapi/merge_requests/42/diffs?unidiff=true&per_page=100",
        server_error(),
    ));
    responses.push((
        "projects/group%2Fapi/merge_requests/42/changes?unidiff=true",
        fixture("changes.json"),
    ));
    let runner = Arc::new(RoutingRunner::new(responses));
    let provider = GitLabProvider::new(runner.clone());

    let snapshot = provider
        .load(&key())
        .await
        .expect("an older gitlab still has to be reviewable");

    assert!(
        !snapshot.files.is_empty(),
        "the fallback has to produce the same files the modern endpoint would"
    );
    let calls = runner.calls.lock().unwrap();
    assert!(
        calls.iter().any(|call| {
            call.args
                .iter()
                .any(|arg| arg.to_string_lossy().contains("/changes"))
        }),
        "the fallback was actually taken"
    );
}

#[tokio::test]
async fn a_multi_line_draft_keeps_its_range_when_the_review_is_reopened() {
    let mut responses = snapshot_responses();
    responses.retain(|(endpoint, _)| !endpoint.ends_with("draft_notes"));
    responses.push((
        "projects/group%2Fapi/merge_requests/42/draft_notes",
        fixture("draft-notes-range.json"),
    ));
    let provider = GitLabProvider::new(Arc::new(RoutingRunner::new(responses)));

    let snapshot = provider.load(&key()).await.unwrap();

    let selection = snapshot.drafts[0]
        .selection
        .as_ref()
        .expect("the draft is anchored");
    assert_eq!(
        (selection.start.line, selection.end.line),
        (2, 4),
        "collapsing the range to its last line is what moves the marking on reopen"
    );
}
