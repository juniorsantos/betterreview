use async_trait::async_trait;
use betterreview::{
    domain::{ChangeRequestKey, CommitOid, PatchAvailability, ProviderKind, RepoPath},
    process::{CommandError, CommandOutput, CommandRunner, CommandSpec},
    providers::{GitHubProvider, ProviderError, ReviewProvider},
};
use serde_json::Value;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

/// Answers by inspecting the request (endpoint args or GraphQL cursor), so
/// calls may arrive in any order or concurrently.
struct RoutingRunner {
    calls: Mutex<Vec<CommandSpec>>,
    delay: Option<Duration>,
    fail_all: bool,
    diff_too_large: bool,
}

impl RoutingRunner {
    fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            delay: None,
            fail_all: false,
            diff_too_large: false,
        }
    }

    fn with_oversized_diff(mut self) -> Self {
        self.diff_too_large = true;
        self
    }

    fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }

    fn failing() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            delay: None,
            fail_all: true,
            diff_too_large: false,
        }
    }

    fn respond(&self, spec: &CommandSpec) -> CommandOutput {
        let args: Vec<String> = spec
            .args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        if args.iter().any(|arg| arg == "graphql") {
            let body: Value =
                serde_json::from_slice(spec.stdin.as_ref().expect("graphql stdin")).unwrap();
            if body["query"].as_str().unwrap_or("").contains("ListOpen(") {
                return ok(fixture("list-open.json"));
            }
            let cursor = &body["variables"]["cursor"];
            if cursor.is_null() {
                ok(fixture("change_request.json"))
            } else {
                ok(fixture("threads-page-2.json"))
            }
        } else if args.iter().any(|arg| arg.contains("/files")) {
            ok(fixture("files-page-1.json"))
        } else if args.iter().any(|arg| arg.contains("/contents/")) {
            ok(b"fn example() {}\n".to_vec())
        } else if args.iter().any(|arg| arg.starts_with("Accept:")) {
            if self.diff_too_large {
                return CommandOutput {
                    status: 1,
                    stdout: Vec::new(),
                    stderr: b"gh: Sorry, the diff exceeded the maximum number of lines (20000) (HTTP 406)"
                        .to_vec(),
                };
            }
            ok(std::fs::read("tests/fixtures/github/pull.diff").unwrap())
        } else {
            ok(b"{}".to_vec())
        }
    }
}

#[async_trait]
impl CommandRunner for RoutingRunner {
    async fn run(&self, spec: CommandSpec) -> Result<CommandOutput, CommandError> {
        if let Some(delay) = self.delay {
            tokio::time::sleep(delay).await;
        }
        self.calls.lock().unwrap().push(spec.clone());
        if self.fail_all {
            return Ok(CommandOutput {
                status: 1,
                stdout: Vec::new(),
                stderr: b"authentication required; run gh auth login".to_vec(),
            });
        }
        Ok(self.respond(&spec))
    }
}

#[tokio::test]
async fn loads_paginated_github_snapshot_with_structured_commands() {
    let runner = Arc::new(RoutingRunner::new());
    let provider = GitHubProvider::new(runner.clone());
    let key = github_key();

    let snapshot = provider.load(&key).await.unwrap();

    assert_eq!(snapshot.key.number, 42);
    assert_eq!(snapshot.base.as_ref(), "base-oid");
    assert_eq!(snapshot.head.as_ref(), "head-oid");
    assert_eq!(snapshot.files.len(), 3);
    assert_eq!(snapshot.threads.len(), 2);
    assert_eq!(snapshot.drafts.len(), 1);
    assert_eq!(snapshot.files[0].head_blob.as_deref(), Some("sha-one-head"));
    assert_eq!(snapshot.files[0].base_blob, None);
    assert_eq!(snapshot.files[2].base_blob.as_deref(), Some("sha-old-base"));
    assert_eq!(snapshot.files[2].head_blob, None);
    assert!(matches!(
        snapshot.files[0].patch,
        PatchAvailability::Available(_)
    ));

    let calls = runner.calls.lock().unwrap();
    assert!(calls.iter().any(|spec| args(spec)
        == [
            "api",
            "graphql",
            "--hostname",
            "ghe.acme.test",
            "--input",
            "-"
        ]));
    assert!(calls.iter().any(|spec| args(spec)
        == [
            "api",
            "--hostname",
            "ghe.acme.test",
            "--paginate",
            "--slurp",
            "repos/acme/api/pulls/42/files?per_page=100"
        ]));
    assert!(calls.iter().any(|spec| args(spec)
        == [
            "api",
            "--hostname",
            "ghe.acme.test",
            "-H",
            "Accept:application/vnd.github.v3.diff",
            "repos/acme/api/pulls/42"
        ]));
    let graphql_calls: Vec<_> = calls
        .iter()
        .filter(|spec| args(spec).get(1) == Some(&"graphql".to_owned()))
        .collect();
    assert_eq!(
        graphql_calls.len(),
        2,
        "per-file blob lookups must not happen"
    );
    let first: Value = serde_json::from_slice(graphql_calls[0].stdin.as_ref().unwrap()).unwrap();
    let second: Value = serde_json::from_slice(graphql_calls[1].stdin.as_ref().unwrap()).unwrap();
    assert!(first.get("query").is_some() && first.get("variables").is_some());
    assert_eq!(
        first["query"].as_str().unwrap().matches("diffSide").count(),
        1
    );
    assert_eq!(first["variables"]["cursor"], Value::Null);
    assert_eq!(second["variables"]["cursor"], "cursor-1");
}

