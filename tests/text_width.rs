use betterreview::tui::{abbreviate_path, display_width, truncate_to_width};

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
