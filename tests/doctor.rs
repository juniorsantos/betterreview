use assert_cmd::Command as AssertCommand;
use async_trait::async_trait;
use betterreview::{
    doctor::{DependencyStatus, Doctor},
    domain::ProviderKind,
    process::{CommandError, CommandOutput, CommandRunner, CommandSpec},
};
use predicates::str::contains;
use std::{collections::BTreeMap, io, sync::Arc};

enum FakeResponse {
    Output(CommandOutput),
    Missing,
}

#[derive(Default)]
struct FakeRunner {
    responses: BTreeMap<String, FakeResponse>,
}

impl FakeRunner {
    fn with(mut self, program: &str, args: &[&str], status: i32, stdout: &str) -> Self {
        self.responses.insert(
            command_key(program, args),
            FakeResponse::Output(CommandOutput {
                status,
                stdout: stdout.as_bytes().to_vec(),
                stderr: Vec::new(),
            }),
        );
        self
    }

    fn missing(mut self, program: &str, args: &[&str]) -> Self {
        self.responses
            .insert(command_key(program, args), FakeResponse::Missing);
        self
    }
}

#[async_trait]
impl CommandRunner for FakeRunner {
    async fn run(&self, spec: CommandSpec) -> Result<CommandOutput, CommandError> {
        let args: Vec<_> = spec
            .args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        let key = command_key(
            spec.program.to_string_lossy().as_ref(),
            &args.iter().map(String::as_str).collect::<Vec<_>>(),
        );
        match self.responses.get(&key) {
            Some(FakeResponse::Output(output)) => Ok(output.clone()),
            Some(FakeResponse::Missing) | None => Err(CommandError::Spawn {
                program: spec.program,
                source: io::Error::new(io::ErrorKind::NotFound, "missing fixture"),
            }),
        }
    }
}

fn command_key(program: &str, args: &[&str]) -> String {
    std::iter::once(program)
        .chain(args.iter().copied())
        .collect::<Vec<_>>()
        .join("\0")
}

#[tokio::test]
async fn reports_dependency_and_authentication_statuses() {
    let runner = FakeRunner::default()
        .with("git", &["--version"], 0, "git version 2.39.5\n")
        .with("delta", &["--version"], 0, "delta 0.19.2\n")
        .with("gh", &["--version"], 0, "gh version 2.96.0\n")
        .with("gh", &["api", "--help"], 0, "--input file\n")
        .with("gh", &["auth", "status", "--hostname", "github.com"], 1, "")
        .with("glab", &["--version"], 0, "glab 1.108.0\n")
        .with("glab", &["api", "--help"], 0, "--input string\n")
        .with(
            "glab",
            &["auth", "status", "--hostname", "gitlab.com"],
            1,
            "",
        );
    let doctor = Doctor::new(Arc::new(runner));

    let report = doctor.check(None, None).await;

    assert_eq!(
        report.requirement("delta").unwrap().status,
        DependencyStatus::Ready
    );
    assert_eq!(
        report.requirement("gh").unwrap().guidance,
        "Run: gh auth login --hostname github.com"
    );
    assert_eq!(
        report.requirement("glab").unwrap().guidance,
        "Run: glab auth login --hostname gitlab.com"
    );
    assert!(!report.is_ready());
}

#[tokio::test]
async fn reports_missing_executable() {
    let runner = FakeRunner::default().missing("git", &["--version"]);
    let doctor = Doctor::new(Arc::new(runner));

    let report = doctor.check(Some(ProviderKind::GitHub), None).await;

    assert_eq!(
        report.requirement("git").unwrap().status,
        DependencyStatus::Missing
    );
}

#[tokio::test]
async fn rejects_old_dependency_version() {
    let runner = FakeRunner::default().with("delta", &["--version"], 0, "delta 0.18.2\n");
    let doctor = Doctor::new(Arc::new(runner));

    let report = doctor.check(Some(ProviderKind::GitHub), None).await;

    assert_eq!(
        report.requirement("delta").unwrap().status,
        DependencyStatus::Unsupported
    );
    assert_eq!(
        report.requirement("delta").unwrap().detected_version,
        Some(semver::Version::new(0, 18, 2))
    );
}

#[tokio::test]
async fn rejects_provider_cli_without_input_flag() {
    let runner = FakeRunner::default()
        .with("gh", &["--version"], 0, "gh version 2.96.0\n")
        .with("gh", &["api", "--help"], 0, "GitHub API help\n");
    let doctor = Doctor::new(Arc::new(runner));

    let report = doctor.check(Some(ProviderKind::GitHub), None).await;

    assert_eq!(
        report.requirement("gh").unwrap().status,
        DependencyStatus::Unsupported
    );
}

#[tokio::test]
async fn all_ready_checks_produce_ready_report() {
    let runner = FakeRunner::default()
        .with("git", &["--version"], 0, "git version 2.39.5\n")
        .with("delta", &["--version"], 0, "delta 0.19.2\n")
        .with("gh", &["--version"], 0, "gh version 2.96.0\n")
        .with("gh", &["api", "--help"], 0, "--input file\n")
        .with(
            "gh",
            &["auth", "status", "--hostname", "ghe.acme.test"],
            0,
            "authenticated\n",
        );
    let doctor = Doctor::new(Arc::new(runner));

    let report = doctor
        .check(Some(ProviderKind::GitHub), Some("ghe.acme.test"))
        .await;

    assert!(report.is_ready());
}

#[test]
fn sessions_command_does_not_succeed_without_runtime() {
    AssertCommand::cargo_bin("betterreview")
        .unwrap()
        .arg("sessions")
        .assert()
        .failure()
        .stderr(contains("application runtime is not installed"));
}
