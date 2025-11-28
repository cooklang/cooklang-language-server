# Cooklang Language Server - Implementation Plan

A comprehensive plan for building an LSP server for the Cooklang recipe markup language using Rust.

## Overview

| Aspect | Details |
|--------|---------|
| **Language** | Rust |
| **LSP Framework** | tower-lsp v0.20 |
| **Parser** | cooklang v0.17 |
| **Recipe Discovery** | cooklang-find v0.5 |
| **Target Editors** | VS Code, Neovim, Emacs, Helix |

---

## Milestone 1: Project Foundation

### 1.1 Initialize Project Structure

Create the Cargo project with the following structure:

```
cooklang-language-server/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── LICENSE
├── .gitignore
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── backend.rs
│   ├── state.rs
│   ├── document.rs
│   └── utils/
│       ├── mod.rs
│       ├── line_index.rs
│       └── position.rs
├── tests/
│   ├── common/
│   │   └── mod.rs
│   └── integration_test.rs
└── editors/
    └── vscode/
        ├── package.json
        └── src/
            └── extension.ts
```

### 1.2 Cargo.toml Configuration

```toml
[package]
name = "cooklang-language-server"
version = "0.1.0"
edition = "2021"
license = "MIT"
description = "Language Server Protocol implementation for Cooklang"
repository = "https://github.com/cooklang/cooklang-language-server"
keywords = ["cooklang", "lsp", "language-server", "recipes"]
categories = ["development-tools", "text-editors"]

[dependencies]
# LSP
tower-lsp = "0.20"
lsp-types = "0.94"

# Async runtime
tokio = { version = "1", features = ["full"] }

# Cooklang
cooklang = "0.17"
cooklang-find = "0.5"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Error handling
thiserror = "2"
anyhow = "1.0"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Utilities
dashmap = "5"  # Concurrent HashMap
parking_lot = "0.12"  # Better mutexes

[dev-dependencies]
tempfile = "3"
tokio-test = "0.4"

[[bin]]
name = "cooklang-lsp"
path = "src/main.rs"

[profile.release]
lto = true
codegen-units = 1
strip = true
```

### 1.3 Implement Core Entry Point

**src/main.rs**
```rust
use tower_lsp::{LspService, Server};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod backend;
mod state;
mod document;
mod utils;

use backend::CooklangBackend;

#[tokio::main]
async fn main() {
    // Initialize logging to stderr (stdout is for LSP)
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(CooklangBackend::new);

    Server::new(stdin, stdout, socket)
        .serve(service)
        .await;
}
```

### 1.4 Implement Backend Skeleton

**src/backend.rs**
```rust
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use crate::state::ServerState;

pub struct CooklangBackend {
    client: Client,
    state: ServerState,
}

impl CooklangBackend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            state: ServerState::new(),
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for CooklangBackend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "cooklang-language-server".into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        tracing::info!("Cooklang LSP initialized");
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}
```

### 1.5 Tasks Checklist

- [ ] Create project with `cargo new cooklang-language-server`
- [ ] Configure Cargo.toml with all dependencies
- [ ] Implement main.rs with logging setup
- [ ] Implement minimal backend.rs
- [ ] Implement state.rs for server state
- [ ] Test server starts and responds to initialize
- [ ] Add .gitignore and LICENSE

---

## Milestone 2: Document Management

### 2.1 Line Index Implementation

**src/utils/line_index.rs**
```rust
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
        let line = self.line_starts
            .partition_point(|&start| start <= offset)
            .saturating_sub(1);
        let line_start = self.line_starts[line];
        let col = offset - line_start;
        (line as u32, col)
    }

    /// Convert (line, column) to byte offset
    pub fn offset(&self, line: u32, col: u32) -> u32 {
        let line_start = self.line_starts
            .get(line as usize)
            .copied()
            .unwrap_or(self.len);
        (line_start + col).min(self.len)
    }

    /// Get the byte range for a line
    pub fn line_range(&self, line: u32) -> std::ops::Range<u32> {
        let start = self.line_starts.get(line as usize).copied().unwrap_or(self.len);
        let end = self.line_starts.get(line as usize + 1).copied().unwrap_or(self.len);
        start..end
    }
}
```

### 2.2 Position Conversion Utilities

