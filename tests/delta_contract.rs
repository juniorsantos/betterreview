use async_trait::async_trait;
use betterreview::{
    diff::{DeltaRenderer, DiffRenderer, DiffRowKind, parse_file_patch},
    domain::{ChangedFile, CommitOid, FileStatus, PatchAvailability, RepoPath},
    process::{CommandError, CommandOutput, CommandRunner, CommandSpec},
};
use std::{
    ffi::OsString,
    sync::{Arc, Mutex},
};

struct RecordingRunner {
    spec: Mutex<Option<CommandSpec>>,
    output: Vec<u8>,
}

#[async_trait]
impl CommandRunner for RecordingRunner {
    async fn run(&self, spec: CommandSpec) -> Result<CommandOutput, CommandError> {
        *self.spec.lock().unwrap() = Some(spec);
        Ok(CommandOutput {
            status: 0,
            stdout: self.output.clone(),
            stderr: Vec::new(),
        })
    }
}

#[tokio::test]
async fn invokes_delta_with_structure_preserving_arguments() {
    let patch = std::fs::read_to_string("tests/fixtures/patches/modified.diff").unwrap();
    let parsed = parsed_patch(&patch);
    let runner = Arc::new(RecordingRunner {
        spec: Mutex::new(None),
        output: patch.as_bytes().to_vec(),
    });
    let renderer = DeltaRenderer::new(runner.clone());

    let rendered = renderer
        .render(patch.as_bytes(), &parsed, 80)
        .await
        .unwrap();

    let stored = runner.spec.lock().unwrap();
    let spec = stored.as_ref().unwrap();
    assert_eq!(spec.program, std::path::PathBuf::from("delta"));
    assert_eq!(
        spec.args,
        vec![
            "--paging=never",
            "--color-only",
            "--detect-dark-light=never",
            "--max-line-length=0",
        ]
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>()
    );
    assert_eq!(spec.stdin.as_deref(), Some(patch.as_bytes()));
    assert_eq!(rendered.rows.len(), parsed.rows.len());
    let added_index = parsed
        .rows
        .iter()
        .position(|row| row.kind == DiffRowKind::Added)
        .unwrap();
    assert_eq!(rendered.rows[added_index].binding.row_index, added_index);
    assert_eq!(
        rendered.rows[added_index].binding.right,
        parsed.rows[added_index].right
    );
}

fn parsed_patch(patch: &str) -> betterreview::diff::ParsedFileDiff {
    let file = ChangedFile {
        path: RepoPath("src/lib.rs".into()),
        previous_path: None,
        status: FileStatus::Modified,
        additions: 0,
        deletions: 0,
        patch: PatchAvailability::Available(patch.into()),
        base_blob: None,
        head_blob: None,
        remotely_reviewed: None,
    };
    parse_file_patch(&file, &CommitOid("head".into())).unwrap()
}
