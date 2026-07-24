pub fn start(active: usize, total: usize, visible: usize) -> usize {
    if visible == 0 || total <= visible {
        return 0;
    }
    let centered = active.saturating_sub(visible / 2);
    centered.min(total - visible)
}