**src/utils/position.rs**
```rust
use lsp_types::{Position, Range};
use crate::utils::line_index::LineIndex;

/// Convert a cooklang Span to an LSP Range
pub fn span_to_range(start: usize, end: usize, line_index: &LineIndex) -> Range {
    let (start_line, start_col) = line_index.line_col(start as u32);
    let (end_line, end_col) = line_index.line_col(end as u32);
    Range {
        start: Position { line: start_line, character: start_col },
        end: Position { line: end_line, character: end_col },
    }
}

/// Convert an LSP Position to a byte offset
pub fn position_to_offset(pos: Position, line_index: &LineIndex) -> usize {
    line_index.offset(pos.line, pos.character) as usize
}

/// Check if a position is within a range
pub fn position_in_range(pos: Position, range: Range) -> bool {
    (pos.line > range.start.line ||
     (pos.line == range.start.line && pos.character >= range.start.character)) &&
    (pos.line < range.end.line ||
     (pos.line == range.end.line && pos.character <= range.end.character))
}
```

### 2.3 Document State

**src/document.rs**
```rust
use cooklang::{CooklangParser, Extensions, ScalableRecipe};
use cooklang::error::SourceReport;
use lsp_types::Url;

use crate::utils::line_index::LineIndex;

/// Represents a parsed Cooklang document
#[derive(Debug)]
pub struct Document {
    pub uri: Url,
    pub version: i32,
    pub content: String,
    pub line_index: LineIndex,
    pub parse_result: Option<ParseResult>,
}

#[derive(Debug)]
pub struct ParseResult {
    pub recipe: ScalableRecipe,
    pub report: SourceReport,
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
        match parser.parse(&self.content) {
            Ok((recipe, report)) => {
                self.parse_result = Some(ParseResult { recipe, report });
            }
            Err(report) => {
                // Store just the errors without a recipe
                self.parse_result = None;
                tracing::warn!("Parse failed: {:?}", report);
            }
        }
    }
}
```

### 2.4 Server State

**src/state.rs**
```rust
use dashmap::DashMap;
use lsp_types::Url;
use cooklang::{CooklangParser, Extensions, Converter};

use crate::document::Document;

/// Thread-safe server state
pub struct ServerState {
    pub documents: DashMap<Url, Document>,
    pub parser: CooklangParser,
}

impl ServerState {
    pub fn new() -> Self {
        Self {
            documents: DashMap::new(),
            parser: CooklangParser::new(Extensions::all(), Converter::default()),
        }
    }

    pub fn open_document(&self, uri: Url, version: i32, content: String) {
        let doc = Document::new(uri.clone(), version, content);
        self.documents.insert(uri, doc);
    }

    pub fn update_document(&self, uri: &Url, version: i32, content: String) {
        if let Some(mut doc) = self.documents.get_mut(uri) {
            doc.update(version, content);
        }
    }

    pub fn close_document(&self, uri: &Url) {
        self.documents.remove(uri);
    }

    pub fn get_document(&self, uri: &Url) -> Option<dashmap::mapref::one::Ref<'_, Url, Document>> {
        self.documents.get(uri)
    }
}
```

### 2.5 Implement Document Sync in Backend

Add to **src/backend.rs**:
```rust
async fn did_open(&self, params: DidOpenTextDocumentParams) {
    let uri = params.text_document.uri;
    let version = params.text_document.version;
    let content = params.text_document.text;

    self.state.open_document(uri.clone(), version, content);
    self.publish_diagnostics(&uri).await;
}

async fn did_change(&self, params: DidChangeTextDocumentParams) {
    let uri = params.text_document.uri;
    let version = params.text_document.version;

    // Using FULL sync, so take the last content change
    if let Some(change) = params.content_changes.into_iter().last() {
        self.state.update_document(&uri, version, change.text);
        self.publish_diagnostics(&uri).await;
    }
}

async fn did_save(&self, params: DidSaveTextDocumentParams) {
    // Optionally re-validate on save
    self.publish_diagnostics(&params.text_document.uri).await;
}

async fn did_close(&self, params: DidCloseTextDocumentParams) {
    let uri = params.text_document.uri;
    self.state.close_document(&uri);
    // Clear diagnostics for closed file
    self.client.publish_diagnostics(uri, vec![], None).await;
}
```

### 2.6 Tasks Checklist

- [ ] Implement LineIndex with comprehensive tests
- [ ] Implement position conversion utilities
- [ ] Implement Document struct with parsing
- [ ] Implement ServerState with DashMap
- [ ] Add did_open handler
- [ ] Add did_change handler
- [ ] Add did_close handler
- [ ] Test document lifecycle

---

## Milestone 3: Diagnostics

### 3.1 Diagnostic Conversion Module

