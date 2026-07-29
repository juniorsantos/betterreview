use std::num::NonZeroU16;

use ratatui::{
    Frame,
    buffer::{Buffer, CellDiffOption},
    layout::Rect,
    widgets::Widget,
};
use unicode_width::UnicodeWidthStr;
use url::Url;

use crate::{app::AppState, providers::ReviewLinks};

use super::{text::display_width, widgets::header};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Target {
    pub area: Rect,
    pub url: String,
}

pub(super) fn apply(frame: &mut Frame, area: Rect, target: &str) {
    if area.width == 0 || area.height == 0 || !safe_target(target) {
        return;
    }
    frame.render_widget(Hyperlink { target }, area);
}

pub(super) fn header_targets(state: &AppState, area: Rect) -> Vec<Target> {
    let Some(links) = ReviewLinks::new(&state.provider.key, &state.provider.web_url) else {
        return Vec::new();
    };
    let review_label = format!("#{}", state.provider.key.number);
    let review_x = area.x.saturating_add(
        u16::try_from(
            display_width(header::NAME_CHIP)
                + display_width(&format!(" {} ", state.provider.key.repository)),
        )
        .unwrap_or(u16::MAX),
    );
    let mut targets = Vec::new();
    if let Some(area) = target_area(area, review_x, &review_label) {
        targets.push(Target {
            area,
            url: links.review_url().to_owned(),
        });
    }

    let head = state.provider.head.0.chars().take(7).collect::<String>();
    let head_x = review_x.saturating_add(
        u16::try_from(display_width(&review_label) + display_width(" · ")).unwrap_or(u16::MAX),
    );
    if let Some(area) = target_area(area, head_x, &head) {
        targets.push(Target {
            area,
            url: links.commit_url(&state.provider.head),
        });
    }
    targets
}

fn target_area(row: Rect, x: u16, label: &str) -> Option<Rect> {
    let right = row.right();
    if x >= right {
        return None;
    }
    let width = u16::try_from(display_width(label))
        .unwrap_or(u16::MAX)
        .min(right - x);
    (width > 0).then(|| Rect::new(x, row.y, width, 1))
}

struct Hyperlink<'a> {
    target: &'a str,
}

impl Widget for Hyperlink<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let right = area.right().min(buffer.area.right());
        if area.x >= right || area.y >= buffer.area.bottom() {
            return;
        }

        let mut x = area.x;
        let mut last = area.x;
        while x < right {
            last = x;
            let width = UnicodeWidthStr::width(buffer[(x, area.y)].symbol()).max(1);
            x = x.saturating_add(u16::try_from(width).unwrap_or(u16::MAX));
        }

        if area.x == last {
            wrap_cell(
                &mut buffer[(area.x, area.y)],
                &format!("\x1b]8;;{}\x1b\\", self.target),
                "\x1b]8;;\x1b\\",
            );
            return;
        }

        wrap_cell(
            &mut buffer[(area.x, area.y)],
            &format!("\x1b]8;;{}\x1b\\", self.target),
            "",
        );
        wrap_cell(&mut buffer[(last, area.y)], "", "\x1b]8;;\x1b\\");
    }
}

fn wrap_cell(cell: &mut ratatui::buffer::Cell, prefix: &str, suffix: &str) {
    let symbol = cell.symbol().to_owned();
    let width = UnicodeWidthStr::width(symbol.as_str()).max(1);
    let width = u16::try_from(width)
        .ok()
        .and_then(NonZeroU16::new)
        .expect("a terminal cell has a non-zero width");
    cell.set_symbol(&format!("{prefix}{symbol}{suffix}"))
        .set_diff_option(CellDiffOption::ForcedWidth(width));
}

fn safe_target(target: &str) -> bool {
    Url::parse(target)
        .ok()
        .is_some_and(|url| matches!(url.scheme(), "http" | "https") && url.host().is_some())
}
