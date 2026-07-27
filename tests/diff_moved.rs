use betterreview::{
    diff::{moved_rows, parse_file_patch},
    domain::{ChangedFile, CommitOid, FileStatus, PatchAvailability, RepoPath},
};

fn parsed(patch: &str) -> betterreview::diff::ParsedFileDiff {
    let file = ChangedFile {
        path: RepoPath("src/app.rs".into()),
        previous_path: None,
        status: FileStatus::Modified,
        additions: 0,
        deletions: 0,
        patch: PatchAvailability::Available(patch.to_owned()),
        base_blob: None,
        head_blob: Some("head".into()),
        remotely_reviewed: Some(false),
    };
    parse_file_patch(&file, &CommitOid("head".into())).unwrap()
}

#[test]
fn a_block_that_only_changed_place_is_marked_on_both_sides() {
    let diff = parsed(concat!(
        "@@ -1,6 +1,6 @@\n",
        "-fn validate(input: &str) -> bool {\n",
        "-    !input.trim().is_empty()\n",
        "-}\n",
        " fn main() {\n",
        "     println!(\"start\");\n",
        " }\n",
        "+fn validate(input: &str) -> bool {\n",
        "+    !input.trim().is_empty()\n",
        "+}\n",
    ));

    let moved = moved_rows(&diff);

    assert_eq!(
        moved.len(),
        6,
        "both the removal and the addition are the same code: {moved:?}"
    );
}

#[test]
fn a_real_edit_is_not_a_move() {
    let diff = parsed(concat!(
        "@@ -1,3 +1,3 @@\n",
        " fn main() {\n",
        "-    println!(\"start of the program here\");\n",
        "+    println!(\"the program starts here now\");\n",
        " }\n",
    ));

    assert!(
        moved_rows(&diff).is_empty(),
        "a rewritten line is not a moved one"
    );
}

#[test]
fn a_lone_brace_appearing_on_both_sides_is_not_a_move() {
    let diff = parsed(concat!(
        "@@ -1,3 +1,3 @@\n",
        " fn a() {\n",
        "-}\n",
        " fn b() {\n",
        "+}\n",
    ));

    assert!(
        moved_rows(&diff).is_empty(),
        "punctuation matches everywhere; it must not paint the file"
    );
}
