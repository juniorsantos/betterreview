use ratatui::{Frame, layout::Rect, text::Line};

use crate::{
    app::AppState,
    tui::widgets::dialog::{Dialog, Sizing, render_dialog},
};

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    if !state.editor_open {
        return;
    }
    let Some(editor) = &state.session.editor else {
        return;
    };
    let (title, hints) = if editor.stale {
        (" Stale draft (head changed) ", "c new comment · Esc close")
    } else if state.editing_draft.is_some() {
        (
            " Editing draft ",
            "Enter save · Alt+Enter new line · Esc close",
        )
    } else if state.replying_thread.is_some() {
        (" Replying ", "Enter send · Alt+Enter new line · Esc close")
    } else {
        (" Comment ", "Enter save · Alt+Enter new line · Esc close")
    };
    let body: Vec<Line> = editor
        .lines
        .iter()
        .map(|line| Line::raw(line.clone()))
        .collect();
    let zones = render_dialog(
        frame,
        area,
        Dialog {
            title: Line::raw(title),
            body,
            hints,
            sizing: Sizing::Fixed {
                width: 76,
                height: 14,
            },
            zones: Vec::new(),
        },
    );
    let inner = zones[0];
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
        // +1: render_dialog prefixes every body line with one breathing
        // space, so column 0 of the text sits one cell right of `inner.x`.
        frame.set_cursor_position((inner.x + 1 + col, inner.y + row));
    }
}