**src/diagnostics.rs**
```rust
use cooklang::error::{SourceDiag, Severity};
use lsp_types::{Diagnostic, DiagnosticSeverity, DiagnosticTag, NumberOrString};

use crate::document::Document;
use crate::utils::position::span_to_range;

pub fn convert_diagnostics(doc: &Document) -> Vec<Diagnostic> {
    let Some(ref parse_result) = doc.parse_result else {
        // If parsing completely failed, report a general error
        return vec![Diagnostic {
            range: lsp_types::Range::default(),
            severity: Some(DiagnosticSeverity::ERROR),
            source: Some("cooklang".into()),
            message: "Failed to parse recipe".into(),
            ..Default::default()
        }];
    };

    parse_result.report
        .iter()
        .map(|diag| convert_single_diagnostic(diag, &doc.line_index))
        .collect()
}

fn convert_single_diagnostic(
    diag: &SourceDiag,
    line_index: &crate::utils::line_index::LineIndex,
) -> Diagnostic {
    let severity = match diag.severity() {
        Severity::Error => DiagnosticSeverity::ERROR,
        Severity::Warning => DiagnosticSeverity::WARNING,
    };

    // Get the primary span from the first label
    let range = diag
        .labels()
        .first()
        .map(|(span, _)| span_to_range(span.start(), span.end(), line_index))
        .unwrap_or_default();

    // Build related information from additional labels
    let related_information = if diag.labels().len() > 1 {
        Some(
            diag.labels()
                .iter()
                .skip(1)
                .filter_map(|(span, hint)| {
                    hint.as_ref().map(|msg| lsp_types::DiagnosticRelatedInformation {
                        location: lsp_types::Location {
                            uri: lsp_types::Url::parse("file:///").unwrap(), // Will be filled by caller
                            range: span_to_range(span.start(), span.end(), line_index),
                        },
                        message: msg.clone(),
                    })
                })
                .collect(),
        )
    } else {
        None
    };

    Diagnostic {
        range,
        severity: Some(severity),
        code: None, // Could add error codes later
        code_description: None,
        source: Some("cooklang".into()),
        message: diag.message().to_string(),
        related_information,
        tags: None,
        data: None,
    }
}
```

### 3.2 Publishing Diagnostics

Add to **src/backend.rs**:
```rust
use crate::diagnostics::convert_diagnostics;

impl CooklangBackend {
    async fn publish_diagnostics(&self, uri: &Url) {
        let diagnostics = if let Some(doc) = self.state.get_document(uri) {
            convert_diagnostics(&doc)
        } else {
            vec![]
        };

        self.client
            .publish_diagnostics(uri.clone(), diagnostics, None)
            .await;
    }
}
```

### 3.3 Update Initialize Capabilities

```rust
capabilities: ServerCapabilities {
    text_document_sync: Some(TextDocumentSyncCapability::Options(
        TextDocumentSyncOptions {
            open_close: Some(true),
            change: Some(TextDocumentSyncKind::FULL),
            save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                include_text: Some(false),
            })),
            ..Default::default()
        },
    )),
    // Diagnostics are push-based, no capability needed
    ..Default::default()
}
```

### 3.4 Tasks Checklist

- [ ] Implement diagnostics.rs module
- [ ] Map cooklang Severity to LSP DiagnosticSeverity
- [ ] Convert spans to LSP ranges correctly
- [ ] Handle related information from labels
- [ ] Implement publish_diagnostics method
- [ ] Test with various error cases
- [ ] Test incremental diagnostic updates

---

## Milestone 4: Semantic Tokens (Syntax Highlighting)

### 4.1 Semantic Token Types Definition

**src/semantic_tokens.rs**
```rust
use lsp_types::{
    SemanticToken, SemanticTokenType, SemanticTokenModifier,
    SemanticTokensLegend, SemanticTokensOptions, SemanticTokensFullOptions,
    SemanticTokensServerCapabilities,
};
use cooklang::parser::{PullParser, Event, BlockKind};
use cooklang::Extensions;

use crate::document::Document;
use crate::utils::line_index::LineIndex;

// Define token types
pub const TOKEN_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::VARIABLE,    // 0: Ingredients (@)
    SemanticTokenType::CLASS,       // 1: Cookware (#)
    SemanticTokenType::FUNCTION,    // 2: Timers (~)
    SemanticTokenType::NUMBER,      // 3: Quantities
    SemanticTokenType::STRING,      // 4: Units
    SemanticTokenType::COMMENT,     // 5: Comments
    SemanticTokenType::KEYWORD,     // 6: Metadata keys
    SemanticTokenType::PROPERTY,    // 7: Metadata values
    SemanticTokenType::NAMESPACE,   // 8: Sections
    SemanticTokenType::OPERATOR,    // 9: Markers (@, #, ~, {, }, %)
];

pub const TOKEN_MODIFIERS: &[SemanticTokenModifier] = &[
    SemanticTokenModifier::DECLARATION,  // First use
    SemanticTokenModifier::DEFINITION,   // Definition
    SemanticTokenModifier::READONLY,     // Locked quantities (=)
];

pub fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: TOKEN_TYPES.to_vec(),
        token_modifiers: TOKEN_MODIFIERS.to_vec(),
    }
}

pub fn capabilities() -> SemanticTokensServerCapabilities {
    SemanticTokensServerCapabilities::SemanticTokensOptions(SemanticTokensOptions {
        legend: legend(),
        full: Some(SemanticTokensFullOptions::Bool(true)),
        range: Some(false),
        work_done_progress_options: Default::default(),
    })
}
```

