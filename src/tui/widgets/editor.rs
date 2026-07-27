use ratatui::{Frame, layout::Rect, style::Style, text::Line, widgets::Paragraph};

use crate::{
    app::AppState,
    tui::{
        theme,
        widgets::dialog::{
            ActionButton, Dialog, Sizing, button_line, center_lines, clamped_width, render_dialog,
        },
    },
};

const DIALOG_WIDTH: u16 = 60;
const DIALOG_HEIGHT: u16 = 12;

pub(in crate::tui) fn text_width(terminal_width: u16) -> usize {
    usize::from(
        clamped_width(DIALOG_WIDTH, terminal_width)
            .saturating_sub(3)
            .max(1),
    )
}

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    if !state.editor_open {
        return;
    }
    let Some(editor) = &state.session.editor else {
        return;
    };
    let (title, labels): (&str, &[&str]) = if editor.stale {
        (
            " Stale draft (head changed) ",
            &["c new comment", "⎋ close"],
        )
    } else if state.editing_draft.is_some() {
        (" Editing draft ", &["↵ save", "⌥↵ new line", "⎋ close"])
    } else if state.replying_thread.is_some() {
        (" Replying ", &["↵ send", "⌥↵ new line", "⎋ close"])
    } else {
        (" Comment ", &["↵ save", "⌥↵ new line", "⎋ close"])
    };
    let body: Vec<Line> = editor
        .lines
        .iter()
        .map(|line| Line::raw(line.clone()))
        .collect();
    let buttons: Vec<ActionButton<'_>> = labels
        .iter()
        .map(|label| ActionButton {
            label,
            selected: false,
            enabled: true,
        })
        .collect();
    let available_width = text_width(area.width);
    let action_lines = center_lines(vec![button_line(&buttons, 1)], available_width);
    let zones = render_dialog(
        frame,
        area,
        Dialog {
            title: Line::raw(title),
            body,
            hints: "",
            sizing: Sizing::Fixed {
                width: DIALOG_WIDTH,
                height: DIALOG_HEIGHT,
            },
            zones: Vec::new(),
        },
    );
    let inner = zones[0];
    let action_height = u16::try_from(action_lines.len())
        .unwrap_or(u16::MAX)
        .min(inner.height);
    let action_area = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(action_height),
        width: inner.width,
        height: action_height,
    };
    frame.render_widget(
        Paragraph::new(action_lines).style(Style::default().bg(theme::BG)),
        action_area,
    );
    if !editor.stale {
        // Show the real terminal cursor at the typing position so the user
        // always knows where input lands. Clamped to the body region
        // `render_dialog` reserved above the blank/hints rows.
        let max_col = inner.width.saturating_sub(1);
        let max_row = inner.height.saturating_sub(action_height).saturating_sub(1);
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
