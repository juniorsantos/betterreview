pub(super) fn start(selected: usize, total: usize, visible: usize) -> usize {
    if visible == 0 || total <= visible {
        return 0;
    }
    selected
        .saturating_sub(visible / 2)
        .min(total.saturating_sub(visible))
}

pub fn wrapped_height(width: usize, panel_width: usize) -> usize {
    if panel_width == 0 {
        return 1;
    }
    width.div_ceil(panel_width).max(1)
}

/// Where to start scrolling when rows occupy more than one terminal line
/// each: `heights` is the visual height of every row, and the result is an
/// offset in *visual* lines, which is what `Paragraph::scroll` expects.
pub fn start_wrapped(selected: usize, heights: &[usize], visible: usize) -> usize {
    let total: usize = heights.iter().sum();
    if visible == 0 || total <= visible {
        return 0;
    }
    let before: usize = heights.iter().take(selected).sum();
    before
        .saturating_sub(visible / 2)
        .min(total.saturating_sub(visible))
}
