use async_trait::async_trait;
use betterreview::{
    domain::{ChangeRequestKey, PatchAvailability, ProviderKind},
    process::{CommandError, CommandOutput, CommandRunner, CommandSpec},
    providers::{GitHubProvider, ProviderError, ReviewProvider},
};
use serde_json::Value;
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
async fn loads_paginated_github_snapshot_with_structured_commands() {
    let page_1 = fixture("change_request.json");
    let page_2 = fixture("threads-page-2.json");
    let files = fixture("files-page-1.json");
    let diff = std::fs::read("tests/fixtures/github/pull.diff").unwrap();
    let responses = vec![ok(page_1), ok(page_2), ok(files), ok(diff)];
    let runner = Arc::new(RecordingRunner::new(responses));
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
            "repos/acme/api/pulls/42/files"
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
async fn maps_malformed_json_and_authentication_errors() {
    let malformed = Arc::new(RecordingRunner::new(vec![ok(b"{".to_vec())]));
    let provider = GitHubProvider::new(malformed);
    assert!(matches!(
        provider.load(&github_key()).await,
        Err(ProviderError::MalformedResponse { .. })
    ));

    let auth = Arc::new(RecordingRunner::new(vec![CommandOutput {
        status: 1,
        stdout: Vec::new(),
        stderr: b"authentication required; run gh auth login".to_vec(),
    }]));
    let provider = GitHubProvider::new(auth);
    assert!(matches!(
        provider.probe("ghe.acme.test").await,
        Err(ProviderError::Authentication { .. })
    ));
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
