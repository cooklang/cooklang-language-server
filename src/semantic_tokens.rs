use tower_lsp::lsp_types::{
    SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokensFullOptions,
    SemanticTokensLegend, SemanticTokensOptions, SemanticTokensServerCapabilities,
};

use crate::document::Document;

// Token type indices
const TOKEN_INGREDIENT: u32 = 0;
const TOKEN_COOKWARE: u32 = 1;
const TOKEN_TIMER: u32 = 2;
#[allow(dead_code)]
const TOKEN_QUANTITY: u32 = 3; // Reserved for future use
#[allow(dead_code)]
const TOKEN_UNIT: u32 = 4; // Reserved for future use
const TOKEN_COMMENT: u32 = 5;
const TOKEN_METADATA_KEY: u32 = 6;
#[allow(dead_code)]
const TOKEN_METADATA_VALUE: u32 = 7; // Reserved for future use
const TOKEN_SECTION: u32 = 8;

pub const TOKEN_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::VARIABLE,  // 0: Ingredients (@)
    SemanticTokenType::CLASS,     // 1: Cookware (#)
    SemanticTokenType::FUNCTION,  // 2: Timers (~)
    SemanticTokenType::NUMBER,    // 3: Quantities
    SemanticTokenType::STRING,    // 4: Units
    SemanticTokenType::COMMENT,   // 5: Comments
    SemanticTokenType::KEYWORD,   // 6: Metadata keys
    SemanticTokenType::PROPERTY,  // 7: Metadata values
    SemanticTokenType::NAMESPACE, // 8: Sections
];

pub const TOKEN_MODIFIERS: &[SemanticTokenModifier] = &[];

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

    fn push(&mut self, line: u32, start: u32, length: u32, token_type: u32) {
        if length == 0 {
            return;
        }

        let delta_line = line - self.prev_line;
        let delta_start = if delta_line == 0 {
            start.saturating_sub(self.prev_start)
        } else {
            start
        };

        self.tokens.push(SemanticToken {
            delta_line,
            delta_start,
            length,
            token_type,
            token_modifiers_bitset: 0,
        });

        self.prev_line = line;
        self.prev_start = start;
    }

    fn build(self) -> Vec<SemanticToken> {
        self.tokens
    }
}