### 4.2 Token Generation

```rust
// Continue in src/semantic_tokens.rs

#[derive(Debug)]
struct TokenBuilder {
    tokens: Vec<SemanticToken>,
    prev_line: u32,
    prev_start: u32,
}

impl TokenBuilder {
    fn new() -> Self {
        Self {
            tokens: Vec::new(),
            prev_line: 0,
            prev_start: 0,
        }
    }

    fn push(&mut self, line: u32, start: u32, length: u32, token_type: u32, modifiers: u32) {
        let delta_line = line - self.prev_line;
        let delta_start = if delta_line == 0 {
            start - self.prev_start
        } else {
            start
        };

        self.tokens.push(SemanticToken {
            delta_line,
            delta_start,
            length,
            token_type,
            token_modifiers_bitset: modifiers,
        });

        self.prev_line = line;
        self.prev_start = start;
    }

    fn build(self) -> Vec<SemanticToken> {
        self.tokens
    }
}

pub fn compute_tokens(doc: &Document) -> Vec<SemanticToken> {
    let mut builder = TokenBuilder::new();
    let parser = PullParser::new(&doc.content, Extensions::all());

    for event in parser {
        match event {
            Event::Ingredient { name, quantity, unit, .. } => {
                // Add tokens for ingredient parts
                // Implementation depends on exact span info from parser
            }
            Event::Cookware { name, quantity, .. } => {
                // Similar for cookware
            }
            Event::Timer { name, quantity, unit, .. } => {
                // Similar for timer
            }
            Event::Metadata { key, value, .. } => {
                // Metadata key and value
            }
            Event::Section { name, .. } => {
                // Section header
            }
            // Handle other events
            _ => {}
        }
    }

    builder.build()
}
```

### 4.3 Add to Backend

```rust
async fn semantic_tokens_full(
    &self,
    params: SemanticTokensParams,
) -> Result<Option<SemanticTokensResult>> {
    let uri = params.text_document.uri;

    let tokens = if let Some(doc) = self.state.get_document(&uri) {
        semantic_tokens::compute_tokens(&doc)
    } else {
        vec![]
    };

    Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
        result_id: None,
        data: tokens,
    })))
}
```

### 4.4 Tasks Checklist

- [ ] Define semantic token types for Cooklang elements
- [ ] Implement TokenBuilder for delta encoding
- [ ] Parse document with PullParser for events
- [ ] Map events to token types with positions
- [ ] Handle nested elements (quantity inside ingredient)
- [ ] Add semantic_tokens_full to backend
- [ ] Update initialize capabilities
- [ ] Test with VS Code theme

---

## Milestone 5: Code Completion

### 5.1 Completion Provider

