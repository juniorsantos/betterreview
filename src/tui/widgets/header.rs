//! Shared borderless chip header (transversal rule 3): exactly two
//! reverse-video chips — the app name on the left and the crate version on
//! the right — bracketing a plain MUTED middle string. Used by the picker
//! and review screens.

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::tui::{text::display_width, theme};

/// Builds the header line: ` betterreview ` chip (ACCENT reversed + bold),
/// `middle` in MUTED, and ` vX.Y.Z ` chip (MUTED reversed) pushed to the
/// right edge of `width` columns.
pub(in crate::tui) fn chip_line(middle: &str, width: u16) -> Line<'static> {
    let name_chip = " betterreview ";
    let version_chip = format!(" v{} ", env!("CARGO_PKG_VERSION"));

    let name_span = Span::styled(
        name_chip,
        Style::default()
            .fg(theme::ACCENT)
            .add_modifier(Modifier::REVERSED | Modifier::BOLD),
    );
    let middle_span = Span::styled(middle.to_owned(), Style::default().fg(theme::MUTED));
    let version_span = Span::styled(
        version_chip.clone(),
        Style::default()
            .fg(theme::MUTED)
            .add_modifier(Modifier::REVERSED),
    );

    let used = display_width(name_chip) + display_width(middle) + display_width(&version_chip);
    let pad = (width as usize).saturating_sub(used);

    Line::from(vec![
        name_span,
        middle_span,
        Span::raw(" ".repeat(pad)),
        version_span,
    ])
}
