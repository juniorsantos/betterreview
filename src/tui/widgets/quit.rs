use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Clear, Paragraph},
};

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect) {
    let width = area.width.saturating_sub(4).min(52);
    let height = 7.min(area.height.saturating_sub(2));
    let popup = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    };
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new("Keep session\nDiscard editor\nCancel").block(
            Block::default()
                .title(" Quit review? ")
                .borders(Borders::ALL),
        ),
        popup,
    );
}