**src/completion.rs**
```rust
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionList, CompletionParams,
    CompletionResponse, Documentation, InsertTextFormat, MarkupContent, MarkupKind,
};

use crate::document::Document;
use crate::state::ServerState;
use crate::utils::position::position_to_offset;

/// Common cooking units
const UNITS: &[(&str, &str)] = &[
    ("g", "grams"),
    ("kg", "kilograms"),
    ("ml", "milliliters"),
    ("l", "liters"),
    ("oz", "ounces"),
    ("lb", "pounds"),
    ("cup", "cups"),
    ("tbsp", "tablespoons"),
    ("tsp", "teaspoons"),
    ("pinch", "pinch"),
    ("clove", "cloves"),
    ("slice", "slices"),
    ("piece", "pieces"),
];

/// Common time units
const TIME_UNITS: &[(&str, &str)] = &[
    ("s", "seconds"),
    ("sec", "seconds"),
    ("min", "minutes"),
    ("minutes", "minutes"),
    ("h", "hours"),
    ("hours", "hours"),
];

/// Common cookware items
const COMMON_COOKWARE: &[&str] = &[
    "pot", "pan", "skillet", "saucepan", "wok",
    "bowl", "mixing bowl", "cutting board", "knife",
    "oven", "stove", "grill", "blender", "food processor",
    "whisk", "spatula", "ladle", "tongs", "colander",
    "baking sheet", "baking dish", "roasting pan",
];

pub fn get_completions(
    doc: &Document,
    params: &CompletionParams,
    state: &ServerState,
) -> Option<CompletionResponse> {
    let offset = position_to_offset(params.text_document_position.position, &doc.line_index);
    let text_before = &doc.content[..offset];

    // Find the trigger context
    let context = find_completion_context(text_before)?;

    let items = match context {
        CompletionContext::Ingredient(prefix) => {
            complete_ingredients(prefix, doc, state)
        }
        CompletionContext::Cookware(prefix) => {
            complete_cookware(prefix, doc)
        }
        CompletionContext::Timer => {
            complete_timer_units()
        }
        CompletionContext::Unit(prefix) => {
            complete_units(prefix)
        }
    };

    Some(CompletionResponse::List(CompletionList {
        is_incomplete: false,
        items,
    }))
}

#[derive(Debug)]
enum CompletionContext {
    Ingredient(String),   // After @
    Cookware(String),     // After #
    Timer,                // After ~
    Unit(String),         // After % or in quantity
}

fn find_completion_context(text: &str) -> Option<CompletionContext> {
    // Find last trigger character
    let chars: Vec<char> = text.chars().collect();

    for i in (0..chars.len()).rev() {
        match chars[i] {
            '@' => {
                let prefix: String = chars[i+1..].iter().collect();
                if !prefix.contains('{') && !prefix.contains('}') {
                    return Some(CompletionContext::Ingredient(prefix));
                }
            }
            '#' => {
                let prefix: String = chars[i+1..].iter().collect();
                if !prefix.contains('{') && !prefix.contains('}') {
                    return Some(CompletionContext::Cookware(prefix));
                }
            }
            '~' => {
                let rest: String = chars[i+1..].iter().collect();
                if !rest.contains('}') {
                    return Some(CompletionContext::Timer);
                }
            }
            '%' => {
                let prefix: String = chars[i+1..].iter().collect();
                if !prefix.contains('}') {
                    return Some(CompletionContext::Unit(prefix));
                }
            }
            '\n' | '\r' => break,
            _ => {}
        }
    }
    None
}

fn complete_ingredients(prefix: &str, doc: &Document, state: &ServerState) -> Vec<CompletionItem> {
    let mut items = Vec::new();

    // Add existing ingredients from current document
    if let Some(ref result) = doc.parse_result {
        for ingredient in &result.recipe.ingredients {
            let name = &ingredient.name;
            if name.to_lowercase().starts_with(&prefix.to_lowercase()) {
                items.push(CompletionItem {
                    label: name.clone(),
                    kind: Some(CompletionItemKind::VARIABLE),
                    detail: Some("Ingredient (from recipe)".into()),
                    insert_text: Some(format!("{}{{", name)),
                    insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                    ..Default::default()
                });
            }
        }
    }

    // Add from other open documents in workspace
    for entry in state.documents.iter() {
        if entry.key() == &doc.uri {
            continue;
        }
        if let Some(ref result) = entry.value().parse_result {
            for ingredient in &result.recipe.ingredients {
                let name = &ingredient.name;
                if name.to_lowercase().starts_with(&prefix.to_lowercase()) {
                    if !items.iter().any(|i| i.label == *name) {
                        items.push(CompletionItem {
                            label: name.clone(),
                            kind: Some(CompletionItemKind::VARIABLE),
                            detail: Some("Ingredient (from workspace)".into()),
                            ..Default::default()
                        });
                    }
                }
            }
        }
    }

    items
}

fn complete_cookware(prefix: &str, doc: &Document) -> Vec<CompletionItem> {
    let mut items = Vec::new();

    // Add existing cookware from document
    if let Some(ref result) = doc.parse_result {
        for cookware in &result.recipe.cookware {
            let name = &cookware.name;
            if name.to_lowercase().starts_with(&prefix.to_lowercase()) {
                items.push(CompletionItem {
                    label: name.clone(),
                    kind: Some(CompletionItemKind::CLASS),
                    detail: Some("Cookware (from recipe)".into()),
                    ..Default::default()
                });
            }
        }
    }

    // Add common cookware
    for &cookware in COMMON_COOKWARE {
        if cookware.to_lowercase().starts_with(&prefix.to_lowercase()) {
            if !items.iter().any(|i| i.label == cookware) {
                items.push(CompletionItem {
                    label: cookware.into(),
                    kind: Some(CompletionItemKind::CLASS),
                    detail: Some("Common cookware".into()),
                    ..Default::default()
                });
            }
        }
    }

    items
}

fn complete_timer_units() -> Vec<CompletionItem> {
    TIME_UNITS
        .iter()
        .map(|(short, long)| CompletionItem {
            label: short.to_string(),
            kind: Some(CompletionItemKind::UNIT),
            detail: Some(long.to_string()),
            documentation: Some(Documentation::String(format!("Time unit: {}", long))),
            ..Default::default()
        })
        .collect()
}

fn complete_units(prefix: &str) -> Vec<CompletionItem> {
    UNITS
        .iter()
        .filter(|(short, _)| short.to_lowercase().starts_with(&prefix.to_lowercase()))
        .map(|(short, long)| CompletionItem {
            label: short.to_string(),
            kind: Some(CompletionItemKind::UNIT),
            detail: Some(long.to_string()),
            ..Default::default()
        })
        .collect()
}
```

