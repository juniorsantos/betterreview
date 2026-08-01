use betterreview::{
    diff::{DiffError, DiffRowKind, MAX_PATCH_BYTES, parse_file_patch},
    domain::{ChangedFile, CommitOid, DiffSide, FileStatus, PatchAvailability, RepoPath},
};

#[test]
fn maps_added_deleted_and_context_lines() {
    let file = fixture_file("modified.diff", FileStatus::Modified);
    let parsed = parse_file_patch(&file, &CommitOid("head-1".into())).unwrap();
    let removed = parsed
        .rows
        .iter()
        .find(|row| row.kind == DiffRowKind::Removed)
        .unwrap();
    let added = parsed
        .rows
        .iter()
        .find(|row| row.kind == DiffRowKind::Added)
        .unwrap();
    let context = parsed
        .rows
        .iter()
        .find(|row| row.kind == DiffRowKind::Context)
        .unwrap();

    assert!(removed.left.is_some());
    assert!(removed.right.is_none());
    assert_eq!(removed.left.as_ref().unwrap().side, DiffSide::Left);
    assert_eq!(removed.left.as_ref().unwrap().old_line, removed.old_line);
    assert_eq!(removed.left.as_ref().unwrap().new_line, Some(2));
    assert!(added.left.is_none());
    assert_eq!(added.right.as_ref().unwrap().side, DiffSide::Right);
    assert_eq!(added.right.as_ref().unwrap().old_line, Some(3));
    assert_eq!(added.right.as_ref().unwrap().new_line, added.new_line);
    assert!(context.left.is_some() && context.right.is_some());
}

#[test]
fn preserves_hunk_identity_across_multiple_hunks() {
    let parsed = parse_file_patch(
        &fixture_file("multiple-hunks.diff", FileStatus::Modified),
        &CommitOid("head-2".into()),
    )
    .unwrap();
    assert_eq!(parsed.hunks.len(), 2);
    assert_eq!(parsed.hunks[0].id, 0);
    assert_eq!(parsed.hunks[1].id, 1);
    assert_ne!(parsed.hunks[0].row_range, parsed.hunks[1].row_range);
    assert_eq!(
        parsed.rows[parsed.hunks[0].row_range.start].old_line,
        Some(1)
    );
    assert_eq!(
        parsed.rows[parsed.hunks[1].row_range.start].old_line,
        Some(10)
    );
}

#[test]
fn added_file_has_only_right_positions() {
    let parsed = parse_file_patch(
        &fixture_file("added.diff", FileStatus::Added),
        &CommitOid("head-added".into()),
    )
    .unwrap();
    let changed: Vec<_> = parsed
        .rows
        .iter()
        .filter(|row| row.kind == DiffRowKind::Added)
        .collect();
    assert_eq!(changed.len(), 2);
    assert!(
        changed
            .iter()
            .all(|row| row.left.is_none() && row.right.is_some())
    );
}

#[test]
fn deleted_file_has_only_left_positions() {
    let parsed = parse_file_patch(
        &fixture_file("deleted.diff", FileStatus::Deleted),
        &CommitOid("head-deleted".into()),
    )
    .unwrap();
    let changed: Vec<_> = parsed
        .rows
        .iter()
        .filter(|row| row.kind == DiffRowKind::Removed)
        .collect();
    assert_eq!(changed.len(), 2);
    assert!(
        changed
            .iter()
            .all(|row| row.left.is_some() && row.right.is_none())
    );
}

#[test]
fn renamed_file_positions_use_current_path() {
    let parsed = parse_file_patch(
        &fixture_file("renamed.diff", FileStatus::Renamed),
        &CommitOid("head-renamed".into()),
    )
    .unwrap();
    assert_eq!(parsed.path, RepoPath("src/new_name.rs".into()));
    assert!(
        parsed
            .rows
            .iter()
            .filter_map(|row| row.left.as_ref())
            .all(|position| { position.path == RepoPath("src/new_name.rs".into()) })
    );
}

#[test]
fn preserves_unicode_text() {
    let parsed = parse_file_patch(
        &fixture_file("unicode.diff", FileStatus::Modified),
        &CommitOid("head-unicode".into()),
    )
    .unwrap();
    assert!(parsed.rows.iter().any(|row| row.raw.contains("café ☕")));
    assert!(parsed.rows.iter().any(|row| row.raw.contains("日本語")));
}

