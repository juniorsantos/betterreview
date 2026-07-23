use async_trait::async_trait;
use betterreview::{
    diff::{DeltaError, DeltaRenderer, DiffError, DiffRenderer, MAX_PATCH_BYTES, parse_file_patch},
    domain::{ChangedFile, CommitOid, FileStatus, PatchAvailability, RepoPath},
    process::{CommandError, CommandOutput, CommandRunner, CommandSpec},
};
use std::{io, sync::Arc, time::Duration};

enum RunnerResult {
    Output(CommandOutput),
    Missing,
    Timeout,
}

struct FakeRunner(RunnerResult);

#[async_trait]
impl CommandRunner for FakeRunner {
    async fn run(&self, spec: CommandSpec) -> Result<CommandOutput, CommandError> {
        match &self.0 {
            RunnerResult::Output(output) => Ok(output.clone()),
            RunnerResult::Missing => Err(CommandError::Spawn {
                program: spec.program,
                source: io::Error::new(io::ErrorKind::NotFound, "missing delta"),
            }),
            RunnerResult::Timeout => Err(CommandError::Timeout {
                timeout: spec.timeout,
            }),
        }
    }
}

#[tokio::test]
async fn reports_missing_delta() {
    let (patch, parsed) = parsed_fixture();
    let renderer = DeltaRenderer::new(Arc::new(FakeRunner(RunnerResult::Missing)));
    assert!(matches!(
        renderer.render(patch.as_bytes(), &parsed, 80).await,
        Err(DeltaError::Missing)
    ));
}

#[tokio::test]
async fn reports_timeout() {
    let (patch, parsed) = parsed_fixture();
    let renderer = DeltaRenderer::new(Arc::new(FakeRunner(RunnerResult::Timeout)));
    assert!(matches!(
        renderer.render(patch.as_bytes(), &parsed, 80).await,
        Err(DeltaError::Timeout(timeout)) if timeout == Duration::from_secs(60)
    ));
}

#[tokio::test]
async fn redacts_sensitive_stderr_from_nonzero_exit() {
    let (patch, parsed) = parsed_fixture();
    let renderer = DeltaRenderer::new(Arc::new(FakeRunner(RunnerResult::Output(CommandOutput {
        status: 1,
        stdout: Vec::new(),
        stderr: b"Authorization: Bearer secret-token\nnormal failure\n".to_vec(),
    }))));

    let error = renderer
        .render(patch.as_bytes(), &parsed, 80)
        .await
        .unwrap_err();

    match error {
        DeltaError::Failed { status, stderr } => {
            assert_eq!(status, 1);
            assert!(!stderr.contains("secret-token"));
            assert!(stderr.contains("[REDACTED]"));
            assert!(stderr.contains("normal failure"));
        }
        error => panic!("unexpected error: {error}"),
    }
}

#[tokio::test]
async fn rejects_structural_line_count_change() {
    let (patch, parsed) = parsed_fixture();
    let shortened = patch.lines().skip(1).collect::<Vec<_>>().join("\n");
    let renderer = DeltaRenderer::new(Arc::new(FakeRunner(RunnerResult::Output(CommandOutput {
        status: 0,
        stdout: shortened.into_bytes(),
        stderr: Vec::new(),
    }))));

    assert!(matches!(
        renderer.render(patch.as_bytes(), &parsed, 80).await,
        Err(DeltaError::StructureChanged { expected, actual })
            if expected == parsed.rows.len() && actual + 1 == expected
    ));
}

#[tokio::test]
async fn rejects_invalid_utf8_from_delta() {
    let (patch, parsed) = parsed_fixture();
    let renderer = DeltaRenderer::new(Arc::new(FakeRunner(RunnerResult::Output(CommandOutput {
        status: 0,
        stdout: b"invalid\xff".to_vec(),
        stderr: Vec::new(),
    }))));
    assert!(matches!(
        renderer.render(patch.as_bytes(), &parsed, 80).await,
        Err(DeltaError::InvalidUtf8)
    ));
}

#[test]
fn unavailable_and_oversized_patches_keep_explicit_diagnostics() {
    for availability in [
        PatchAvailability::Binary,
        PatchAvailability::Collapsed,
        PatchAvailability::Truncated {
            reason: "provider truncated patch".into(),
        },
    ] {
        let file = file_with_patch(availability);
        assert!(matches!(
            parse_file_patch(&file, &CommitOid("head".into())),
            Err(DiffError::PatchUnavailable { .. })
        ));
    }

    let file = file_with_patch(PatchAvailability::Available(
        "+".repeat(MAX_PATCH_BYTES + 1),
    ));
    assert!(matches!(
        parse_file_patch(&file, &CommitOid("head".into())),
        Err(DiffError::PatchTooLarge { .. })
    ));
}

fn parsed_fixture() -> (String, betterreview::diff::ParsedFileDiff) {
    let patch = std::fs::read_to_string("tests/fixtures/patches/modified.diff").unwrap();
    let file = file_with_patch(PatchAvailability::Available(patch.clone()));
    let parsed = parse_file_patch(&file, &CommitOid("head".into())).unwrap();
    (patch, parsed)
}

fn file_with_patch(patch: PatchAvailability) -> ChangedFile {
    ChangedFile {
        path: RepoPath("src/lib.rs".into()),
        previous_path: None,
        status: FileStatus::Modified,
        additions: 0,
        deletions: 0,
        patch,
        base_blob: None,
        head_blob: None,
        remotely_reviewed: None,
    }
}
