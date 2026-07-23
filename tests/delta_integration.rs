use betterreview::{
    diff::{DeltaRenderer, DiffRenderer, parse_file_patch},
    domain::{ChangedFile, CommitOid, FileStatus, PatchAvailability, RepoPath},
    process::TokioCommandRunner,
};
use std::{process::Command, sync::Arc};

#[tokio::test]
async fn real_delta_preserves_canonical_rows_and_bindings() {
    let status = Command::new("delta")
        .arg("--version")
        .status()
        .expect("delta executable is required for this integration test");
    assert!(status.success(), "delta --version must succeed");
    let renderer = DeltaRenderer::new(Arc::new(TokioCommandRunner));

    for (name, path, width) in [
        ("modified.diff", "src/lib.rs", 80),
        ("unicode.diff", "README.md", 80),
        ("multiple-hunks.diff", "src/lib.rs", 80),
        ("long-line.diff", "src/long.rs", 40),
    ] {
        let patch = std::fs::read_to_string(format!("tests/fixtures/patches/{name}")).unwrap();
        let parsed = parsed_patch(path, &patch);

        let rendered = renderer
            .render(patch.as_bytes(), &parsed, width)
            .await
            .unwrap();

        assert_eq!(rendered.rows.len(), parsed.rows.len(), "fixture {name}");
        for (index, row) in rendered.rows.iter().enumerate() {
            assert_eq!(row.binding.row_index, index, "fixture {name}");
            assert_eq!(row.binding.left, parsed.rows[index].left, "fixture {name}");
            assert_eq!(
                row.binding.right, parsed.rows[index].right,
                "fixture {name}"
            );
        }
    }
}

fn parsed_patch(path: &str, patch: &str) -> betterreview::diff::ParsedFileDiff {
    let file = ChangedFile {
        path: RepoPath(path.into()),
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
