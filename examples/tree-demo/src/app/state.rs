#[derive(Debug, Default)]
pub struct State {
    pub cursor: usize,
    pub selected: Option<usize>,
}

impl State {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn move_cursor(&mut self, delta: isize) {
        self.cursor = self.cursor.saturating_add_signed(delta);
    }
}
