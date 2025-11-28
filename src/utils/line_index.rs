/// Maps byte offsets to line/column positions and vice versa
#[derive(Debug, Clone)]
pub struct LineIndex {
    /// Byte offset of the start of each line
    line_starts: Vec<u32>,
    /// Total length of the text in bytes
    len: u32,
}

impl LineIndex {
    pub fn new(text: &str) -> Self {
        let mut line_starts = vec![0];
        for (idx, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push((idx + 1) as u32);
            }
        }
        Self {
            line_starts,
            len: text.len() as u32,
        }
    }

    /// Convert byte offset to (line, column) - both 0-indexed
    pub fn line_col(&self, offset: u32) -> (u32, u32) {
        let line = self
            .line_starts
            .partition_point(|&start| start <= offset)
            .saturating_sub(1);
        let line_start = self.line_starts[line];
        let col = offset - line_start;
        (line as u32, col)
    }

    /// Convert (line, column) to byte offset
    pub fn offset(&self, line: u32, col: u32) -> u32 {
        let line_start = self
            .line_starts
            .get(line as usize)
            .copied()
            .unwrap_or(self.len);
        (line_start + col).min(self.len)
    }

    /// Get the byte range for a line
    pub fn line_range(&self, line: u32) -> std::ops::Range<u32> {
        let start = self
            .line_starts
            .get(line as usize)
            .copied()
            .unwrap_or(self.len);
        let end = self
            .line_starts
            .get(line as usize + 1)
            .copied()
            .unwrap_or(self.len);
        start..end
    }

    /// Get total number of lines
    pub fn len(&self) -> usize {
        self.line_starts.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_index_simple() {
        let text = "line1\nline2\nline3";
        let index = LineIndex::new(text);

        assert_eq!(index.line_col(0), (0, 0));
        assert_eq!(index.line_col(5), (0, 5));
        assert_eq!(index.line_col(6), (1, 0));
        assert_eq!(index.line_col(11), (1, 5));
        assert_eq!(index.line_col(12), (2, 0));
    }

    #[test]
    fn test_offset_conversion() {
        let text = "line1\nline2\nline3";
        let index = LineIndex::new(text);

        assert_eq!(index.offset(0, 0), 0);
        assert_eq!(index.offset(1, 0), 6);
        assert_eq!(index.offset(2, 0), 12);
        assert_eq!(index.offset(2, 5), 17);
    }

    #[test]
    fn test_empty_text() {
        let text = "";
        let index = LineIndex::new(text);
        assert_eq!(index.line_col(0), (0, 0));
    }
}
