use ratatui::{Frame, layout::Rect, text::Line};

use crate::{
    app::AppState,
    tui::widgets::dialog::{Dialog, menu_line, render_dialog},
};

const OPTIONS: [&str; 3] = ["Manter sessão", "Descartar editor", "Cancelar"];

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let body: Vec<Line> = OPTIONS
        .iter()
        .enumerate()
        .map(|(index, option)| menu_line(option, index == state.quit_selected))
        .collect();
    render_dialog(
        frame,
        area,
        Dialog {
            title: " Sair da revisão ",
            body,
            hints: "j/k mover · Enter confirmar · Esc cancelar",
            width: 52,
            height: 7,
        },
    );
}
