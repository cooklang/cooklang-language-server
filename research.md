# Cooklang Language Server Research

This document outlines the research findings for building a Language Server Protocol (LSP) implementation for Cooklang using Rust.

## Table of Contents

1. [Cooklang Language Overview](#cooklang-language-overview)
2. [Core Dependencies](#core-dependencies)
3. [Cooklang-rs Parser](#cooklang-rs-parser)
4. [Cooklang-find Library](#cooklang-find-library)
5. [Tower-LSP Framework](#tower-lsp-framework)
6. [LSP Capabilities to Implement](#lsp-capabilities-to-implement)
7. [Architecture Design](#architecture-design)
8. [Implementation Plan](#implementation-plan)

---

## Cooklang Language Overview

Cooklang is a markup language for recipes. The syntax includes:

### Syntax Elements

| Element | Syntax | Example |
|---------|--------|---------|
| **Ingredients** | `@name{quantity%unit}` | `@potato{2}`, `@bacon{1%kg}` |
| **Cookware** | `#name{}` | `#pot`, `#potato masher{}` |
| **Timers** | `~name{quantity%unit}` | `~{25%minutes}`, `~eggs{3%minutes}` |
| **Comments** | `-- line comment` or `[- block -]` | `-- Don't burn!` |
| **Metadata** | YAML front matter between `---` | Title, tags, servings |
| **Notes** | Lines starting with `>` | `> Background info` |
| **Sections** | `=Section Name=` | `=Preparation=` |
| **Preparations** | `@ingredient{}(prep)` | `@onion{1}(finely chopped)` |

### Scaling
- Default servings in metadata
- Lock quantities with `=`: `@salt{=1%tsp}`
- Steps are paragraphs separated by blank lines

---

## Core Dependencies

### Required Crates

```toml
[dependencies]
# LSP Framework
tower-lsp = "0.20"
tokio = { version = "1", features = ["full"] }

# Cooklang Parsing
cooklang = "0.17"
cooklang-find = "0.5"  # Optional: for workspace recipe discovery

# LSP Types
lsp-types = "0.94"  # Included via tower-lsp

# Utilities
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = "0.3"
```

---

## Cooklang-rs Parser

**Repository**: https://github.com/cooklang/cooklang-rs
**Crates.io**: https://crates.io/crates/cooklang
**Version**: 0.17.2
**License**: MIT

### Key Features

- Canonical Cooklang parser implementation
- Opt-in extensions (superset of original format)
- Rich error reporting with annotated code spans
- Unit conversion functionality
- Recipe scaling capabilities
- Configuration file parser for aisle specifications

### Core API

#### Parser Construction

```rust
use cooklang::{CooklangParser, Extensions, Converter};

// Full-featured parser
let parser = CooklangParser::new(Extensions::all(), Converter::default());

// Default configuration
let parser = CooklangParser::default();

// Convenience function (creates new parser each time - avoid in loops)
let result = cooklang::parse(input);
```

#### Parsing Methods

```rust
// Full recipe parsing
let result = parser.parse(input);  // Returns (Recipe, warnings)

// Metadata-only parsing (faster)
let metadata = parser.parse_metadata(input);
```

### Data Model

#### Recipe Structure

```rust
// Main types in cooklang::model
struct Recipe {
    metadata: Metadata,
    sections: Vec<Section>,
    ingredients: Vec<Ingredient>,
    cookware: Vec<Cookware>,
    timers: Vec<Timer>,
}

struct Section {
    name: Option<String>,
    content: Vec<Content>,
}

enum Content {
    Step(Step),
    Text(String),
}

struct Step {
    items: Vec<Item>,
}

enum Item {
    Text { value: String },
    Ingredient { index: usize },
    Cookware { index: usize },
    Timer { index: usize },
    InlineQuantity { ... },
}

struct Ingredient {
    name: String,
    alias: Option<String>,
    quantity: Option<Quantity>,
    note: Option<String>,
    modifiers: Modifiers,
}

struct Cookware {
    name: String,
    alias: Option<String>,
    quantity: Option<Quantity>,
    note: Option<String>,
    modifiers: Modifiers,
}

struct Timer {
    name: Option<String>,
    quantity: Option<Quantity>,
}
```

### Location Tracking

#### Span Type

```rust
use cooklang::span::Span;

// Span represents a range in source code
// Contains start and end byte offsets
struct Span {
    start: usize,
    end: usize,
}
```

#### Located Wrapper

```rust
use cooklang::located::Located;

// Wraps values with location information
struct Located<T> {
    value: T,
    span: Span,
}

impl<T> Located<T> {
    fn new(value: T, span: Span) -> Self;
    fn value(&self) -> &T;
    fn span(&self) -> Span;
    fn into_inner(self) -> T;
    fn take_pair(self) -> (T, Span);
    fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Located<U>;
}
```

### Pull Parser (Event-Based)

```rust
use cooklang::parser::PullParser;

// Create pull parser for event-based processing
let parser = PullParser::new(input, extensions);

// Event types
enum Event<'a> {
    YAMLFrontMatter(&'a str),
    Metadata { key: &'a str, value: &'a str },
    Section { name: &'a str },
    Start(BlockKind),
    End(BlockKind),
    Text { value: &'a str },
    Ingredient { ... },
    Cookware { ... },
    Timer { ... },
    Error(SourceDiag),
    Warning(SourceDiag),
}

enum BlockKind {
    Step,
    Text,
}

// Iterate over events
for event in parser {
    match event {
        Event::Ingredient { name, quantity, .. } => { /* handle */ }
        Event::Error(diag) => { /* report error */ }
        // ...
    }
}
```

### Extensions

```rust
use cooklang::Extensions;

// Available extensions (bitflags)
Extensions::COMPONENT_MODIFIERS    // Enables modifiers
Extensions::COMPONENT_ALIAS        // @igr|alias{} syntax
Extensions::ADVANCED_UNITS         // Extra unit checks, omit % in simple cases
Extensions::MODES                  // >> [mode]: value syntax
Extensions::INLINE_QUANTITIES      // Find quantities in all text
Extensions::RANGE_VALUES           // @igr{2-3} range support
Extensions::TIMER_REQUIRES_TIME    // Timer without time is error
Extensions::INTERMEDIATE_PREPARATIONS
Extensions::COMPAT                 // Subset for compatibility

// Common configurations
Extensions::all()      // All extensions enabled
Extensions::default()  // All extensions (same as all)
Extensions::none()     // No extensions
Extensions::COMPAT     // Maximum compatibility
```

### Error Handling

```rust
use cooklang::error::{SourceDiag, SourceReport, Severity, Label};

// SourceDiag contains diagnostic information
struct SourceDiag {
    severity: Severity,
    message: String,
    labels: Vec<Label>,
    // ... additional fields
}

enum Severity {
    Error,
    Warning,
}

// Label type: (Span, Option<String>)
type Label = (Span, Option<String>);

// Rich error formatting
fn write_rich_error(writer: &mut impl Write, source: &str, diag: &SourceDiag);
```

---

## Cooklang-find Library

**Repository**: https://github.com/cooklang/cooklang-find
**Version**: 0.5.0
**License**: MIT

### Purpose

Library for discovering and organizing Cooklang recipes in the filesystem.

### Core API

```rust
use cooklang_find::{get_recipe, build_tree, search};
use std::path::Path;

// Find a specific recipe across multiple directories
// First matching directory wins
let recipe = get_recipe(
    vec![Path::new("/recipes"), Path::new("/backup")],
    Path::new("pasta/carbonara.cook")
)?;

// Build hierarchical tree of recipes
let tree = build_tree(Path::new("/recipes"))?;

// Case-insensitive content search
let results = search(Path::new("/recipes"), "garlic")?;
```

### Error Types

```rust
enum FetchError { /* Recipe location issues */ }
enum RecipeEntryError { /* Parsing problems */ }
enum TreeError { /* Hierarchical structure issues */ }
enum SearchError { /* Discovery failures */ }
```

### Features

- YAML frontmatter metadata parsing
- Image association (jpg, jpeg, png, webp)
- Multiple search directories with priority
- Nested directory organization

### Dependencies

```toml
[dependencies]
camino = { version = "1.1", features = ["serde1"] }
glob = "0.3"
serde = { version = "1.0", features = ["derive"] }
serde_yaml = "0.9"
thiserror = "2"
```

---

## Tower-LSP Framework

**Repository**: https://github.com/ebkalderon/tower-lsp
**Version**: 0.20.0
**License**: MIT / Apache-2.0

### Overview

Tower-based framework for building Language Server Protocol servers in Rust.

### Core Components

```rust
use tower_lsp::{LspService, Server, Client, LanguageServer};
use tower_lsp::lsp_types::*;

// Backend struct holds server state
#[derive(Debug)]
struct Backend {
    client: Client,
    // Custom state fields...
}

// Implement the LanguageServer trait
#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult>;
    async fn initialized(&self, _: InitializedParams);
    async fn shutdown(&self) -> Result<()>;

    // Document sync
    async fn did_open(&self, params: DidOpenTextDocumentParams);
    async fn did_change(&self, params: DidChangeTextDocumentParams);
    async fn did_save(&self, params: DidSaveTextDocumentParams);
    async fn did_close(&self, params: DidCloseTextDocumentParams);

    // Language features
    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>>;
    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>>;
    async fn goto_definition(&self, params: GotoDefinitionParams) -> Result<Option<GotoDefinitionResponse>>;
    // ... more methods
}
```

### Server Initialization

```rust
#[tokio::main]
async fn main() {
    // Setup logging
    tracing_subscriber::fmt().init();

    // Create stdin/stdout streams
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    // Build the service
    let (service, socket) = LspService::new(|client| Backend { client });

    // Start the server
    Server::new(stdin, stdout, socket)
        .serve(service)
        .await;
}
```

### Declaring Capabilities

```rust
async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
    Ok(InitializeResult {
        capabilities: ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Kind(
                TextDocumentSyncKind::INCREMENTAL
            )),
            completion_provider: Some(CompletionOptions {
                trigger_characters: Some(vec!["@".into(), "#".into(), "~".into()]),
                ..Default::default()
            }),
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            definition_provider: Some(OneOf::Left(true)),
            document_symbol_provider: Some(OneOf::Left(true)),
            diagnostic_provider: Some(DiagnosticServerCapabilities::Options(
                DiagnosticOptions::default()
            )),
            ..Default::default()
        },
        ..Default::default()
    })
}
```

### Publishing Diagnostics

```rust
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};

async fn publish_diagnostics(&self, uri: Url, text: &str) {
    let diagnostics = self.validate(text);
    self.client
        .publish_diagnostics(uri, diagnostics, None)
        .await;
}

fn validate(&self, text: &str) -> Vec<Diagnostic> {
    let result = cooklang::parse(text);
    // Convert cooklang errors to LSP diagnostics
    result.warnings
        .iter()
        .map(|diag| convert_to_lsp_diagnostic(text, diag))
        .collect()
}
```

---

## LSP Capabilities to Implement

### Phase 1: Essential Features

| Feature | Description | Priority |
|---------|-------------|----------|
| **Diagnostics** | Report parsing errors and warnings | High |
| **Document Sync** | Track open/change/save/close | High |
| **Syntax Highlighting** | Semantic tokens for ingredients, cookware, timers | High |

### Phase 2: Code Intelligence

| Feature | Description | Priority |
|---------|-------------|----------|
| **Completion** | Suggest ingredients, cookware, units after `@`, `#`, `~` | Medium |
| **Hover** | Show ingredient details, quantities, conversions | Medium |
| **Document Symbols** | Outline sections, ingredients, cookware | Medium |

### Phase 3: Navigation & Refactoring

| Feature | Description | Priority |
|---------|-------------|----------|
| **Go to Definition** | Jump to ingredient first use | Low |
| **Find References** | Find all uses of an ingredient | Low |
| **Rename** | Rename ingredients/cookware across recipe | Low |
| **Code Actions** | Quick fixes for common issues | Low |

### Phase 4: Advanced Features

| Feature | Description | Priority |
|---------|-------------|----------|
| **Workspace Support** | Multi-recipe workspaces via cooklang-find | Low |
| **Code Lens** | Show scaling info, conversion hints | Low |
| **Formatting** | Auto-format recipes | Low |
| **Folding Ranges** | Collapse sections | Low |

---

## Architecture Design

### Project Structure

```
cooklang-language-server/
├── Cargo.toml
├── src/
│   ├── main.rs              # Entry point
│   ├── lib.rs               # Library exports
│   ├── backend.rs           # LSP Backend implementation
│   ├── document.rs          # Document management
│   ├── diagnostics.rs       # Error conversion
│   ├── completion.rs        # Completion provider
│   ├── hover.rs             # Hover provider
│   ├── symbols.rs           # Document symbols
│   ├── semantic_tokens.rs   # Syntax highlighting
│   └── utils/
│       ├── mod.rs
│       └── position.rs      # Span <-> Position conversion
└── tests/
    └── integration_tests.rs
```

### Core Components

#### 1. Document Store

```rust
use std::collections::HashMap;
use std::sync::RwLock;

struct DocumentStore {
    documents: RwLock<HashMap<Url, Document>>,
}

struct Document {
    uri: Url,
    version: i32,
    content: String,
    line_index: LineIndex,
    parsed: Option<ParsedRecipe>,
}

struct LineIndex {
    line_starts: Vec<usize>,  // Byte offsets of line starts
}

impl LineIndex {
    fn new(text: &str) -> Self;
    fn line_col(&self, offset: usize) -> (u32, u32);
    fn offset(&self, line: u32, col: u32) -> usize;
}
```

#### 2. Position Conversion

```rust
use cooklang::span::Span;
use tower_lsp::lsp_types::{Position, Range};

fn span_to_range(span: Span, line_index: &LineIndex) -> Range {
    let (start_line, start_col) = line_index.line_col(span.start);
    let (end_line, end_col) = line_index.line_col(span.end);
    Range {
        start: Position { line: start_line, character: start_col },
        end: Position { line: end_line, character: end_col },
    }
}

fn position_to_offset(pos: Position, line_index: &LineIndex) -> usize {
    line_index.offset(pos.line, pos.character)
}
```

#### 3. Diagnostic Conversion

```rust
use cooklang::error::{SourceDiag, Severity};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

fn convert_diagnostic(diag: &SourceDiag, line_index: &LineIndex) -> Diagnostic {
    let severity = match diag.severity {
        Severity::Error => DiagnosticSeverity::ERROR,
        Severity::Warning => DiagnosticSeverity::WARNING,
    };

    // Get primary span from labels
    let range = diag.labels
        .first()
        .map(|(span, _)| span_to_range(*span, line_index))
        .unwrap_or_default();

    Diagnostic {
        range,
        severity: Some(severity),
        source: Some("cooklang".into()),
        message: diag.message.clone(),
        ..Default::default()
    }
}
```

#### 4. Semantic Token Types

```rust
use tower_lsp::lsp_types::SemanticTokenType;

const TOKEN_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::VARIABLE,   // Ingredients
    SemanticTokenType::CLASS,      // Cookware
    SemanticTokenType::FUNCTION,   // Timers
    SemanticTokenType::NUMBER,     // Quantities
    SemanticTokenType::STRING,     // Units
    SemanticTokenType::COMMENT,    // Comments
    SemanticTokenType::KEYWORD,    // Metadata keys
    SemanticTokenType::NAMESPACE,  // Sections
];
```

---

## Implementation Plan

### Step 1: Project Setup

1. Create new Cargo project
2. Add dependencies (tower-lsp, cooklang, tokio, etc.)
3. Set up basic project structure
4. Implement minimal `LanguageServer` trait

### Step 2: Document Management

1. Implement `DocumentStore` for tracking open documents
2. Implement `LineIndex` for position conversion
3. Handle `did_open`, `did_change`, `did_close` events
4. Parse documents on change

### Step 3: Diagnostics

1. Parse recipe using `cooklang::parse()`
2. Convert `SourceDiag` to LSP `Diagnostic`
3. Publish diagnostics via `client.publish_diagnostics()`
4. Handle incremental updates

### Step 4: Semantic Tokens

1. Use `PullParser` for event-based tokenization
2. Map events to semantic token types
3. Calculate token positions and lengths
4. Return semantic tokens in LSP format

### Step 5: Completion

1. Detect trigger characters (`@`, `#`, `~`, `%`)
2. Provide context-aware completions:
   - After `@`: existing ingredients, common ingredients
   - After `#`: existing cookware, common tools
   - After `~`: time units
   - After `%`: common units

### Step 6: Hover

1. Find element at cursor position
2. For ingredients: show quantity, note, modifiers
3. For cookware: show usage count
4. For timers: show formatted duration

### Step 7: Document Symbols

1. Extract sections as symbol containers
2. List ingredients, cookware, timers as children
3. Provide outline view of recipe structure

### Step 8: Testing & Packaging

1. Write integration tests
2. Test with VS Code, Neovim, etc.
3. Create release binaries
4. Write editor extension (VS Code)

---

## References

- **Cooklang Specification**: https://cooklang.org/docs/spec/
- **cooklang-rs Repository**: https://github.com/cooklang/cooklang-rs
- **cooklang-rs Documentation**: https://docs.rs/cooklang/
- **cooklang-find Repository**: https://github.com/cooklang/cooklang-find
- **Tower-LSP Repository**: https://github.com/ebkalderon/tower-lsp
- **Tower-LSP Documentation**: https://docs.rs/tower-lsp/
- **LSP Specification**: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/
- **lsp-types Documentation**: https://docs.rs/lsp-types/
- **rust-analyzer (Reference)**: https://github.com/rust-lang/rust-analyzer
