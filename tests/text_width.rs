use betterreview::tui::{
    abbreviate_path, display_width, expand_tabs, panel_title, truncate_to_width,
};

#[test]
fn width_counts_terminal_cells_not_characters() {
    assert_eq!(display_width("abc"), 3);
    assert_eq!(
        display_width("テスト"),
        6,
        "each CJK ideograph takes two cells"
    );
    assert_eq!(
        display_width("café"),
        4,
        "an accented latin letter takes one"
    );
    assert_eq!(display_width("a→b"), 3);
}

#[test]
fn truncation_never_splits_a_wide_character() {
    assert_eq!(truncate_to_width("テスト", 6), "テスト");
    assert_eq!(
        display_width(&truncate_to_width("テスト", 5)),
        4,
        "a two-cell character cannot occupy the single remaining cell"
    );
    assert_eq!(truncate_to_width("abcdef", 3), "abc");
    assert_eq!(truncate_to_width("abc", 10), "abc");
}

#[test]
fn abbreviating_a_path_keeps_the_file_name() {
    assert_eq!(
        abbreviate_path("aadfadf/bsdff/casdfdsf/config.rs", 22),
        "a/b/casdfdsf/config.rs"
    );
    assert_eq!(
        abbreviate_path("src/app/reducer.rs", 30),
        "src/app/reducer.rs",
        "a path that fits is untouched"
    );
}

#[test]
fn abbreviation_falls_back_to_shortening_the_name_itself() {
    let result = abbreviate_path("src/very_long_file_name_here.rs", 12);

    assert!(display_width(&result) <= 12, "got {result:?}");
    assert!(
        result.ends_with(".rs"),
        "the extension identifies the file and must survive: {result:?}"
    );
}

#[test]
fn abbreviation_measures_cells_so_cjk_paths_do_not_overflow() {
    let result = abbreviate_path("ソース/アプリ/テスト.rs", 16);

    assert!(
        display_width(&result) <= 16,
        "{result:?} takes {} cells",
        display_width(&result)
    );
}

#[test]
fn wrapped_height_counts_the_rows_a_line_needs() {
    use betterreview::tui::wrapped_height;

    assert_eq!(wrapped_height(0, 80), 1, "an empty line still takes a row");
    assert_eq!(wrapped_height(80, 80), 1);
    assert_eq!(wrapped_height(81, 80), 2);
    assert_eq!(wrapped_height(240, 80), 3);
    assert_eq!(wrapped_height(10, 0), 1, "a zero-width panel cannot divide");
}

#[test]
fn scrolling_counts_visual_lines_not_rows() {
    use betterreview::tui::start_wrapped;

    // Ten rows, each taking three terminal lines: 30 visual lines total.
    let heights = vec![3usize; 10];

    assert_eq!(start_wrapped(0, &heights, 30), 0, "everything fits");
    assert_eq!(
        start_wrapped(9, &heights, 9),
        21,
        "the last row starts at visual line 27; centring it clamps to 30-9"
    );
    assert_eq!(
        start_wrapped(5, &heights, 9),
        11,
        "row 5 begins at visual line 15, centred in 9 lines"
    );
}

#[test]
fn a_row_taller_than_the_panel_still_scrolls_to_it() {
    use betterreview::tui::start_wrapped;

    let heights = vec![1, 40, 1];

    assert_eq!(
        start_wrapped(1, &heights, 10),
        0,
        "the tall row starts at 1"
    );
    assert_eq!(
        start_wrapped(2, &heights, 10),
        32,
        "past the tall row, clamped to the end"
    );
}

#[test]
fn a_tab_advances_to_the_next_stop_not_a_fixed_number_of_spaces() {
    assert_eq!(expand_tabs("\tif", 4, 0), "    if");
    assert_eq!(
        expand_tabs("ab\tc", 4, 0),
        "ab  c",
        "a tab two columns in advances two, landing on the stop"
    );
    assert_eq!(
        expand_tabs("abcd\te", 4, 0),
        "abcd    e",
        "a tab exactly on a stop advances a full width"
    );
    assert_eq!(
        expand_tabs("\tx", 4, 2),
        "  x",
        "the starting column shifts where the stop falls"
    );
    assert_eq!(expand_tabs("no tabs", 4, 0), "no tabs");
}

#[test]
fn a_panel_title_drops_its_count_before_it_drops_its_name() {
    assert_eq!(panel_title("Files", Some("4/12"), 20), "─ Files (4/12) ");
    assert_eq!(
        panel_title("Files", Some("4/12"), 10),
        "─ Files ",
        "the count is what yields when the border is narrow"
    );
    assert_eq!(panel_title("Files", None, 20), "─ Files ");
    assert_eq!(
        panel_title("Files", Some("4/12"), 3),
        "─ Files ",
        "the name survives even when nothing fits: a nameless panel is useless"
    );
}

#[test]
fn a_panel_title_stands_off_the_corner() {
    assert_eq!(panel_title("Files", Some("4/12"), 30), "─ Files (4/12) ");
    assert_eq!(
        panel_title("Files", None, 30),
        "─ Files ",
        "the title never sits flush against the corner"
    );
}
