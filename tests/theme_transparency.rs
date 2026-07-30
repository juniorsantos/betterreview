use betterreview::tui::theme;

#[test]
fn canvas_background_follows_the_transparency_setting() {
    theme::set_transparent(false);

    assert_eq!(
        theme::canvas().bg,
        Some(theme::BG),
        "the default has to keep looking the way it always did"
    );
    theme::set_transparent(true);

    let canvas = theme::canvas();

    assert_eq!(
        canvas.bg, None,
        "a transparent terminal must keep its transparency"
    );
    assert_eq!(
        canvas.fg,
        Some(theme::FG),
        "only the canvas fill goes; the palette stays"
    );

    theme::set_transparent(false);
}
