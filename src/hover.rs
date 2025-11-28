use tower_lsp::lsp_types::{Hover, HoverContents, HoverParams, MarkupContent, MarkupKind};

use crate::document::Document;
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
                let timer_name = timer.name.as_ref().map(|n| n.as_str()).unwrap_or("");
                if timer_name.eq_ignore_ascii_case(&name) || name.is_empty() {
                    return Some(create_hover(format_timer_hover(timer)));
                }
            }
            format!("**Timer:** {}", if name.is_empty() { "unnamed" } else { &name })
        }
        ElementType::Section => {
            format!("**Section:** {}", element_text.trim_matches('=').trim())
        }
        ElementType::Metadata => {
            format!("**Metadata:** {}", element_text.trim_start_matches('>').trim())
        }
        ElementType::Comment => {
            "**Comment**".to_string()
        }
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
    let bytes = content.as_bytes();
    let len = bytes.len();

    if offset >= len {
        return None;
    }

    // Look backwards to find the start of an element
    let mut start = offset;
    while start > 0 {
        let ch = bytes[start - 1];
        match ch {
            b'@' => {
                // Found ingredient start
                let end = find_element_end(content, start);
                return Some((
                    ElementType::Ingredient,
                    content[start - 1..end].to_string(),
                ));
            }
            b'#' => {
                let end = find_element_end(content, start);
                return Some((ElementType::Cookware, content[start - 1..end].to_string()));
            }
            b'~' => {
                let end = find_element_end(content, start);
                return Some((ElementType::Timer, content[start - 1..end].to_string()));
            }
            b'\n' | b'\r' => break,
            _ => start -= 1,
        }
    }

    // Check if we're on a special line
    let line_start = content[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_end = content[offset..]
        .find('\n')
        .map(|i| offset + i)
        .unwrap_or(len);
    let line = &content[line_start..line_end];

    if line.starts_with("--") {
        return Some((ElementType::Comment, line.to_string()));
    }
    if line.starts_with(">>") {
        return Some((ElementType::Metadata, line.to_string()));
    }
    if line.starts_with('=') && line.ends_with('=') {
        return Some((ElementType::Section, line.to_string()));
    }

    // Check if cursor is inside an element
    let before = &content[line_start..offset];
    if let Some(at_pos) = before.rfind('@') {
        if !before[at_pos..].contains('}') {
            let end = find_element_end(content, line_start + at_pos + 1);
            return Some((
                ElementType::Ingredient,
                content[line_start + at_pos..end].to_string(),
            ));
        }
    }
    if let Some(hash_pos) = before.rfind('#') {
        if !before[hash_pos..].contains('}') {
            let end = find_element_end(content, line_start + hash_pos + 1);
            return Some((
                ElementType::Cookware,
                content[line_start + hash_pos..end].to_string(),
            ));
        }
    }
    if let Some(tilde_pos) = before.rfind('~') {
        if !before[tilde_pos..].contains('}') {
            let end = find_element_end(content, line_start + tilde_pos + 1);
            return Some((
                ElementType::Timer,
                content[line_start + tilde_pos..end].to_string(),
            ));
        }
    }

    None
}

fn find_element_end(content: &str, start: usize) -> usize {
    let bytes = content.as_bytes();
    let mut pos = start;
    let mut in_braces = false;

    while pos < bytes.len() {
        match bytes[pos] {
            b'{' => in_braces = true,
            b'}' => return pos + 1,
            b' ' | b'\n' | b'\r' if !in_braces => return pos,
            _ => {}
        }
        pos += 1;
    }
    pos
}

fn extract_name(element: &str) -> String {
    // Remove @ # ~ prefix and extract name before {
    let s = element.trim_start_matches(|c| c == '@' || c == '#' || c == '~');
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
