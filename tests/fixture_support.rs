mod support;

use betterreview::app::DisplayRow;
use support::{FileSpec, Fixture};

#[test]
fn the_parsed_and_rendered_diffs_agree_because_both_come_from_the_patch() {
    let state = Fixture::new()
        .file(FileSpec::new(
            "src/app.rs",
            "@@ -1,2 +1,3 @@\n context\n-old\n+new\n+extra\n",
        ))
        .build();

    let parsed = state.parsed_diff.as_ref().expect("the patch parsed");
    let rendered = state.rendered_diff.as_ref().expect("and rendered");

    assert_eq!(
        parsed.rows.len(),
        rendered.rows.len(),
        "a fixture whose two halves disagree describes a state the app cannot produce"
    );
    for (index, row) in rendered.rows.iter().enumerate() {
        assert_eq!(row.binding.row_index, index);
        assert_eq!(row.binding.left, parsed.rows[index].left);
        assert_eq!(row.binding.right, parsed.rows[index].right);
    }
}

#[test]
fn hunk_coordinates_come_from_the_real_parser_not_from_hand() {
    let state = Fixture::new()
        .file(FileSpec::new(
            "src/app.rs",
            "@@ -1 +1,3 @@\n context\n+one\n+two\n@@ -5 +7 @@\n context\n",
        ))
        .build();

    let hunks = &state.parsed_diff.as_ref().unwrap().hunks;

    assert_eq!(hunks.len(), 2);
    assert_eq!(
        (hunks[0].old_start, hunks[0].new_start, hunks[0].new_count),
        (1, 1, 3)
    );
    assert_eq!((hunks[1].old_start, hunks[1].new_start), (5, 7));
}

#[test]
fn a_cached_file_lets_the_gaps_expand() {
    let mut state = Fixture::new()
        .file(
            FileSpec::new(
                "src/app.rs",
                "@@ -1 +1 @@\n context\n@@ -5 +5 @@\n context\n",
            )
            .cached_lines(7),
        )
        .build();
    state.expanded_gaps.insert(1);
    betterreview::app::refresh_display_rows(&mut state);

    let context: Vec<u32> = state
        .display_rows
        .iter()
        .filter_map(|row| match row {
            DisplayRow::Context { new_line, .. } => Some(*new_line),
            _ => None,
        })
        .collect();

    assert_eq!(context, vec![2, 3, 4]);
}
