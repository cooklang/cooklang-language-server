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
        self.parse_errors = report.errors().cloned().collect();
        self.parse_warnings = report.warnings().cloned().collect();

        // Get the recipe output if available
        self.parse_result = result.output().cloned().map(|recipe| ParseResult { recipe });
    }
}
