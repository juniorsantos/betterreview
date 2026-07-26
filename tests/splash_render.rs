use betterreview::tui::splash;
use ratatui::{Terminal, backend::TestBackend};

#[test]
fn splash_shows_name_version_and_loading() {
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal.draw(splash).unwrap();
    let buffer = terminal.backend().buffer();
    let screen: String = (0..24)
        .map(|y| {
            (0..80)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(screen.contains("betterreview"));
    assert!(screen.contains(env!("CARGO_PKG_VERSION")));
    assert!(screen.contains("loading"));
}
