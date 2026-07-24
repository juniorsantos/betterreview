use ratatui::{Frame, layout::Rect, text::Line};

use crate::{
    app::AppState,
    tui::widgets::dialog::{Dialog, render_dialog},
};

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    if !state.editor_open {
        return;
    }
    let Some(editor) = &state.session.editor else {
        return;
    };
    let (title, hints) = if editor.stale {
        (
            " Draft antigo (head mudou) ",
            "c novo comentário · Esc fechar",
        )
    } else if state.editing_draft.is_some() {
        (
            " Editando draft ",
            "Enter salvar · Alt+Enter nova linha · Esc fechar",
        )
    } else if state.replying_thread.is_some() {
        (
            " Respondendo ",
            "Enter enviar · Alt+Enter nova linha · Esc fechar",
        )
    } else {
        (
            " Comentário ",
            "Enter salvar · Alt+Enter nova linha · Esc fechar",
        )
    };
    let body: Vec<Line> = editor
        .lines
        .iter()
        .map(|line| Line::raw(line.clone()))
        .collect();
    let inner = render_dialog(
        frame,
        area,
        Dialog {
            title,
            body,
            hints,
            width: 76,
            height: 14,
        },
    );
    if !editor.stale {
        // Show the real terminal cursor at the typing position so the user
        // always knows where input lands. Clamped to the body region
        // `render_dialog` reserved above the blank/hints rows.
        let max_col = inner.width.saturating_sub(1);
        let max_row = inner.height.saturating_sub(1);
        let col = u16::try_from(editor.grapheme_col)
            .unwrap_or(u16::MAX)
            .min(max_col);
        let row = u16::try_from(editor.cursor_row)
            .unwrap_or(u16::MAX)
            .min(max_row);
        frame.set_cursor_position((inner.x + col, inner.y + row));
    }
}
