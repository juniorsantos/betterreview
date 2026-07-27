use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
};

use crate::{
    app::AppState,
    tui::{
        theme,
        widgets::dialog::{Dialog, Sizing, clamped_width, menu_button, render_dialog},
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
    let compact = available_width < 61;
    let spacious = available_width >= 69;
    let labels = if compact {
        ["Keep draft", "Discard draft", "Cancel"]
    } else {
        OPTIONS
    };
    let padding = usize::from(spacious) + 1;
    let mut actions = Vec::new();
    for (index, label) in labels.into_iter().enumerate() {
        if index > 0 {
            actions.push(Span::raw(" ".repeat(padding)));
        }
        let color = match index {
            0 => theme::BUTTON_SUCCESS,
            1 => theme::BUTTON_DANGER,
            _ => theme::BUTTON_NEUTRAL,
        };
        actions.push(menu_button(
            label,
            index == state.quit_selected,
            color,
            padding,
        ));
    }
    render_dialog(
        frame,
        area,
        Dialog {
            title: Line::raw(" Quit review "),
            body: vec![Line::from(actions)],
            hints: "j/k move · Enter confirm · Esc cancel",
            sizing: Sizing::Content {
                max_width: DIALOG_MAX_WIDTH,
            },
            zones: Vec::new(),
        },
    );
}
