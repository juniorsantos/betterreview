use betterreview::tui::theme;
use ratatui::style::Color;

fn channel(value: u8) -> f64 {
    let value = f64::from(value) / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn luminance(color: Color) -> f64 {
    let Color::Rgb(red, green, blue) = color else {
        panic!("the palette is defined in rgb");
    };
    0.2126 * channel(red) + 0.7152 * channel(green) + 0.0722 * channel(blue)
}

fn contrast(color: Color) -> f64 {
    let (a, b) = (luminance(color), luminance(theme::BG));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

#[test]
fn decoration_recedes_and_content_carries_the_contrast() {
    let code = contrast(theme::FG);
    let numbers = contrast(theme::MUTED);
    let frames = contrast(theme::BORDER);
    let filler = contrast(theme::FILLER);

    assert!(
        code > numbers && numbers > frames && frames > filler,
        "the palette must be tiered by role — code {code:.1}, numbers {numbers:.1}, \
         frames {frames:.1}, filler {filler:.1}"
    );
}

#[test]
fn frames_stay_below_reading_strength() {
    let frames = contrast(theme::BORDER);
    assert!(
        (1.5..=3.0).contains(&frames),
        "a panel frame is decoration: perceivable when looked for, invisible otherwise, got {frames:.1}:1"
    );
}

#[test]
fn the_alignment_filler_is_the_quietest_token() {
    let filler = contrast(theme::FILLER);
    assert!(
        filler <= 1.6,
        "the filler covers whole regions, so it must read as texture and never as content, got {filler:.1}:1"
    );
}

#[test]
fn line_numbers_stay_readable_while_subordinate() {
    let numbers = contrast(theme::MUTED);
    assert!(
        numbers >= 4.5,
        "secondary text still has to be readable, got {numbers:.1}:1"
    );
    assert!(
        numbers < contrast(theme::FG) / 2.0,
        "and it must be clearly subordinate to the code"
    );
}
