use tower_lsp::lsp_types::{Hover, HoverContents, HoverParams, MarkupContent, MarkupKind};

use crate::document::Document;
use crate::utils::component::component_end;
use crate::utils::position::position_to_offset;

pub fn get_hover(doc: &Document, params: &HoverParams) -> Option<Hover> {
    let offset = position_to_offset(
        params.text_document_position_params.position,
        &doc.line_index,
    );

    let content = &doc.content;
    let parse_result = doc.parse_result.as_ref()?;

    // Find what element is at the cursor position
    // Look backwards and forwards to find the element boundaries
    let (element_type, element_text) = find_element_at_offset(content, offset)?;

    let hover_text = match element_type {
        ElementType::Ingredient => {
            // Find the ingredient in the parsed recipe
            let name = extract_name(&element_text);
            for ingredient in &parse_result.recipe.ingredients {
                if ingredient.name.eq_ignore_ascii_case(&name) {
                    return Some(create_hover(format_ingredient_hover(ingredient)));
                }
            }
            format!("**Ingredient:** {}", name)
        }
        ElementType::Cookware => {
            let name = extract_name(&element_text);
            for cookware in &parse_result.recipe.cookware {
                if cookware.name.eq_ignore_ascii_case(&name) {
                    return Some(create_hover(format_cookware_hover(cookware)));
                }
            }
            format!("**Cookware:** {}", name)
        }
        ElementType::Timer => {
            // Find matching timer by name or duration
            let name = extract_name(&element_text);
            for timer in &parse_result.recipe.timers {
                let timer_name = timer.name.as_deref().unwrap_or("");
                if timer_name.eq_ignore_ascii_case(&name) || name.is_empty() {
                    return Some(create_hover(format_timer_hover(timer)));
                }
            }
            format!(
                "**Timer:** {}",
                if name.is_empty() { "unnamed" } else { &name }
            )
        }
        ElementType::Section => {
            format!("**Section:** {}", element_text.trim_matches('=').trim())
        }
        ElementType::Metadata => {
            format!(
                "**Metadata:** {}",
                element_text.trim_start_matches('>').trim()
            )
        }
        ElementType::Comment => "**Comment**".to_string(),
    };

    Some(create_hover(hover_text))
}

#[derive(Debug)]
enum ElementType {
    Ingredient,
    Cookware,
    Timer,
    Section,
    Metadata,
    Comment,
}

fn find_element_at_offset(content: &str, offset: usize) -> Option<(ElementType, String)> {
    let len = content.len();

    if offset >= len {
        return None;
    }

    // Find line boundaries (these operations are UTF-8 safe)
    let line_start = content[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_end = content[offset..]
        .find('\n')
        .map(|i| offset + i)
        .unwrap_or(len);
    let line = &content[line_start..line_end];

    // Check if we're on a special line first
    if line.starts_with("--") {
        return Some((ElementType::Comment, line.to_string()));
    }
    if line.starts_with(">>") {
        return Some((ElementType::Metadata, line.to_string()));
    }
    if line.starts_with('=') && line.ends_with('=') && line.len() > 1 {
        return Some((ElementType::Section, line.to_string()));
    }

    // Get the text before cursor on this line
    let before_cursor = &content[line_start..offset];

    // Look for element markers (@, #, ~) scanning backwards
    // Use rfind which is UTF-8 safe and returns char boundaries
    let markers = [
        ('@', ElementType::Ingredient),
        ('#', ElementType::Cookware),
        ('~', ElementType::Timer),
    ];

    let mut best_match: Option<(usize, ElementType)> = None;

    for (marker, elem_type) in markers {
        if let Some(pos) = before_cursor.rfind(marker) {
            // Check we're not past a closing brace (element already complete)
            let after_marker = &before_cursor[pos..];
            if !after_marker.contains('}') {
                // This marker is still open, check if it's the closest one
                match best_match {
                    None => best_match = Some((line_start + pos, elem_type)),
                    Some((best_pos, _)) if line_start + pos > best_pos => {
                        best_match = Some((line_start + pos, elem_type));
                    }
                    _ => {}
                }
            }
        }
    }

    if let Some((marker_pos, elem_type)) = best_match {
        let end = component_end(content, marker_pos);
        return Some((elem_type, content[marker_pos..end].to_string()));
    }

    None
}

fn extract_name(element: &str) -> String {
    // Remove @ # ~ prefix and extract name before {
    let s = element.trim_start_matches(['@', '#', '~']);
    s.split('{').next().unwrap_or(s).trim().to_string()
}

fn create_hover(text: String) -> Hover {
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: text,
        }),
        range: None,
    }
}

fn format_ingredient_hover(ingredient: &cooklang::model::Ingredient) -> String {
    let mut parts = Vec::new();

    parts.push(format!("**Ingredient:** {}", ingredient.name));

    if let Some(ref quantity) = ingredient.quantity {
        parts.push(format!("**Quantity:** {}", quantity));
    }

    if let Some(ref note) = ingredient.note {
        parts.push(format!("**Note:** {}", note));
    }

    parts.join("\n\n")
}

fn format_cookware_hover(cookware: &cooklang::model::Cookware) -> String {
    let mut parts = Vec::new();

    parts.push(format!("**Cookware:** {}", cookware.name));

    if let Some(ref quantity) = cookware.quantity {
        parts.push(format!("**Quantity:** {}", quantity));
    }

    if let Some(ref note) = cookware.note {
        parts.push(format!("**Note:** {}", note));
    }

    parts.join("\n\n")
}

fn format_timer_hover(timer: &cooklang::model::Timer) -> String {
    let mut parts = Vec::new();

    if let Some(ref name) = timer.name {
        parts.push(format!("**Timer:** {}", name));
    } else {
        parts.push("**Timer**".to_string());
    }

    if let Some(ref quantity) = timer.quantity {
        parts.push(format!("**Duration:** {}", quantity));
    }

    parts.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;
    use tower_lsp::lsp_types::{
        Position, TextDocumentIdentifier, TextDocumentPositionParams, Url,
    };

    fn hover_at(content: &str, cursor: usize) -> String {
        let doc = Document::new(
            Url::parse("file:///test.cook").unwrap(),
            1,
            content.to_string(),
        );
        let line_index = &doc.line_index;
        let (line, character) = line_index.line_col(cursor as u32);
        let params = HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: doc.uri.clone(),
                },
                position: Position { line, character },
            },
            work_done_progress_params: Default::default(),
        };
        match get_hover(&doc, &params).unwrap().contents {
            HoverContents::Markup(m) => m.value,
            _ => panic!("expected markup hover"),
        }
    }

    #[test]
    fn hover_multi_word_ingredient_shows_full_name_and_quantity() {
        // Regression for cooklang/CookVSCode#10: hovering anywhere on a
        // multi-word ingredient must report the whole name and quantity.
        let content = "Chill @heavy whipping cream{1%cup}.";
        let cursor = content.find("whipping").unwrap(); // cursor on the 2nd word
        let hover = hover_at(content, cursor);
        assert!(
            hover.contains("**Ingredient:** heavy whipping cream"),
            "got: {hover}"
        );
        assert!(hover.contains("**Quantity:** 1 cup"), "got: {hover}");
    }

    #[test]
    fn hover_ingredient_with_apostrophe_and_space() {
        let content = "Add @lady's fingers{20}.";
        let cursor = content.find("fingers").unwrap();
        let hover = hover_at(content, cursor);
        assert!(
            hover.contains("**Ingredient:** lady's fingers"),
            "got: {hover}"
        );
    }
}
