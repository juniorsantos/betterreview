use betterreview::tui::theme;

#[test]
fn the_canvas_paints_our_background_by_default() {
    theme::set_transparent(false);

    assert_eq!(
        theme::canvas().bg,
        Some(theme::BG),
        "the default has to keep looking the way it always did"
    );
}

#[test]
fn a_transparent_canvas_leaves_the_terminal_background_alone() {
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
