use ratatui::{Frame, layout::Rect, text::Line};

use crate::{
    app::AppState,
    tui::widgets::dialog::{ActionButton, Dialog, Sizing, outlined_button_rows, render_dialog},
};

const OPTIONS: [&str; 2] = ["Delete", "Cancel"];

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let buttons: Vec<ActionButton<'_>> = OPTIONS
        .iter()
        .enumerate()
        .map(|(index, label)| ActionButton {
            label,
            selected: index == state.delete_selected,
            enabled: true,
        })
        .collect();
    let body = outlined_button_rows(&buttons, 2, 2);
    render_dialog(
        frame,
        area,
        Dialog {
            title: Line::raw(" Delete comment "),
            body,
            hints: "j/k move · ↵ confirm · ⎋ cancel",
            sizing: Sizing::Content { max_width: 52 },
            zones: Vec::new(),
        },
    );
}