### 5.2 Update Backend

```rust
async fn completion(
    &self,
    params: CompletionParams,
) -> Result<Option<CompletionResponse>> {
    let uri = &params.text_document_position.text_document.uri;

    let response = if let Some(doc) = self.state.get_document(uri) {
        completion::get_completions(&doc, &params, &self.state)
    } else {
        None
    };

    Ok(response)
}
```

### 5.3 Update Capabilities

```rust
completion_provider: Some(CompletionOptions {
    trigger_characters: Some(vec![
        "@".into(),  // Ingredients
        "#".into(),  // Cookware
        "~".into(),  // Timers
        "%".into(),  // Units
    ]),
    resolve_provider: Some(false),
    ..Default::default()
}),
```

### 5.4 Tasks Checklist

- [ ] Define common units and cookware lists
- [ ] Implement context detection
- [ ] Complete ingredients from current document
- [ ] Complete ingredients from workspace
- [ ] Complete cookware suggestions
- [ ] Complete unit suggestions after %
- [ ] Complete timer units
- [ ] Add snippet support for full syntax
- [ ] Test trigger characters work

---

## Milestone 6: Hover Information

### 6.1 Hover Provider

**src/hover.rs**
```rust
use lsp_types::{Hover, HoverContents, HoverParams, MarkupContent, MarkupKind, Range};

use crate::document::Document;
use crate::utils::position::position_to_offset;

pub fn get_hover(doc: &Document, params: &HoverParams) -> Option<Hover> {
    let offset = position_to_offset(
        params.text_document_position_params.position,
        &doc.line_index,
    );

    let parse_result = doc.parse_result.as_ref()?;
    let recipe = &parse_result.recipe;

    // Find what's at the cursor position
    // This requires span information from the parser

    // Check ingredients
    for (idx, ingredient) in recipe.ingredients.iter().enumerate() {
        // Check if cursor is within this ingredient's span
        // (Need to track spans during parsing)
        if let Some(hover_info) = format_ingredient_hover(ingredient) {
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: hover_info,
                }),
                range: None, // Could add the ingredient's range
            });
        }
    }

    // Check cookware
    for cookware in &recipe.cookware {
        if let Some(hover_info) = format_cookware_hover(cookware) {
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: hover_info,
                }),
                range: None,
            });
        }
    }

    // Check timers
    for timer in &recipe.timers {
        if let Some(hover_info) = format_timer_hover(timer) {
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: hover_info,
                }),
                range: None,
            });
        }
    }

    None
}

fn format_ingredient_hover(ingredient: &cooklang::model::Ingredient) -> Option<String> {
    let mut parts = Vec::new();

    parts.push(format!("**Ingredient:** {}", ingredient.name));

    if let Some(ref quantity) = ingredient.quantity {
        parts.push(format!("**Quantity:** {}", quantity));
    }

    if let Some(ref alias) = ingredient.alias {
        parts.push(format!("**Alias:** {}", alias));
    }

    if let Some(ref note) = ingredient.note {
        parts.push(format!("**Note:** {}", note));
    }

    Some(parts.join("\n\n"))
}

fn format_cookware_hover(cookware: &cooklang::model::Cookware) -> Option<String> {
    let mut parts = Vec::new();

    parts.push(format!("**Cookware:** {}", cookware.name));

    if let Some(ref quantity) = cookware.quantity {
        parts.push(format!("**Quantity:** {}", quantity));
    }

    Some(parts.join("\n\n"))
}

fn format_timer_hover(timer: &cooklang::model::Timer) -> Option<String> {
    let mut parts = Vec::new();

    if let Some(ref name) = timer.name {
        parts.push(format!("**Timer:** {}", name));
    } else {
        parts.push("**Timer**".to_string());
    }

    if let Some(ref quantity) = timer.quantity {
        parts.push(format!("**Duration:** {}", quantity));
    }

    Some(parts.join("\n\n"))
}
```

