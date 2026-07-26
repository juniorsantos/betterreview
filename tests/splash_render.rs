use betterreview::tui::splash;
use ratatui::{Terminal, backend::TestBackend};

fn screen(width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(splash).unwrap();
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn splash_shows_name_version_and_loading() {
    let screen = screen(80, 24);

    assert!(screen.contains("BetterReview"));
    assert!(screen.contains(env!("CARGO_PKG_VERSION")));
    assert!(screen.contains("loading"));
}

#[test]
fn a_roomy_terminal_gets_the_block_banner() {
    let screen = screen(80, 24);

    let banner = include_str!("../assets/banner.txt");
    for (index, row) in banner.lines().enumerate() {
        assert!(
            screen.contains(row.trim_end()),
            "banner row {index} missing from the splash"
        );
    }
}

#[test]
fn the_banner_is_centered_horizontally() {
    let screen = screen(80, 24);
    let row = screen
        .lines()
        .find(|line| line.contains('█'))
        .expect("banner row");

    let left = row.len() - row.trim_start().len();
    let right = row.len() - row.trim_end().len();
    assert!(
        left.abs_diff(right) <= 1,
        "banner off-center: left={left} right={right}"
    );
}

#[test]
fn a_narrow_terminal_falls_back_to_the_chip() {
    let screen = screen(50, 16);

    assert!(
        !screen.contains('█'),
        "50 columns cannot hold a 50-wide banner plus margins"
    );
    assert!(screen.contains("betterreview"));
    assert!(screen.contains(env!("CARGO_PKG_VERSION")));
    assert!(screen.contains("loading"));
}

#[test]
fn a_short_terminal_falls_back_even_when_wide() {
    let screen = screen(120, 18);

    assert!(!screen.contains('█'), "18 rows cannot hold 12 banner rows");
    assert!(screen.contains("betterreview"));
}
