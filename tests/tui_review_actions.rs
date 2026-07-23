use betterreview::tui::EditorState;

#[test]
fn editor_moves_and_deletes_by_unicode_grapheme() {
    let mut editor = EditorState::from_text("a🇧🇷e\u{301}");
    assert_eq!(editor.grapheme_col, 3);

    editor.move_left();
    editor.backspace();

    assert_eq!(editor.body(), "ae\u{301}");
    assert_eq!(editor.grapheme_col, 1);
}

#[test]
fn editor_inserts_multiline_paste_at_the_cursor() {
    let mut editor = EditorState::from_text("before after");
    for _ in 0..5 {
        editor.move_left();
    }

    editor.insert_text("first\nsecond");

    assert_eq!(editor.body(), "before first\nsecondafter");
    assert_eq!(editor.row, 1);
    assert_eq!(editor.grapheme_col, 6);
}

#[test]
fn backspace_at_start_joins_lines_without_splitting_unicode() {
    let mut editor = EditorState::from_text("olá\nmundo");
    editor.row = 1;
    editor.grapheme_col = 0;

    editor.backspace();

    assert_eq!(editor.body(), "olámundo");
    assert_eq!(editor.row, 0);
    assert_eq!(editor.grapheme_col, 3);
}

#[test]
fn stale_read_only_editor_preserves_text() {
    let mut editor = EditorState::from_text("texto antigo");
    editor.read_only = true;

    editor.insert_char('!');
    editor.insert_text("novo");
    editor.backspace();

    assert_eq!(editor.body(), "texto antigo");
}