### 6.2 Tasks Checklist

- [ ] Implement hover.rs module
- [ ] Format ingredient hover with quantity, alias, note
- [ ] Format cookware hover with quantity
- [ ] Format timer hover with duration
- [ ] Track element spans for position lookup
- [ ] Add hover to backend
- [ ] Update initialize capabilities
- [ ] Test hover in editor

---

## Milestone 7: Document Symbols

### 7.1 Symbol Provider

**src/symbols.rs**
```rust
use lsp_types::{
    DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse,
    SymbolKind, Range,
};

use crate::document::Document;
use crate::utils::position::span_to_range;

pub fn get_document_symbols(doc: &Document) -> Option<DocumentSymbolResponse> {
    let parse_result = doc.parse_result.as_ref()?;
    let recipe = &parse_result.recipe;

    let mut symbols = Vec::new();

    // Add sections
    for section in &recipe.sections {
        let section_name = section.name.clone().unwrap_or_else(|| "Unnamed Section".into());

        let mut children = Vec::new();

        // Add steps as children of sections
        for (idx, content) in section.content.iter().enumerate() {
            if let cooklang::model::Content::Step(step) = content {
                children.push(DocumentSymbol {
                    name: format!("Step {}", idx + 1),
                    kind: SymbolKind::FUNCTION,
                    range: Range::default(), // Need span info
                    selection_range: Range::default(),
                    detail: None,
                    children: None,
                    tags: None,
                    deprecated: None,
                });
            }
        }

        symbols.push(DocumentSymbol {
            name: section_name,
            kind: SymbolKind::NAMESPACE,
            range: Range::default(), // Need span info
            selection_range: Range::default(),
            detail: Some(format!("{} steps", children.len())),
            children: Some(children),
            tags: None,
            deprecated: None,
        });
    }

    // Add ingredients as top-level symbols
    for ingredient in &recipe.ingredients {
        let detail = ingredient.quantity.as_ref().map(|q| q.to_string());

        symbols.push(DocumentSymbol {
            name: ingredient.name.clone(),
            kind: SymbolKind::VARIABLE,
            range: Range::default(),
            selection_range: Range::default(),
            detail,
            children: None,
            tags: None,
            deprecated: None,
        });
    }

    // Add cookware
    for cookware in &recipe.cookware {
        symbols.push(DocumentSymbol {
            name: cookware.name.clone(),
            kind: SymbolKind::CLASS,
            range: Range::default(),
            selection_range: Range::default(),
            detail: None,
            children: None,
            tags: None,
            deprecated: None,
        });
    }

    Some(DocumentSymbolResponse::Nested(symbols))
}
```

### 7.2 Tasks Checklist

- [ ] Implement symbols.rs module
- [ ] Create hierarchy: Sections → Steps
- [ ] Add ingredients as symbols
- [ ] Add cookware as symbols
- [ ] Add timers as symbols
- [ ] Include quantity details
- [ ] Track spans for accurate ranges
- [ ] Add to backend
- [ ] Test outline view

---

## Milestone 8: Advanced Features

### 8.1 Go to Definition

Navigate to first use/definition of an ingredient or cookware.

### 8.2 Find References

Find all uses of an ingredient throughout the recipe.

### 8.3 Rename Symbol

Rename ingredients or cookware across the entire document.

### 8.4 Code Actions

- Fix common issues (missing quantity, invalid unit)
- Convert between units
- Add missing braces

### 8.5 Workspace Support

Use cooklang-find for:
- Cross-file ingredient references
- Recipe search
- Workspace-wide rename

### 8.6 Tasks Checklist

- [ ] Implement goto_definition
- [ ] Implement find_references
- [ ] Implement rename
- [ ] Add quick fixes for diagnostics
- [ ] Integrate cooklang-find for workspace
- [ ] Add workspace symbol search

---

## Milestone 9: Testing

