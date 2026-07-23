use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorState {
    pub lines: Vec<String>,
    pub row: usize,
    pub grapheme_col: usize,
    pub read_only: bool,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            lines: vec![String::new()],
            row: 0,
            grapheme_col: 0,
            read_only: false,
        }
    }
}

impl EditorState {
    pub fn from_text(value: &str) -> Self {
        let lines = value.split('\n').map(str::to_owned).collect::<Vec<_>>();
        let row = lines.len().saturating_sub(1);
        let grapheme_col = grapheme_count(&lines[row]);
        Self {
            lines,
            row,
            grapheme_col,
            read_only: false,
        }
    }

    pub fn insert_char(&mut self, value: char) {
        let mut encoded = [0; 4];
        self.insert_text(value.encode_utf8(&mut encoded));
    }

    pub fn insert_text(&mut self, value: &str) {
        if self.read_only || value.is_empty() {
            return;
        }
        self.normalize_cursor();
        let byte = byte_index(&self.lines[self.row], self.grapheme_col);
        let prefix = self.lines[self.row][..byte].to_owned();
        let suffix = self.lines[self.row][byte..].to_owned();
        let inserted = value
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .split('\n')
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if inserted.len() == 1 {
            let before_cursor = format!("{prefix}{}", inserted[0]);
            self.grapheme_col = grapheme_count(&before_cursor);
            self.lines[self.row] = format!("{before_cursor}{suffix}");
            return;
        }
        let first = format!("{prefix}{}", inserted[0]);
        let last_index = inserted.len() - 1;
        let last_cursor = grapheme_count(&inserted[last_index]);
        let last = format!("{}{suffix}", inserted[last_index]);
        self.lines[self.row] = first;
        for (offset, line) in inserted[1..last_index].iter().cloned().enumerate() {
            self.lines.insert(self.row + 1 + offset, line);
        }
        self.lines.insert(self.row + last_index, last);
        self.row += last_index;
        self.grapheme_col = last_cursor;
    }

    pub fn backspace(&mut self) {
        if self.read_only {
            return;
        }
        self.normalize_cursor();
        if self.grapheme_col == 0 {
            if self.row == 0 {
                return;
            }
            let current = self.lines.remove(self.row);
            self.row -= 1;
            self.grapheme_col = grapheme_count(&self.lines[self.row]);
            self.lines[self.row].push_str(&current);
            return;
        }
        let line = &mut self.lines[self.row];
        let start = byte_index(line, self.grapheme_col - 1);
        let end = byte_index(line, self.grapheme_col);
        line.replace_range(start..end, "");
        self.grapheme_col -= 1;
    }

    pub fn move_left(&mut self) {
        self.normalize_cursor();
        if self.grapheme_col > 0 {
            self.grapheme_col -= 1;
        } else if self.row > 0 {
            self.row -= 1;
            self.grapheme_col = grapheme_count(&self.lines[self.row]);
        }
    }

    pub fn move_right(&mut self) {
        self.normalize_cursor();
        let count = grapheme_count(&self.lines[self.row]);
        if self.grapheme_col < count {
            self.grapheme_col += 1;
        } else if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.grapheme_col = 0;
        }
    }

    pub fn body(&self) -> String {
        self.lines.join("\n")
    }

    fn normalize_cursor(&mut self) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.row = self.row.min(self.lines.len() - 1);
        self.grapheme_col = self.grapheme_col.min(grapheme_count(&self.lines[self.row]));
    }
}

fn grapheme_count(value: &str) -> usize {
    value.graphemes(true).count()
}

fn byte_index(value: &str, grapheme_col: usize) -> usize {
    value
        .grapheme_indices(true)
        .nth(grapheme_col)
        .map(|(index, _)| index)
        .unwrap_or(value.len())
}
