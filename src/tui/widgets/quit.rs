use ratatui::{Frame, layout::Rect, text::Line};

use crate::{
    app::AppState,
    tui::widgets::dialog::{
        ActionButton, Dialog, Sizing, button_line, center_lines, clamped_width, render_dialog,
    },
};

const DIALOG_MAX_WIDTH: u16 = 90;

const OPTIONS: [&str; 3] = [
    "Quit keeping the draft",
    "Quit discarding the draft",
    "Cancel",
];

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let available_width = usize::from(
        clamped_width(DIALOG_MAX_WIDTH, area.width)
            .saturating_sub(3)
            .max(1),
    );
    let buttons: Vec<ActionButton<'_>> = OPTIONS
        .iter()
        .enumerate()
        .map(|(index, label)| ActionButton {
            label,
            selected: index == state.quit_selected,
            enabled: true,
        })
        .collect();
    let actions = center_lines(vec![button_line(&buttons, 1)], available_width);
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
