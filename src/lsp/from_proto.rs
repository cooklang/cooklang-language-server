//! Conversion from LSP types to internal types.
//!
//! This module handles all conversions from lsp_types to our internal
//! representations, including position encoding (UTF-16 to UTF-8 byte offsets).

use anyhow::{format_err, Result};
use text_size::{TextRange, TextSize};
use tower_lsp::lsp_types;

use crate::utils::line_index::LineIndex;

/// The position encoding used by the LSP client.
#[derive(Debug, Clone, Copy, Default)]
pub enum PositionEncoding {
    /// UTF-8 byte offsets (rare, but some clients support it)
    Utf8,
    /// UTF-16 code units (default, used by most editors)
    #[default]
    Utf16,
}

/// Convert an LSP Position to a byte offset in the document.
pub fn offset(
    line_index: &LineIndex,
    position: lsp_types::Position,
    encoding: PositionEncoding,
) -> Result<TextSize> {
    let line = position.line;
    let col = position.character;

    // Get the byte offset of the start of the line
    let line_start = line_index.line_start(line).ok_or_else(|| {
        format_err!(
            "Invalid line {} (document has {} lines)",
            line,
            line_index.line_count()
        )
    })?;

    // Convert the column based on encoding
    let col_offset = match encoding {
        PositionEncoding::Utf8 => TextSize::from(col),
        PositionEncoding::Utf16 => line_index
            .utf16_to_utf8_col(line, col)
            .map(TextSize::from)
            .ok_or_else(|| format_err!("Invalid UTF-16 column {} on line {}", col, line))?,
    };

    Ok(line_start + col_offset)
}

/// Convert an LSP Range to a TextRange.
pub fn text_range(
    line_index: &LineIndex,
    range: lsp_types::Range,
    encoding: PositionEncoding,
) -> Result<TextRange> {
    let start = offset(line_index, range.start, encoding)?;
    let end = offset(line_index, range.end, encoding)?;

    if end < start {
        return Err(format_err!("Invalid range: end before start"));
    }

    Ok(TextRange::new(start, end))
}

/// Convert an LSP Position directly to (line, column) in UTF-8 bytes.
pub fn line_col(
    line_index: &LineIndex,
    position: lsp_types::Position,
    encoding: PositionEncoding,
) -> Result<(u32, u32)> {
    let line = position.line;
    let col = match encoding {
        PositionEncoding::Utf8 => position.character,
        PositionEncoding::Utf16 => line_index
            .utf16_to_utf8_col(line, position.character)
            .ok_or_else(|| {
                format_err!(
                    "Invalid UTF-16 column {} on line {}",
                    position.character,
                    line
                )
            })?,
    };

    Ok((line, col))
}
