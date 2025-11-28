/// Maps byte offsets to LSP positions (line/column in UTF-16 code units)
#[derive(Debug, Clone)]
pub struct LineIndex {
    /// Byte offset of the start of each line
    line_starts: Vec<u32>,
    /// The source text for UTF-16 conversion
    text: String,
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
            text: text.to_string(),
        }
    }

    /// Convert byte offset to (line, column) where column is UTF-16 code units
    pub fn line_col(&self, byte_offset: u32) -> (u32, u32) {
        let byte_offset = byte_offset as usize;
        let line = self
            .line_starts
            .partition_point(|&start| (start as usize) <= byte_offset)
            .saturating_sub(1);
        let line_start = self.line_starts[line] as usize;

        // Convert byte offset to UTF-16 code units
        let line_text = &self.text[line_start..byte_offset.min(self.text.len())];
        let utf16_col = line_text.encode_utf16().count() as u32;

        (line as u32, utf16_col)
    }

    /// Convert (line, column in UTF-16 code units) to byte offset
    pub fn offset(&self, line: u32, utf16_col: u32) -> u32 {
        let line_start = self
            .line_starts
            .get(line as usize)
            .copied()
            .unwrap_or(self.text.len() as u32) as usize;

        let line_end = self
            .line_starts
            .get(line as usize + 1)
            .map(|&end| (end as usize).saturating_sub(1)) // exclude newline
            .unwrap_or(self.text.len());

        let line_text = &self.text[line_start..line_end];

        // Convert UTF-16 column to byte offset
        let mut utf16_count = 0u32;
        let mut byte_offset = line_start;

        for ch in line_text.chars() {
            if utf16_count >= utf16_col {
                break;
            }
            utf16_count += ch.len_utf16() as u32;
            byte_offset += ch.len_utf8();
        }

        byte_offset as u32
    }

    /// Get byte offset to UTF-16 length for a byte range (for semantic tokens)
    pub fn utf16_len(&self, byte_start: usize, byte_end: usize) -> u32 {
        let text = &self.text[byte_start.min(self.text.len())..byte_end.min(self.text.len())];
        text.encode_utf16().count() as u32
    }

    /// Get the byte range for a line
    pub fn line_range(&self, line: u32) -> std::ops::Range<u32> {
        let start = self
            .line_starts
            .get(line as usize)
            .copied()
            .unwrap_or(self.text.len() as u32);
        let end = self
            .line_starts
            .get(line as usize + 1)
            .copied()
            .unwrap_or(self.text.len() as u32);
        start..end
    }

    /// Get total number of lines
    pub fn len(&self) -> usize {
        self.line_starts.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_index_ascii() {
        let text = "line1\nline2\nline3";
        let index = LineIndex::new(text);

        assert_eq!(index.line_col(0), (0, 0));
        assert_eq!(index.line_col(5), (0, 5));
        assert_eq!(index.line_col(6), (1, 0));
        assert_eq!(index.line_col(11), (1, 5));
        assert_eq!(index.line_col(12), (2, 0));
    }

    #[test]
    fn test_offset_conversion_ascii() {
        let text = "line1\nline2\nline3";
        let index = LineIndex::new(text);

        assert_eq!(index.offset(0, 0), 0);
        assert_eq!(index.offset(1, 0), 6);
        assert_eq!(index.offset(2, 0), 12);
        assert_eq!(index.offset(2, 5), 17);
    }

    #[test]
    fn test_utf8_multibyte() {
        // "Café" - é is 2 bytes in UTF-8, 1 UTF-16 code unit
        let text = "Café";
        let index = LineIndex::new(text);

        // Byte offsets: C=0, a=1, f=2, é=3-4
        // UTF-16 cols:  C=0, a=1, f=2, é=3
        assert_eq!(index.line_col(0), (0, 0)); // C
        assert_eq!(index.line_col(1), (0, 1)); // a
        assert_eq!(index.line_col(2), (0, 2)); // f
        assert_eq!(index.line_col(3), (0, 3)); // é start
        assert_eq!(index.line_col(5), (0, 4)); // after é (byte 5 = after the 2-byte é)
    }

    #[test]
    fn test_utf8_emoji() {
        // 🍳 is 4 bytes in UTF-8, 2 UTF-16 code units (surrogate pair)
        let text = "A🍳B";
        let index = LineIndex::new(text);

        // Byte offsets: A=0, 🍳=1-4, B=5
        // UTF-16 cols:  A=0, 🍳=1-2, B=3
        assert_eq!(index.line_col(0), (0, 0)); // A
        assert_eq!(index.line_col(1), (0, 1)); // 🍳 start
        assert_eq!(index.line_col(5), (0, 3)); // B
    }

    #[test]
    fn test_utf16_len() {
        let text = "Café🍳";
        let index = LineIndex::new(text);

        // "Caf" = 3 UTF-16 units
        assert_eq!(index.utf16_len(0, 3), 3);
        // "é" = 1 UTF-16 unit (2 bytes)
        assert_eq!(index.utf16_len(3, 5), 1);
        // "🍳" = 2 UTF-16 units (4 bytes, surrogate pair)
        assert_eq!(index.utf16_len(5, 9), 2);
    }

    #[test]
    fn test_offset_from_utf16() {
        let text = "Café";
        let index = LineIndex::new(text);

        assert_eq!(index.offset(0, 0), 0); // C
        assert_eq!(index.offset(0, 1), 1); // a
        assert_eq!(index.offset(0, 2), 2); // f
        assert_eq!(index.offset(0, 3), 3); // é
        assert_eq!(index.offset(0, 4), 5); // end
    }

    #[test]
    fn test_empty_text() {
        let text = "";
        let index = LineIndex::new(text);
        assert_eq!(index.line_col(0), (0, 0));
    }

    #[test]
    fn test_chinese_characters() {
        // 中文 - each character is 3 bytes in UTF-8, 1 UTF-16 code unit
        let text = "中文";
        let index = LineIndex::new(text);

        assert_eq!(index.line_col(0), (0, 0)); // 中 start
        assert_eq!(index.line_col(3), (0, 1)); // 文 start
        assert_eq!(index.line_col(6), (0, 2)); // end
    }
}
