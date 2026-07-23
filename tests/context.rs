use async_trait::async_trait;
use betterreview::{
    context::{
        ContextError, ContextResolver, DiscoveryInput, ResolveRequest, parse_change_request_url,
        parse_remote_url,
    },
    domain::{ChangeRequestKey, ProviderKind},
    process::{CommandError, CommandOutput, CommandRunner, CommandSpec},
};
use rstest::rstest;
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[rstest]
#[case(
    "https://github.com/acme/api/pull/12",
    "github.com",
    "acme/api",
    12,
    ProviderKind::GitHub
)]
#[case(
    "https://ghe.acme.test/acme/api/pull/13",
    "ghe.acme.test",
    "acme/api",
    13,
    ProviderKind::GitHub
)]
#[case(
    "https://gitlab.com/acme/api/-/merge_requests/14",
    "gitlab.com",
    "acme/api",
    14,
    ProviderKind::GitLab
)]
#[case(
    "https://git.acme.test/group/sub/api/-/merge_requests/15",
    "git.acme.test",
    "group/sub/api",
    15,
    ProviderKind::GitLab
)]
fn parses_change_request_urls(
    #[case] input: &str,
    #[case] host: &str,
    #[case] repo: &str,
    #[case] number: u64,
    #[case] provider: ProviderKind,
) {
    let parsed = parse_change_request_url(input).unwrap();
    assert_eq!(
        parsed,
        ChangeRequestKey {
            provider,
            host: host.into(),
            repository: repo.into(),
            number,
        }
    );
}

#[rstest]
#[case("git@github.com:acme/api.git", "github.com", "acme/api")]
#[case("ssh://git@git.acme.test/group/api.git", "git.acme.test", "group/api")]
#[case("https://gitlab.com/acme/api.git", "gitlab.com", "acme/api")]
fn parses_git_remotes(#[case] input: &str, #[case] host: &str, #[case] repo: &str) {
    let parsed = parse_remote_url(input).unwrap();
    assert_eq!(
        (parsed.host.as_str(), parsed.repository.as_str()),
        (host, repo)
    );
}

#[derive(Debug)]
struct ExpectedCommand {
    program: &'static str,
    args: Vec<&'static str>,
    output: CommandOutput,
}

#[derive(Debug)]
struct FakeRunner {
    commands: Mutex<VecDeque<ExpectedCommand>>,
}

impl FakeRunner {
    fn new(commands: impl IntoIterator<Item = ExpectedCommand>) -> Self {
        Self {
            commands: Mutex::new(commands.into_iter().collect()),
        }
    }

    fn assert_consumed(&self) {
        assert!(self.commands.lock().unwrap().is_empty());
    }
}

#[async_trait]
impl CommandRunner for FakeRunner {
    async fn run(&self, spec: CommandSpec) -> Result<CommandOutput, CommandError> {
        let expected = self.commands.lock().unwrap().pop_front().unwrap();
        assert_eq!(spec.program, PathBuf::from(expected.program));
        assert_eq!(
            spec.args,
            expected
                .args
                .into_iter()
                .map(std::ffi::OsString::from)
                .collect::<Vec<_>>()
        );
        Ok(expected.output)
    }
}

fn output(status: i32, stdout: &str) -> CommandOutput {
    CommandOutput {
        status,
        stdout: stdout.as_bytes().to_vec(),
        stderr: Vec::new(),
    }
}

fn request(target: Option<&str>) -> ResolveRequest {
    ResolveRequest {
        cwd: PathBuf::from("/work"),
        target: target.map(Into::into),
        provider_hint: None,
        host_hint: None,
        repository_hint: None,
    }
}

#[tokio::test]
async fn explicit_url_takes_precedence_without_git_discovery() {
    let runner = Arc::new(FakeRunner::new([]));
    let resolver = ContextResolver::new(runner.clone());

    let context = resolver
        .resolve(request(Some(
            "https://gitlab.com/acme/api/-/merge_requests/14",
        )))
        .await
        .unwrap();

    assert_eq!(
        context.discovery,
        Some(DiscoveryInput::Exact(ChangeRequestKey {
            provider: ProviderKind::GitLab,
            host: "gitlab.com".into(),
            repository: "acme/api".into(),
            number: 14,
        }))
    );
    assert!(!context.show_session_picker);
    runner.assert_consumed();
}

