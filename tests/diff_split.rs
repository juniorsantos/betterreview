use betterreview::diff::{DiffRow, DiffRowKind, SplitPair, pair_rows};

fn row(kind: DiffRowKind) -> DiffRow {
    DiffRow {
        raw: String::new(),
        kind,
        old_line: None,
        new_line: None,
        left: None,
        right: None,
    }
}

fn rows(kinds: &[DiffRowKind]) -> Vec<DiffRow> {
    kinds.iter().copied().map(row).collect()
}

fn pair(left: Option<usize>, right: Option<usize>) -> SplitPair {
    SplitPair { left, right }
}

#[test]
fn context_lines_show_on_both_sides() {
    let parsed = rows(&[DiffRowKind::Context, DiffRowKind::Context]);

    assert_eq!(
        pair_rows(&parsed),
        vec![pair(Some(0), Some(0)), pair(Some(1), Some(1))]
    );
}

#[test]
fn a_removed_line_pairs_with_the_added_one_that_replaces_it() {
    let parsed = rows(&[DiffRowKind::Removed, DiffRowKind::Added]);

    assert_eq!(pair_rows(&parsed), vec![pair(Some(0), Some(1))]);
}

#[test]
fn extra_removals_leave_the_right_side_empty() {
    let parsed = rows(&[
        DiffRowKind::Removed,
        DiffRowKind::Removed,
        DiffRowKind::Added,
    ]);

    assert_eq!(
        pair_rows(&parsed),
        vec![pair(Some(0), Some(2)), pair(Some(1), None)]
    );
}

#[test]
fn extra_additions_leave_the_left_side_empty() {
    let parsed = rows(&[DiffRowKind::Removed, DiffRowKind::Added, DiffRowKind::Added]);

    assert_eq!(
        pair_rows(&parsed),
        vec![pair(Some(0), Some(1)), pair(None, Some(2))]
    );
}

#[test]
fn a_pure_addition_block_has_no_left_side() {
    let parsed = rows(&[DiffRowKind::Context, DiffRowKind::Added, DiffRowKind::Added]);

    assert_eq!(
        pair_rows(&parsed),
        vec![
            pair(Some(0), Some(0)),
            pair(None, Some(1)),
            pair(None, Some(2))
        ]
    );
}

#[test]
fn a_pure_deletion_block_has_no_right_side() {
    let parsed = rows(&[DiffRowKind::Removed, DiffRowKind::Context]);

    assert_eq!(
        pair_rows(&parsed),
        vec![pair(Some(0), None), pair(Some(1), Some(1))]
    );
}

#[test]
fn headers_and_metadata_never_become_pairs() {
    let parsed = rows(&[
        DiffRowKind::Header,
        DiffRowKind::HunkHeader,
        DiffRowKind::Context,
        DiffRowKind::Metadata,
    ]);

    assert_eq!(pair_rows(&parsed), vec![pair(Some(2), Some(2))]);
}

#[test]
fn each_block_pairs_independently() {
    let parsed = rows(&[
        DiffRowKind::Removed,
        DiffRowKind::Added,
        DiffRowKind::Context,
        DiffRowKind::Removed,
        DiffRowKind::Added,
    ]);

    assert_eq!(
        pair_rows(&parsed),
        vec![
            pair(Some(0), Some(1)),
            pair(Some(2), Some(2)),
            pair(Some(3), Some(4)),
        ]
    );
}

#[test]
fn additions_before_removals_still_pair_within_the_block() {
    let parsed = rows(&[DiffRowKind::Added, DiffRowKind::Removed]);

    assert_eq!(
        pair_rows(&parsed),
        vec![pair(None, Some(0)), pair(Some(1), None)],
        "git never emits + before - inside a block; keep each on its own row"
    );
}
