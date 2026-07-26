use betterreview::diff::{DeltaError, sanitize_ansi};

#[test]
fn strips_clipboard_cursor_and_erase_sequences_but_keeps_sgr() {
    let sanitized =
        sanitize_ansi(b"safe\x1b]52;c;ZXhmaWx0cmF0ZQ==\x07\x1b[2J\x1b[31mred\x1b[0m").unwrap();
    assert!(!sanitized.windows(3).any(|bytes| bytes == b"]52"));
    assert!(!sanitized.windows(3).any(|bytes| bytes == b"[2J"));
    assert!(sanitized.windows(4).any(|bytes| bytes == b"[31m"));
    let text = String::from_utf8_lossy(&sanitized);
    assert!(text.contains("safe"));
    assert!(text.contains("red"));
}

#[test]
fn strips_hyperlinks_and_device_control_strings() {
    let input = b"a\x1b]8;;https://example.test\x1b\\link\x1b]8;;\x1b\\b\x1bPsecret\x1b\\c\x1b_hidden\x1b\\d\x1b^private\x1b\\e";
    let sanitized = sanitize_ansi(input).unwrap();
    assert_eq!(sanitized, b"alinkbcde");
}

#[test]
fn strips_utf8_encoded_c1_controls() {
    let sanitized = sanitize_ansi("safe\u{009b}31mtext".as_bytes()).unwrap();
    assert_eq!(sanitized, b"safe31mtext");
}

#[test]
fn rejects_invalid_utf8() {
    assert!(matches!(
        sanitize_ansi(b"valid\xffinvalid"),
        Err(DeltaError::InvalidUtf8)
    ));
}

#[test]
fn a_bidi_override_is_escaped_instead_of_taking_effect() {
    let payload = "if access_level != \u{202e}\u{2066}// \u{2069}\u{2066}root\u{2069}";

    let cleaned = String::from_utf8(sanitize_ansi(payload.as_bytes()).unwrap()).unwrap();

    assert!(
        !cleaned.contains('\u{202e}'),
        "a right-to-left override must never reach the terminal: {cleaned:?}"
    );
    assert!(
        !cleaned.contains('\u{2066}') && !cleaned.contains('\u{2069}'),
        "nor may the isolates that complete the trojan-source attack: {cleaned:?}"
    );
    assert!(
        cleaned.contains("<U+202E>"),
        "and the reviewer has to see that something was there: {cleaned:?}"
    );
}

#[test]
fn zero_width_characters_are_made_visible() {
    let payload = "let admin\u{200b} = false;";

    let cleaned = String::from_utf8(sanitize_ansi(payload.as_bytes()).unwrap()).unwrap();

    assert_eq!(cleaned, "let admin<U+200B> = false;");
}

#[test]
fn ordinary_text_is_untouched_by_the_confusable_pass() {
    let payload = "let café = \"日本語\"; // ok";

    let cleaned = String::from_utf8(sanitize_ansi(payload.as_bytes()).unwrap()).unwrap();

    assert_eq!(cleaned, payload, "accents and CJK are not confusables");
}
