use ratatui::{Frame, layout::Rect, text::Line};

use crate::{
    app::AppState,
    tui::{
        theme,
        widgets::dialog::{Dialog, Sizing, menu_line, render_dialog},
    },
};

const OPTIONS: [&str; 3] = [
    "Quit keeping the draft",
    "Quit discarding the draft",
    "Cancel",
];

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let body: Vec<Line> = OPTIONS
        .iter()
        .enumerate()
        .map(|(index, option)| {
            let color = match index {
                0 => theme::SUCCESS,
                1 => theme::DANGER,
                _ => theme::FILLER,
            };
            menu_line(option, index == state.quit_selected, color)
        })
        .collect();
    render_dialog(
        frame,
        area,
        Dialog {
            title: Line::raw(" Quit review "),
            body,
            hints: "j/k move · Enter confirm · Esc cancel",
            sizing: Sizing::Content { max_width: 52 },
            zones: Vec::new(),
        },
    );
}
