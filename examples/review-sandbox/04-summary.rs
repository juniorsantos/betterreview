pub struct ReviewSummary {
    pub files: usize,
    pub comments: usize,
}

impl ReviewSummary {
    pub fn is_complete(&self) -> bool {
        self.files > 0 && self.comments > 0
    }
}
