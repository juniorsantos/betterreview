use ratatui::{Frame, layout::Rect, text::Line};

use crate::{
    app::AppState,
    tui::widgets::dialog::{Dialog, render_dialog},
};

const TEXT: &str = "Navigation          Files                Review\n\
                    j/k       move      e    expand panel   v      selection\n\
                    Tab/h/l   focus     z    fold folder    c      comment\n\
                    ]f / [f   file      m    reviewed       s      suggestion\n\
                    ]u / [u   unreviewed                    t      threads\n\
                    ]h / [h   hunk\n\
                    ]c / [c   comment\n\
                    /         search\n\
                    \n\
                    Comments: e edit  x delete  r reply  T hide/show\n\
                    Editor: Enter save   Alt+Enter newline   Esc close\n\
                    R submit review      r refresh           q quit";

pub(in crate::tui) fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    if !state.help_visible {
        return;
    }
    let body: Vec<Line> = TEXT.lines().map(Line::raw).collect();
    render_dialog(
        frame,
        area,
        Dialog {
            title: " Ajuda ",
            body,
            hints: "Esc fechar",
            width: 66,
            height: 16,
        },
    );
}
