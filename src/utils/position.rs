use tower_lsp::lsp_types::{Position, Range};

use crate::utils::line_index::LineIndex;

/// Convert byte offsets to an LSP Range
pub fn span_to_range(start: usize, end: usize, line_index: &LineIndex) -> Range {
    let (start_line, start_col) = line_index.line_col(start as u32);
    let (end_line, end_col) = line_index.line_col(end as u32);
    Range {
        start: Position {
            line: start_line,
            character: start_col,
        },
        end: Position {
            line: end_line,
            character: end_col,
        },
    }
}

/// Convert an LSP Position to a byte offset
pub fn position_to_offset(pos: Position, line_index: &LineIndex) -> usize {
    line_index.offset(pos.line, pos.character) as usize
}

/// Check if a position is within a range
pub fn position_in_range(pos: Position, range: Range) -> bool {
    (pos.line > range.start.line
        || (pos.line == range.start.line && pos.character >= range.start.character))
        && (pos.line < range.end.line
            || (pos.line == range.end.line && pos.character <= range.end.character))
}