#[tokio::test]
async fn snapshot_fetches_overlap_instead_of_running_sequentially() {
    let delay = Duration::from_millis(50);
    let runner = Arc::new(RoutingRunner::new().with_delay(delay));
    let provider = GitHubProvider::new(runner);

    let started = std::time::Instant::now();
    provider.load(&github_key()).await.unwrap();
    let elapsed = started.elapsed();

    // 4 sequential calls would take >= 200ms; thread pages must stay
    // sequential (pagination) but files and diff overlap with them.
    assert!(
        elapsed < Duration::from_millis(175),
        "load took {elapsed:?}, fetches are not overlapping"
    );
}

#[tokio::test]
async fn maps_malformed_json_and_authentication_errors() {
    struct MalformedRunner;
    #[async_trait]
    impl CommandRunner for MalformedRunner {
        async fn run(&self, _spec: CommandSpec) -> Result<CommandOutput, CommandError> {
            Ok(ok(b"{".to_vec()))
        }
    }
    let provider = GitHubProvider::new(Arc::new(MalformedRunner));
    assert!(matches!(
        provider.load(&github_key()).await,
        Err(ProviderError::MalformedResponse { .. })
    ));

    let provider = GitHubProvider::new(Arc::new(RoutingRunner::failing()));
    assert!(matches!(
        provider.probe("ghe.acme.test").await,
        Err(ProviderError::Authentication { .. })
    ));
}

#[tokio::test]
async fn lists_open_pull_requests_in_one_call() {
    let runner = Arc::new(RoutingRunner::new());
    let provider = GitHubProvider::new(runner.clone());

    let list = provider
        .list_open("ghe.acme.test", "acme/api")
        .await
        .unwrap();

    assert_eq!(list.len(), 2);
    assert_eq!(list[0].number, 7);
    assert_eq!(list[0].source_branch, "feature/picker");
    assert_eq!(list[0].author, "jsjunior");
    assert!(!list[0].draft);
    assert_eq!(list[0].description, "Adds the review picker prefetch flow.");
    assert!(list[0].reviewed_current_head());
    assert_eq!(list[1].author, "unknown");
    assert!(list[1].draft);
    assert_eq!(list[1].description, "");
    assert!(!list[1].reviewed_current_head());
    let calls = runner.calls.lock().unwrap();
    assert_eq!(
        calls
            .iter()
            .filter(|spec| args(spec).get(1) == Some(&"graphql".to_owned()))
            .count(),
        1
    );
}

fn github_key() -> ChangeRequestKey {
    ChangeRequestKey {
        provider: ProviderKind::GitHub,
        host: "ghe.acme.test".into(),
        repository: "acme/api".into(),
        number: 42,
    }
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("tests/fixtures/github/{name}")).unwrap()
}

fn ok(stdout: Vec<u8>) -> CommandOutput {
    CommandOutput {
        status: 0,
        stdout,
        stderr: Vec::new(),
    }
}

fn args(spec: &CommandSpec) -> Vec<String> {
    spec.args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

#[tokio::test]
async fn reads_file_contents_at_a_revision() {
    let runner = Arc::new(RoutingRunner::new());
    let provider = GitHubProvider::new(runner.clone());
    let key = github_key();

    let contents = provider
        .read_file(
            &key,
            &RepoPath("src/nested/my file.rs".into()),
            &CommitOid("deadbeef".into()),
        )
        .await
        .unwrap();

    assert_eq!(contents, "fn example() {}\n");
    let calls = runner.calls.lock().unwrap();
    assert!(calls.iter().any(|spec| args(spec)
        == [
            "api",
            "--hostname",
            "ghe.acme.test",
            "-H",
            "Accept:application/vnd.github.raw",
            "repos/acme/api/contents/src/nested/my%20file.rs?ref=deadbeef",
        ]));
}

#[tokio::test]
async fn falls_back_to_rest_patches_when_the_raw_diff_is_too_large() {
    let runner = Arc::new(RoutingRunner::new().with_oversized_diff());
    let provider = GitHubProvider::new(runner);

    let snapshot = provider.load(&github_key()).await.unwrap();

    // The REST files fixture carries "available" as each file's patch body.
    assert!(matches!(
        &snapshot.files[0].patch,
        PatchAvailability::Available(patch) if patch == "available"
    ));
    assert_eq!(snapshot.files.len(), 3);
}

#[tokio::test]
async fn own_pull_request_disables_approve_and_request_changes() {
    use betterreview::domain::{ReviewOutcome, Support};
    // The fixture's viewer login matches the PR author (alice).
    let runner = Arc::new(RoutingRunner::new());
    let provider = GitHubProvider::new(runner);

    let snapshot = provider.load(&github_key()).await.unwrap();

    assert!(matches!(
        snapshot.capabilities.for_outcome(ReviewOutcome::Approve),
        Support::Unsupported { .. }
    ));
    assert!(matches!(
        snapshot
            .capabilities
            .for_outcome(ReviewOutcome::RequestChanges),
        Support::Unsupported { .. }
    ));
    assert!(matches!(
        snapshot.capabilities.for_outcome(ReviewOutcome::Comment),
        Support::Supported
    ));
}

#[tokio::test]
async fn a_multi_line_draft_keeps_its_range_when_the_review_is_reopened() {
    let provider = GitHubProvider::new(Arc::new(RoutingRunner::new()));

    let snapshot = provider.load(&github_key()).await.unwrap();

    let selection = snapshot.drafts[0]
        .selection
        .as_ref()
        .expect("the draft is anchored");
    assert_eq!(
        (selection.start.line, selection.end.line),
        (1, 3),
        "collapsing the range to its last line is what moves the marking on reopen"
    );
}
