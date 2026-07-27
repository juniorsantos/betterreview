use ratatui::{Frame, layout::Rect, text::Line};

use crate::{
    app::AppState,
    tui::{
        theme,
        widgets::dialog::{Dialog, Sizing, menu_line, render_dialog},
    },
};

const OPTIONS: [&str; 2] = ["Delete", "Cancel"];

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let body: Vec<Line> = OPTIONS
        .iter()
        .enumerate()
        .map(|(index, option)| {
            let color = if index == 0 {
                theme::DANGER
            } else {
                theme::FILLER
            };
            menu_line(option, index == state.delete_selected, color)
        })
        .collect();
    render_dialog(
        frame,
        area,
        Dialog {
            title: Line::raw(" Delete comment "),
            body,
            hints: "j/k move · Enter confirm · Esc cancel",
            sizing: Sizing::Content { max_width: 52 },
            zones: Vec::new(),
        },
    );
}
