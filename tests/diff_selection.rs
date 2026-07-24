use betterreview::{
    diff::{
        DiffCursor, DiffRowKind, ParsedFileDiff, SelectionError, parse_file_patch,
        validate_selection,
    },
    domain::{ChangedFile, CommitOid, DiffSide, FileStatus, PatchAvailability, RepoPath},
};

#[test]
fn accepts_right_side_range_inside_one_hunk() {
    let diff = parsed_fixture("modified.diff", FileStatus::Modified);
    let selection = validate_selection(
        &diff,
        DiffCursor {
            row: 7,
            side: DiffSide::Right,
        },
        DiffCursor {
            row: 9,
            side: DiffSide::Right,
        },
    )
    .unwrap();
    assert_eq!(selection.start.side, DiffSide::Right);
    assert!(selection.start.line <= selection.end.line);
}

#[test]
fn rejects_cross_hunk_range() {
    let diff = parsed_fixture("multiple-hunks.diff", FileStatus::Modified);
    let error = validate_selection(
        &diff,
        DiffCursor {
            row: diff.hunks[0].row_range.start,
            side: DiffSide::Right,
        },
        DiffCursor {
            row: diff.hunks[1].row_range.start,
            side: DiffSide::Right,
        },
    )
    .unwrap_err();
    assert_eq!(error, SelectionError::DifferentHunks);
}

#[test]
fn accepts_single_added_line() {
    let diff = parsed_fixture("added.diff", FileStatus::Added);
    let row = row_with_kind(&diff, DiffRowKind::Added);
    let selection = validate_selection(
        &diff,
        DiffCursor {
            row,
            side: DiffSide::Right,
        },
        DiffCursor {
            row,
            side: DiffSide::Right,
        },
    )
    .unwrap();
    assert_eq!(selection.start, selection.end);
}

#[test]
fn accepts_single_deleted_line() {
    let diff = parsed_fixture("deleted.diff", FileStatus::Deleted);
    let row = row_with_kind(&diff, DiffRowKind::Removed);
    let selection = validate_selection(
        &diff,
        DiffCursor {
            row,
            side: DiffSide::Left,
        },
        DiffCursor {
            row,
            side: DiffSide::Left,
        },
    )
    .unwrap();
    assert_eq!(selection.start, selection.end);
}

#[test]
fn normalizes_reversed_cursor_order() {
    let diff = parsed_fixture("modified.diff", FileStatus::Modified);
    let selection = validate_selection(
        &diff,
        DiffCursor {
            row: 9,
            side: DiffSide::Right,
        },
        DiffCursor {
            row: 7,
            side: DiffSide::Right,
        },
    )
    .unwrap();
    assert!(selection.start.line < selection.end.line);
}

#[test]
fn rejects_range_crossing_sides() {
    let diff = parsed_fixture("modified.diff", FileStatus::Modified);
    let error = validate_selection(
        &diff,
        DiffCursor {
            row: 5,
            side: DiffSide::Left,
        },
        DiffCursor {
            row: 5,
            side: DiffSide::Right,
        },
    )
    .unwrap_err();
    assert_eq!(error, SelectionError::DifferentSides);
}

#[test]
fn rejects_header_and_metadata_rows() {
    let header = parsed_fixture("modified.diff", FileStatus::Modified);
    let error = validate_selection(
        &header,
        DiffCursor {
            row: 0,
            side: DiffSide::Right,
        },
        DiffCursor {
            row: 0,
            side: DiffSide::Right,
        },
    )
    .unwrap_err();
    assert_eq!(error, SelectionError::NotCommentable);

    let metadata = parsed_fixture("no-newline.diff", FileStatus::Modified);
    let row = row_with_kind(&metadata, DiffRowKind::Metadata);
    let error = validate_selection(
        &metadata,
        DiffCursor {
            row,
            side: DiffSide::Right,
        },
        DiffCursor {
            row,
            side: DiffSide::Right,
        },
    )
    .unwrap_err();
    assert_eq!(error, SelectionError::NotCommentable);
}

#[test]
fn rejects_range_missing_right_position() {
    let diff = parsed_fixture("modified.diff", FileStatus::Modified);
    let error = validate_selection(
        &diff,
        DiffCursor {
            row: 5,
            side: DiffSide::Right,
        },
        DiffCursor {
            row: 7,
            side: DiffSide::Right,
        },
    )
    .unwrap_err();
    assert_eq!(error, SelectionError::MissingSidePosition);
}

#[test]
fn rejects_range_missing_left_position() {
    let diff = parsed_fixture("modified.diff", FileStatus::Modified);
    let error = validate_selection(
        &diff,
        DiffCursor {
            row: 5,
            side: DiffSide::Left,
        },
        DiffCursor {
            row: 8,
            side: DiffSide::Left,
        },
    )
    .unwrap_err();
    assert_eq!(error, SelectionError::MissingSidePosition);
}

fn row_with_kind(diff: &ParsedFileDiff, kind: DiffRowKind) -> usize {
    diff.rows.iter().position(|row| row.kind == kind).unwrap()
}

fn parsed_fixture(name: &str, status: FileStatus) -> ParsedFileDiff {
    let path = match status {
        FileStatus::Added => "src/new.rs",
        FileStatus::Deleted => "src/old.rs",
        _ => "src/lib.rs",
    };
    let file = ChangedFile {
        path: RepoPath(path.into()),
        previous_path: None,
        status,
        additions: 0,
        deletions: 0,
        patch: PatchAvailability::Available(
            std::fs::read_to_string(format!("tests/fixtures/patches/{name}")).unwrap(),
        ),
        base_blob: None,
        head_blob: None,
        remotely_reviewed: None,
    };
    parse_file_patch(&file, &CommitOid("head".into())).unwrap()
}

#[test]
fn selection_edges_on_hunk_headers_are_trimmed_inward() {
    let diff = parsed_fixture("multiple-hunks.diff", FileStatus::Modified);
    // Start on code of the first hunk, end ON the second hunk's @@ header
    // row: the header edge is trimmed inward instead of rejecting.
    let last_code_of_first = diff.hunks[0].row_range.end - 1;
    let second_header = diff.hunks[1].row_range.start - 1;
    let selection = validate_selection(
        &diff,
        DiffCursor {
            row: last_code_of_first,
            side: DiffSide::Right,
        },
        DiffCursor {
            row: second_header,
            side: DiffSide::Right,
        },
    )
    .expect("edge on a hunk header must be trimmed, not rejected");
    assert!(selection.end.line >= selection.start.line);
}
