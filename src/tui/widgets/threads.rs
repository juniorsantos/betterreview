use ratatui::{
    Frame,
    layout::Rect,
    text::Line,
    widgets::{Block, Borders, Clear, List, ListItem},
};

use crate::app::AppState;

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    if !state.thread_panel_open {
        return;
    }
    let width = area.width.saturating_sub(4).min(60);
    let popup = Rect {
        x: area.right().saturating_sub(width + 1),
        y: area.y + 1,
        width,
        height: area.height.saturating_sub(2),
    };
    let items = state
        .provider
        .threads
        .iter()
        .flat_map(|thread| {
            let status = if thread.resolved {
                "resolvido"
            } else {
                "aberto"
            };
            let mut lines = vec![ListItem::new(Line::raw(format!(
                "{} [{}]",
                thread.path.0, status
            )))];
            lines.extend(thread.comments.iter().map(|comment| {
                ListItem::new(Line::raw(format!("{}: {}", comment.author, comment.body)))
            }));
            lines
        })
        .collect::<Vec<_>>();
    frame.render_widget(Clear, popup);
    frame.render_widget(
        List::new(items).block(Block::default().title(" Threads ").borders(Borders::ALL)),
        popup,
    );
}
