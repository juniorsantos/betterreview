use ratatui::{Frame, layout::Rect, text::Line};

use crate::{
    app::AppState,
    tui::widgets::dialog::{Dialog, menu_line, render_dialog},
};

const OPTIONS: [&str; 2] = ["Delete", "Cancel"];

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let body: Vec<Line> = OPTIONS
        .iter()
        .enumerate()
        .map(|(index, option)| menu_line(option, index == state.delete_selected))
        .collect();
    render_dialog(
        frame,
        area,
        Dialog {
            title: Line::raw(" Delete comment "),
            body,
            hints: "j/k move · Enter confirm · Esc cancel",
            width: 52,
            height: 6,
        },
    );
}
