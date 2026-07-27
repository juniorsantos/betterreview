use ratatui::{Frame, layout::Rect, text::Line};

use crate::{
    app::AppState,
    tui::widgets::dialog::{ActionButton, Dialog, Sizing, button_line, render_dialog},
};

const DIALOG_MAX_WIDTH: u16 = 90;

const OPTIONS: [&str; 3] = [
    "Quit keeping the draft",
    "Quit discarding the draft",
    "Cancel",
];

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let buttons: Vec<ActionButton<'_>> = OPTIONS
        .iter()
        .enumerate()
        .map(|(index, label)| ActionButton {
            label,
            selected: index == state.quit_selected,
            enabled: true,
        })
        .collect();
    let actions = vec![Line::raw(""), button_line(&buttons, 1)];
    render_dialog(
        frame,
        area,
        Dialog {
            title: Line::raw(" Quit review "),
            body: actions,
            hints: "⇥ move · ↵ confirm · ⎋ cancel",
            sizing: Sizing::Content {
                max_width: DIALOG_MAX_WIDTH,
            },
            zones: Vec::new(),
        },
    );
}