#[test]
fn no_newline_marker_is_metadata_without_position() {
    let parsed = parse_file_patch(
        &fixture_file("no-newline.diff", FileStatus::Modified),
        &CommitOid("head-no-newline".into()),
    )
    .unwrap();
    let markers: Vec<_> = parsed
        .rows
        .iter()
        .filter(|row| row.raw == "\\ No newline at end of file")
        .collect();
    assert_eq!(markers.len(), 2);
    assert!(markers.iter().all(|row| {
        row.kind == DiffRowKind::Metadata && row.left.is_none() && row.right.is_none()
    }));
}

#[test]
fn rejects_malformed_hunk_header() {
    let file = available_file("@@ -one +1 @@\n-old\n+new\n");
    assert!(matches!(
        parse_file_patch(&file, &CommitOid("head".into())),
        Err(DiffError::MalformedHunk { .. })
    ));
}

#[test]
fn rejects_hunk_counter_underflow() {
    let file = available_file("@@ -1 +1 @@\n one\n extra\n");
    assert!(matches!(
        parse_file_patch(&file, &CommitOid("head".into())),
        Err(DiffError::HunkCountMismatch { .. })
    ));
}

#[test]
fn rejects_patch_larger_than_limit() {
    let patch = "+".repeat(MAX_PATCH_BYTES + 1);
    let file = available_file(&patch);
    assert!(matches!(
        parse_file_patch(&file, &CommitOid("head".into())),
        Err(DiffError::PatchTooLarge { .. })
    ));
}

fn fixture_file(name: &str, status: FileStatus) -> ChangedFile {
    let path = match status {
        FileStatus::Added => "src/new.rs",
        FileStatus::Deleted => "src/old.rs",
        FileStatus::Renamed => "src/new_name.rs",
        _ if name == "unicode.diff" => "README.md",
        _ => "src/lib.rs",
    };
    let previous_path = (status == FileStatus::Renamed).then(|| RepoPath("src/old_name.rs".into()));
    ChangedFile {
        path: RepoPath(path.into()),
        previous_path,
        status,
        additions: 0,
        deletions: 0,
        patch: PatchAvailability::Available(
            std::fs::read_to_string(format!("tests/fixtures/patches/{name}")).unwrap(),
        ),
        base_blob: None,
        head_blob: None,
        remotely_reviewed: None,
    }
}

fn available_file(patch: &str) -> ChangedFile {
    ChangedFile {
        path: RepoPath("src/lib.rs".into()),
        previous_path: None,
        status: FileStatus::Modified,
        additions: 0,
        deletions: 0,
        patch: PatchAvailability::Available(patch.into()),
        base_blob: None,
        head_blob: None,
        remotely_reviewed: None,
    }
}

fn file_with(patch: &str) -> ChangedFile {
    ChangedFile {
        path: RepoPath("src/lib.rs".into()),
        previous_path: None,
        status: FileStatus::Modified,
        additions: 1,
        deletions: 1,
        patch: PatchAvailability::Available(patch.to_owned()),
        base_blob: None,
        head_blob: Some("head-blob".into()),
        remotely_reviewed: Some(false),
    }
}

#[test]
fn a_hunk_keeps_the_enclosing_section_git_puts_after_the_ranges() {
    let file = file_with("@@ -1,2 +1,2 @@ fn resolve_remote()\n context\n-old\n+new\n");

    let parsed = parse_file_patch(&file, &CommitOid("head-1".into())).unwrap();

    assert_eq!(
        parsed.hunks[0].section.as_deref(),
        Some("fn resolve_remote()"),
        "git computes the enclosing function for us; throwing it away is the bug"
    );
}

#[test]
fn a_hunk_without_a_section_carries_none_rather_than_an_empty_string() {
    let file = file_with("@@ -1,2 +1,2 @@\n context\n-old\n+new\n");

    let parsed = parse_file_patch(&file, &CommitOid("head-1".into())).unwrap();

    assert_eq!(parsed.hunks[0].section, None);
}
