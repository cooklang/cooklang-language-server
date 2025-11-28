use cooklang::{CooklangParser, Extensions, Recipe};
use cooklang::error::SourceDiag;
use tower_lsp::lsp_types::Url;

use crate::utils::line_index::LineIndex;

/// Represents a parsed Cooklang document
#[derive(Debug)]
pub struct Document {
    pub uri: Url,
    pub version: i32,
    pub content: String,
    pub line_index: LineIndex,
    pub parse_result: Option<ParseResult>,
    /// Errors from parsing, stored even if parse completely failed
    pub parse_errors: Vec<SourceDiag>,
    /// Warnings from parsing
    pub parse_warnings: Vec<SourceDiag>,
}

#[derive(Debug, Clone)]
pub struct ParseResult {
    pub recipe: Recipe,
    pub errors: Vec<SourceDiag>,
    pub warnings: Vec<SourceDiag>,
}

impl Document {
    pub fn new(uri: Url, version: i32, content: String) -> Self {
        let line_index = LineIndex::new(&content);
        let mut doc = Self {
            uri,
            version,
            content,
            line_index,
            parse_result: None,
            parse_errors: Vec::new(),
            parse_warnings: Vec::new(),
        };
        doc.reparse();
        doc
    }

    pub fn update(&mut self, version: i32, content: String) {
        self.version = version;
        self.content = content;
        self.line_index = LineIndex::new(&self.content);
        self.reparse();
    }

    fn reparse(&mut self) {
        let parser = CooklangParser::new(Extensions::all(), Default::default());
        let result = parser.parse(&self.content);

        // Get errors and warnings from the report
        let report = result.report();
        let errors: Vec<_> = report.errors().cloned().collect();
        let warnings: Vec<_> = report.warnings().cloned().collect();

        // Always store errors and warnings at document level
        self.parse_errors = errors.clone();
        self.parse_warnings = warnings.clone();

        // Get the recipe output if available
        if let Some(recipe) = result.output().cloned() {
            self.parse_result = Some(ParseResult {
                recipe,
                errors,
                warnings,
            });
        } else {
            // Clear parse result but errors are still in parse_errors
            self.parse_result = None;
        }
    }
}
