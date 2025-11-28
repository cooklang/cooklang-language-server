use cooklang::error::{SourceDiag, Severity};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

use crate::document::Document;
use crate::utils::position::span_to_range;

pub fn get_diagnostics(doc: &Document) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    // Always use document-level errors/warnings (available even when parse fails)
    for error in &doc.parse_errors {
        if let Some(diag) = convert_source_diag(error, &doc.line_index) {
            diagnostics.push(diag);
        }
    }

    for warning in &doc.parse_warnings {
        if let Some(diag) = convert_source_diag(warning, &doc.line_index) {
            diagnostics.push(diag);
        }
    }

    // If no parse result and no specific errors, show a generic message
    if doc.parse_result.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            range: tower_lsp::lsp_types::Range::default(),
            severity: Some(DiagnosticSeverity::ERROR),
            source: Some("cooklang".into()),
            message: "Failed to parse recipe".into(),
            ..Default::default()
        });
    }

    diagnostics
}

fn convert_source_diag(
    diag: &SourceDiag,
    line_index: &crate::utils::line_index::LineIndex,
) -> Option<Diagnostic> {
    // Get the primary span from the first label
    let range = diag
        .labels
        .first()
        .map(|(span, _)| span_to_range(span.start(), span.end(), line_index))
        .unwrap_or_default();

    let severity = match diag.severity {
        Severity::Error => DiagnosticSeverity::ERROR,
        Severity::Warning => DiagnosticSeverity::WARNING,
    };

    Some(Diagnostic {
        range,
        severity: Some(severity),
        source: Some("cooklang".into()),
        message: diag.message.to_string(),
        ..Default::default()
    })
}