#[tokio::test]
async fn explicit_provider_hint_takes_precedence_over_the_request_url_path() {
    let runner = Arc::new(FakeRunner::new([]));
    let resolver = ContextResolver::new(runner.clone());
    let mut request = request(Some("https://gitlab.com/acme/api/-/merge_requests/14"));
    request.provider_hint = Some(ProviderKind::GitHub);

    let context = resolver.resolve(request).await.unwrap();

    assert_eq!(
        context.discovery,
        Some(DiscoveryInput::Exact(ChangeRequestKey {
            provider: ProviderKind::GitHub,
            host: "gitlab.com".into(),
            repository: "acme/api".into(),
            number: 14,
        }))
    );
    runner.assert_consumed();
}

#[tokio::test]
async fn numeric_target_with_repository_hints_resolves_an_exact_request() {
    let runner = Arc::new(FakeRunner::new([]));
    let resolver = ContextResolver::new(runner.clone());
    let mut request = request(Some("42"));
    request.host_hint = Some("github.com".into());
    request.repository_hint = Some("acme/api".into());

    let context = resolver.resolve(request).await.unwrap();

    assert_eq!(
        context.discovery,
        Some(DiscoveryInput::Exact(ChangeRequestKey {
            provider: ProviderKind::GitHub,
            host: "github.com".into(),
            repository: "acme/api".into(),
            number: 42,
        }))
    );
    runner.assert_consumed();
}

#[tokio::test]
async fn current_branch_uses_the_repository_remote_and_branch_name() {
    let runner = Arc::new(FakeRunner::new([
        ExpectedCommand {
            program: "git",
            args: vec!["-C", "/work", "rev-parse", "--show-toplevel"],
            output: output(0, "/repo\n"),
        },
        ExpectedCommand {
            program: "git",
            args: vec!["-C", "/repo", "remote"],
            output: output(0, "origin\n"),
        },
        ExpectedCommand {
            program: "git",
            args: vec!["-C", "/repo", "remote", "get-url", "origin"],
            output: output(0, "git@github.com:acme/api.git\n"),
        },
        ExpectedCommand {
            program: "git",
            args: vec!["-C", "/repo", "branch", "--show-current"],
            output: output(0, "feature/context\n"),
        },
    ]));
    let resolver = ContextResolver::new(runner.clone());

    let context = resolver.resolve(request(None)).await.unwrap();

    assert_eq!(
        context.discovery,
        Some(DiscoveryInput::CurrentBranch {
            provider: ProviderKind::GitHub,
            host: "github.com".into(),
            repository: "acme/api".into(),
            branch: "feature/context".into(),
        })
    );
    assert_eq!(context.repository_root, Some(PathBuf::from("/repo")));
    assert_eq!(context.remote_name.as_deref(), Some("origin"));
    assert!(!context.show_session_picker);
    runner.assert_consumed();
}

#[tokio::test]
async fn missing_repository_falls_back_to_the_session_picker() {
    let runner = Arc::new(FakeRunner::new([ExpectedCommand {
        program: "git",
        args: vec!["-C", "/work", "rev-parse", "--show-toplevel"],
        output: output(128, ""),
    }]));
    let resolver = ContextResolver::new(runner.clone());

    let context = resolver.resolve(request(None)).await.unwrap();

    assert_eq!(context.discovery, None);
    assert!(context.show_session_picker);
    assert_eq!(context.repository_root, None);
    runner.assert_consumed();
}

#[tokio::test]
async fn ambiguous_private_host_requires_a_provider_hint() {
    let runner = Arc::new(FakeRunner::new([
        ExpectedCommand {
            program: "git",
            args: vec!["-C", "/work", "rev-parse", "--show-toplevel"],
            output: output(0, "/repo\n"),
        },
        ExpectedCommand {
            program: "git",
            args: vec!["-C", "/repo", "remote"],
            output: output(0, "origin\n"),
        },
        ExpectedCommand {
            program: "git",
            args: vec!["-C", "/repo", "remote", "get-url", "origin"],
            output: output(0, "ssh://git@git.acme.test/group/api.git\n"),
        },
        ExpectedCommand {
            program: "gh",
            args: vec!["auth", "status", "--hostname", "git.acme.test"],
            output: output(0, ""),
        },
        ExpectedCommand {
            program: "glab",
            args: vec!["auth", "status", "--hostname", "git.acme.test"],
            output: output(0, ""),
        },
    ]));
    let resolver = ContextResolver::new(runner.clone());

    let error = resolver.resolve(request(None)).await.unwrap_err();

    assert!(matches!(
        error,
        ContextError::AmbiguousProvider { ref host } if host == "git.acme.test"
    ));
    assert!(error.to_string().contains("--provider"));
    runner.assert_consumed();
}
