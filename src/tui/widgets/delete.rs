use ratatui::{Frame, layout::Rect, text::Line};

use crate::{
    app::AppState,
    tui::widgets::dialog::{Dialog, menu_line, render_dialog},
};

const OPTIONS: [&str; 2] = ["Excluir", "Cancelar"];

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
            title: " Excluir comentário ",
            body,
            hints: "j/k mover · Enter confirmar · Esc cancelar",
            width: 52,
            height: 6,
        },
    );
}
