//! Conversion from internal types to LSP types.
//!
//! This module handles all conversions from our internal representations
//! to lsp_types, including position encoding (UTF-8 byte offsets to UTF-16).

use text_size::{TextRange, TextSize};
use tower_lsp::lsp_types;

use crate::lsp::from_proto::PositionEncoding;
use crate::utils::line_index::LineIndex;

/// Convert a byte offset to an LSP Position.
pub fn position(
    line_index: &LineIndex,
    offset: TextSize,
    encoding: PositionEncoding,
) -> lsp_types::Position {
    let (line, col) = line_index.line_col(u32::from(offset));

    let character = match encoding {
        PositionEncoding::Utf8 => col,
        PositionEncoding::Utf16 => line_index.utf8_to_utf16_col(line, col),
    };

    lsp_types::Position::new(line, character)
}

/// Convert a TextRange to an LSP Range.
pub fn range(
    line_index: &LineIndex,
    range: TextRange,
    encoding: PositionEncoding,
) -> lsp_types::Range {
    let start = position(line_index, range.start(), encoding);
    let end = position(line_index, range.end(), encoding);
    lsp_types::Range::new(start, end)
}

/// Convert a span (start, end byte offsets) to an LSP Range.
pub fn span_to_range(
    line_index: &LineIndex,
    start: usize,
    end: usize,
    encoding: PositionEncoding,
) -> lsp_types::Range {
    let start_pos = position(line_index, TextSize::from(start as u32), encoding);
    let end_pos = position(line_index, TextSize::from(end as u32), encoding);
    lsp_types::Range::new(start_pos, end_pos)
}

/// Severity conversion from cooklang to LSP.
pub fn diagnostic_severity(severity: cooklang::error::Severity) -> lsp_types::DiagnosticSeverity {
    match severity {
        cooklang::error::Severity::Error => lsp_types::DiagnosticSeverity::ERROR,
        cooklang::error::Severity::Warning => lsp_types::DiagnosticSeverity::WARNING,
    }
}

/// Symbol kind for Cooklang elements.
pub mod symbol_kind {
    use tower_lsp::lsp_types::SymbolKind;

    pub const INGREDIENT: SymbolKind = SymbolKind::VARIABLE;
    pub const COOKWARE: SymbolKind = SymbolKind::CLASS;
    pub const TIMER: SymbolKind = SymbolKind::FUNCTION;
    pub const SECTION: SymbolKind = SymbolKind::NAMESPACE;
    pub const METADATA: SymbolKind = SymbolKind::PROPERTY;
}

/// Completion item kind for Cooklang elements.
pub mod completion_kind {
    use tower_lsp::lsp_types::CompletionItemKind;

    pub const INGREDIENT: CompletionItemKind = CompletionItemKind::VARIABLE;
    pub const COOKWARE: CompletionItemKind = CompletionItemKind::CLASS;
    pub const TIMER: CompletionItemKind = CompletionItemKind::FUNCTION;
    pub const UNIT: CompletionItemKind = CompletionItemKind::UNIT;
    pub const SNIPPET: CompletionItemKind = CompletionItemKind::SNIPPET;
}
