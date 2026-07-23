pub(super) fn start(selected: usize, total: usize, visible: usize) -> usize {
    if visible == 0 || total <= visible {
        return 0;
    }
    selected
        .saturating_sub(visible / 2)
        .min(total.saturating_sub(visible))
}