### 9.1 Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_index() {
        let text = "line1\nline2\nline3";
        let index = LineIndex::new(text);

        assert_eq!(index.line_col(0), (0, 0));
        assert_eq!(index.line_col(5), (0, 5));
        assert_eq!(index.line_col(6), (1, 0));
        assert_eq!(index.line_col(12), (2, 0));
    }

    #[test]
    fn test_completion_context() {
        assert!(matches!(
            find_completion_context("Add @pot"),
            Some(CompletionContext::Ingredient(_))
        ));
        assert!(matches!(
            find_completion_context("Use #pan"),
            Some(CompletionContext::Cookware(_))
        ));
    }
}
```

### 9.2 Integration Tests

```rust
#[tokio::test]
async fn test_diagnostics() {
    let doc = Document::new(
        Url::parse("file:///test.cook").unwrap(),
        1,
        "Add @ingredient{bad".into(), // Missing closing brace
    );

    let diagnostics = convert_diagnostics(&doc);
    assert!(!diagnostics.is_empty());
    assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
}

#[tokio::test]
async fn test_full_lsp_flow() {
    // Test initialize -> open -> change -> close cycle
}
```

### 9.3 Tasks Checklist

- [ ] Write unit tests for LineIndex
- [ ] Write unit tests for position conversion
- [ ] Write unit tests for completion context
- [ ] Write integration tests for diagnostics
- [ ] Write integration tests for LSP flow
- [ ] Set up CI with GitHub Actions
- [ ] Add test coverage reporting

---

## Milestone 10: Packaging & Distribution

### 10.1 Release Builds

```yaml
# .github/workflows/release.yml
name: Release
on:
  push:
    tags: ['v*']

jobs:
  build:
    strategy:
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
          - os: macos-latest
            target: x86_64-apple-darwin
          - os: macos-latest
            target: aarch64-apple-darwin
          - os: windows-latest
            target: x86_64-pc-windows-msvc

    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - run: cargo build --release --target ${{ matrix.target }}
      - uses: actions/upload-artifact@v4
        with:
          name: cooklang-lsp-${{ matrix.target }}
          path: target/${{ matrix.target }}/release/cooklang-lsp*
```

### 10.2 VS Code Extension

**editors/vscode/package.json**
```json
{
  "name": "cooklang",
  "displayName": "Cooklang",
  "description": "Cooklang recipe language support",
  "version": "0.1.0",
  "engines": { "vscode": "^1.75.0" },
  "categories": ["Programming Languages"],
  "activationEvents": ["onLanguage:cooklang"],
  "main": "./out/extension.js",
  "contributes": {
    "languages": [{
      "id": "cooklang",
      "extensions": [".cook"],
      "configuration": "./language-configuration.json"
    }],
    "configuration": {
      "title": "Cooklang",
      "properties": {
        "cooklang.serverPath": {
          "type": "string",
          "description": "Path to cooklang-lsp executable"
        }
      }
    }
  }
}
```

### 10.3 Tasks Checklist

- [ ] Set up GitHub Actions for CI
- [ ] Create release workflow for binaries
- [ ] Build for Linux, macOS, Windows
- [ ] Create VS Code extension scaffold
- [ ] Publish extension to marketplace
- [ ] Create Neovim configuration docs
- [ ] Add Helix configuration docs
- [ ] Write user documentation
- [ ] Publish to crates.io

---

## Summary

| Milestone | Description | Priority |
|-----------|-------------|----------|
| 1 | Project Foundation | Critical |
| 2 | Document Management | Critical |
| 3 | Diagnostics | Critical |
| 4 | Semantic Tokens | High |
| 5 | Code Completion | High |
| 6 | Hover Information | Medium |
| 7 | Document Symbols | Medium |
| 8 | Advanced Features | Low |
| 9 | Testing | High |
| 10 | Packaging | High |

### Dependencies Graph

```
M1 (Foundation)
 └─→ M2 (Documents)
      ├─→ M3 (Diagnostics) ─→ M9 (Testing)
      ├─→ M4 (Semantic Tokens)
      ├─→ M5 (Completion)
      ├─→ M6 (Hover)
      └─→ M7 (Symbols)
           └─→ M8 (Advanced)

M9 (Testing) ─→ M10 (Packaging)
```

### Estimated Complexity

| Milestone | Lines of Code (est.) | Complexity |
|-----------|---------------------|------------|
| M1 | ~200 | Low |
| M2 | ~400 | Medium |
| M3 | ~200 | Low |
| M4 | ~400 | Medium |
| M5 | ~500 | Medium |
| M6 | ~200 | Low |
| M7 | ~200 | Low |
| M8 | ~600 | High |
| M9 | ~400 | Medium |
| M10 | ~100 | Low |
| **Total** | **~3200** | - |
