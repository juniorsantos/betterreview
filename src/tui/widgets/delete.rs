use ratatui::{Frame, layout::Rect, text::Line};

use crate::{
    app::AppState,
    tui::{
        text::display_width,
        widgets::dialog::{ActionButton, Dialog, Sizing, button_line, center_lines, render_dialog},
    },
};

const OPTIONS: [&str; 2] = ["Delete", "Cancel"];
const HINTS: &str = "⇥ move · ↵ confirm · ⎋ cancel";

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
    let mut body = vec![Line::raw("")];
    body.extend(center_lines(
        vec![button_line(&buttons, 1)],
        display_width(HINTS),
    ));
    render_dialog(
        frame,
        area,
        Dialog {
            title: Line::raw(" Delete comment "),
            body,
            hints: HINTS,
            sizing: Sizing::Content { max_width: 52 },
            zones: Vec::new(),
        },
    );
}
