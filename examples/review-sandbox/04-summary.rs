pub struct ReviewSummary {
    pub files: usize,
    pub comments: usize,
    pub unresolved_threads: usize,
}

impl ReviewSummary {
    pub fn is_complete(&self) -> bool {
        self.files > 0 && self.unresolved_threads == 0
    }

    pub fn progress(&self) -> String {
        format!("{} files · {} comments", self.files, self.comments)
    }
}