pub fn get_semantic_tokens(doc: &Document) -> Vec<SemanticToken> {
    let mut builder = TokenBuilder::new();
    let content = &doc.content;
    let line_index = &doc.line_index;

    // Scan through the document and identify tokens
    let mut chars = content.char_indices().peekable();

    while let Some((idx, ch)) = chars.next() {
        match ch {
            // Ingredient: @name or @name{...}
            '@' => {
                let start = idx;
                let mut end = idx + 1;

                // Collect the ingredient name (no spaces allowed outside braces)
                while let Some(&(i, c)) = chars.peek() {
                    if c == '{' {
                        // Include until closing brace (spaces allowed inside)
                        chars.next();
                        end = i + 1;
                        while let Some(&(i2, c2)) = chars.peek() {
                            end = i2 + c2.len_utf8();
                            chars.next();
                            if c2 == '}' {
                                break;
                            }
                        }
                        break;
                    } else if c.is_alphanumeric() || c == '_' {
                        end = i + c.len_utf8();
                        chars.next();
                    } else {
                        break;
                    }
                }

                let (line, col) = line_index.line_col(start as u32);
                let length = line_index.utf16_len(start, end);
                builder.push(line, col, length, TOKEN_INGREDIENT);
            }

            // Cookware: #name or #name{}
            '#' => {
                let start = idx;
                let mut end = idx + 1;

                while let Some(&(i, c)) = chars.peek() {
                    if c == '{' {
                        chars.next();
                        end = i + 1;
                        while let Some(&(i2, c2)) = chars.peek() {
                            end = i2 + c2.len_utf8();
                            chars.next();
                            if c2 == '}' {
                                break;
                            }
                        }
                        break;
                    } else if c.is_alphanumeric() || c == '_' {
                        end = i + c.len_utf8();
                        chars.next();
                    } else {
                        break;
                    }
                }

                let (line, col) = line_index.line_col(start as u32);
                let length = line_index.utf16_len(start, end);
                builder.push(line, col, length, TOKEN_COOKWARE);
            }

            // Timer: ~name{...} or ~{...}
            '~' => {
                let start = idx;
                let mut end = idx + 1;

                while let Some(&(i, c)) = chars.peek() {
                    if c == '{' {
                        chars.next();
                        end = i + 1;
                        while let Some(&(i2, c2)) = chars.peek() {
                            end = i2 + c2.len_utf8();
                            chars.next();
                            if c2 == '}' {
                                break;
                            }
                        }
                        break;
                    } else if c.is_alphanumeric() || c == '_' {
                        end = i + c.len_utf8();
                        chars.next();
                    } else {
                        break;
                    }
                }

                let (line, col) = line_index.line_col(start as u32);
                let length = line_index.utf16_len(start, end);
                builder.push(line, col, length, TOKEN_TIMER);
            }

            // Line comment: -- ... OR YAML front matter: ---
            '-' => {
                let is_line_start =
                    idx == 0 || content.as_bytes().get(idx.saturating_sub(1)) == Some(&b'\n');

                if let Some(&(_, '-')) = chars.peek() {
                    let start = idx;
                    chars.next();

                    // Check for YAML front matter (--- at start of line)
                    if is_line_start {
                        if let Some(&(_, '-')) = chars.peek() {
                            chars.next();
                            // This is ---, check if it's only dashes until end of line
                            let mut is_yaml_delimiter = true;
                            let mut end = idx + 3;

                            while let Some(&(i, c)) = chars.peek() {
                                if c == '\n' {
                                    break;
                                }
                                if c != '-' && !c.is_whitespace() {
                                    is_yaml_delimiter = false;
                                }
                                end = i + c.len_utf8();
                                chars.next();
                            }

                            if is_yaml_delimiter {
                                // Highlight the --- line as metadata
                                let (line, col) = line_index.line_col(start as u32);
                                let length = line_index.utf16_len(start, end);
                                builder.push(line, col, length, TOKEN_METADATA_KEY);
                                continue;
                            }
                        }
                    }

                    // Regular comment: --
                    let mut end = idx + 2;
                    while let Some(&(i, c)) = chars.peek() {
                        if c == '\n' {
                            break;
                        }
                        end = i + c.len_utf8();
                        chars.next();
                    }

                    let (line, col) = line_index.line_col(start as u32);
                    let length = line_index.utf16_len(start, end);
                    builder.push(line, col, length, TOKEN_COMMENT);
                }
            }

            // Section: = Section Name = (must start at beginning of line)
            '=' => {
                // Check if this is at the start of a line
                let is_line_start =
                    idx == 0 || content.as_bytes().get(idx.saturating_sub(1)) == Some(&b'\n');

                if is_line_start {
                    let start = idx;
                    let mut end = idx + 1;
                    let mut found_closing = false;

                    while let Some(&(i, c)) = chars.peek() {
                        if c == '\n' {
                            break;
                        }
                        end = i + c.len_utf8();
                        chars.next();
                        if c == '=' {
                            found_closing = true;
                            break;
                        }
                    }

                    if found_closing {
                        let (line, col) = line_index.line_col(start as u32);
                        let length = line_index.utf16_len(start, end);
                        builder.push(line, col, length, TOKEN_SECTION);
                    }
                }
            }

            // Metadata: >> key: value
            '>' => {
                if let Some(&(_, '>')) = chars.peek() {
                    let start = idx;
                    chars.next();
                    let mut end = idx + 2;

                    // Read until end of line
                    while let Some(&(i, c)) = chars.peek() {
                        if c == '\n' {
                            break;
                        }
                        end = i + c.len_utf8();
                        chars.next();
                    }

                    let (line, col) = line_index.line_col(start as u32);
                    let length = line_index.utf16_len(start, end);
                    builder.push(line, col, length, TOKEN_METADATA_KEY);
                }
            }

            _ => {}
        }
    }

    builder.build()
}
