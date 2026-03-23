use std::path::Path;
use std::sync::LazyLock;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionList, CompletionParams, CompletionResponse,
    CompletionTextEdit, Documentation, InsertTextFormat, Position, Range, TextEdit,
};

use crate::document::Document;
use crate::state::ServerState;
use crate::utils::position::position_to_offset;

/// Parse unit pairs from embedded data (format: "short = long")
fn parse_unit_pairs(data: &'static str) -> Vec<(&'static str, &'static str)> {
    data.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return None;
            }
            let mut parts = trimmed.split('=').map(|s| s.trim());
            match (parts.next(), parts.next()) {
                (Some(short), Some(long)) if parts.next().is_none() => Some((short, long)),
                _ => None,
            }
        })
        .collect()
}

/// Parse simple list from embedded data (one item per line)
fn parse_simple_list(data: &'static str) -> Vec<&'static str> {
    data.lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

/// Common cooking units (loaded from embedded data/units.txt)
static UNITS: LazyLock<Vec<(&'static str, &'static str)>> =
    LazyLock::new(|| parse_unit_pairs(include_str!("../data/units.txt")));

/// Common time units (loaded from embedded data/time_units.txt)
static TIME_UNITS: LazyLock<Vec<(&'static str, &'static str)>> =
    LazyLock::new(|| parse_unit_pairs(include_str!("../data/time_units.txt")));

/// Common cookware items (loaded from embedded data/cookware.txt)
static COMMON_COOKWARE: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| parse_simple_list(include_str!("../data/cookware.txt")));

/// Common ingredients for suggestions (loaded from embedded data/ingredients.txt)
static COMMON_INGREDIENTS: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| parse_simple_list(include_str!("../data/ingredients.txt")));

pub fn get_completions(
    doc: &Document,
    params: &CompletionParams,
    state: &ServerState,
    workspace_root: Option<&Path>,
) -> Option<CompletionResponse> {
    let offset = position_to_offset(params.text_document_position.position, &doc.line_index);
    let text_before = &doc.content[..offset.min(doc.content.len())];

    let context = find_completion_context(text_before)?;

    let items = match context {
        CompletionContext::Ingredient(prefix) => complete_ingredients(&prefix, doc, state),
        CompletionContext::Cookware(prefix) => complete_cookware(&prefix, doc),
        CompletionContext::Timer => complete_timer_units(),
        CompletionContext::Unit(prefix) => complete_units(&prefix),
        CompletionContext::Quantity => complete_quantity_snippets(),
        CompletionContext::RecipeReference(prefix) => {
            if let Some(root) = workspace_root {
                // Calculate the range from after '@' to cursor so the client
                // knows exactly what text to replace (. and / break word
                // boundaries, so without an explicit range the client can't
                // match or place completions correctly).
                let after_at_offset = offset - prefix.len();
                let (line, utf8_col) = doc.line_index.line_col(after_at_offset as u32);
                let utf16_col = doc.line_index.utf8_to_utf16_col(line, utf8_col);
                let replace_range = Range {
                    start: Position {
                        line,
                        character: utf16_col,
                    },
                    end: params.text_document_position.position,
                };
                complete_recipe_references(&prefix, root, replace_range)
            } else {
                vec![]
            }
        }
    };

    Some(CompletionResponse::List(CompletionList {
        is_incomplete: false,
        items,
    }))
}

#[derive(Debug)]
enum CompletionContext {
    Ingredient(String),      // After @
    Cookware(String),        // After #
    Timer,                   // After ~
    Unit(String),            // After % or in quantity
    Quantity,                // Inside {} after number
    RecipeReference(String), // After @. (file path reference)
}

fn find_completion_context(text: &str) -> Option<CompletionContext> {
    // Limit backward scan to last 200 characters for performance
    const MAX_SCAN: usize = 200;
    let byte_start = text.len().saturating_sub(MAX_SCAN);
    // Find valid UTF-8 char boundary at or after byte_start
    let scan_start = text.ceil_char_boundary(byte_start);
    let scan_text = &text[scan_start..];

    let chars: Vec<char> = scan_text.chars().collect();
    let len = chars.len();

    // Scan backwards to find context
    for i in (0..len).rev() {
        match chars[i] {
            '@' => {
                let prefix: String = chars[i + 1..].iter().collect();
                // Check we're not inside braces already
                if !prefix.contains('}') {
                    if prefix.contains('{') {
                        // Inside braces - could be quantity context
                        return Some(CompletionContext::Quantity);
                    }
                    let name_prefix = prefix.split('{').next().unwrap_or("").to_string();
                    // Check if this is a recipe/menu file reference (starts with ./ or ../)
                    if name_prefix.starts_with('.') {
                        return Some(CompletionContext::RecipeReference(name_prefix));
                    }
                    return Some(CompletionContext::Ingredient(name_prefix));
                }
                return None;
            }
            '#' => {
                let prefix: String = chars[i + 1..].iter().collect();
                if !prefix.contains('}') {
                    return Some(CompletionContext::Cookware(
                        prefix.split('{').next().unwrap_or("").to_string(),
                    ));
                }
                return None;
            }
            '~' => {
                let rest: String = chars[i + 1..].iter().collect();
                if !rest.contains('}') {
                    return Some(CompletionContext::Timer);
                }
                return None;
            }
            '%' => {
                let prefix: String = chars[i + 1..].iter().collect();
                if !prefix.contains('}') {
                    return Some(CompletionContext::Unit(prefix.trim().to_string()));
                }
                return None;
            }
            '{' => {
                // Check if we're in an ingredient/cookware/timer context
                for j in (0..i).rev() {
                    match chars[j] {
                        '@' | '#' | '~' => {
                            let inside: String = chars[i + 1..].iter().collect();
                            if inside.contains('%') {
                                let after_percent: String =
                                    inside.split('%').next_back().unwrap_or("").to_string();
                                return Some(CompletionContext::Unit(
                                    after_percent.trim().to_string(),
                                ));
                            }
                            return Some(CompletionContext::Quantity);
                        }
                        '\n' | '\r' => break,
                        _ => continue,
                    }
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
    let prefix_lower = prefix.to_lowercase();

    // Add existing ingredients from current document (highest priority)
    if let Some(ref result) = doc.parse_result {
        for ingredient in &result.recipe.ingredients {
            let name = &ingredient.name;
            if name.to_lowercase().starts_with(&prefix_lower) {
                items.push(CompletionItem {
                    label: name.clone(),
                    kind: Some(CompletionItemKind::VARIABLE),
                    detail: Some("Ingredient (from recipe)".into()),
                    insert_text: Some(format!("{}{{$0}}", name)),
                    insert_text_format: Some(InsertTextFormat::SNIPPET),
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
                if name.to_lowercase().starts_with(&prefix_lower)
                    && !items.iter().any(|i| &i.label == name)
                {
                    items.push(CompletionItem {
                        label: name.clone(),
                        kind: Some(CompletionItemKind::VARIABLE),
                        detail: Some("Ingredient (from workspace)".into()),
                        insert_text: Some(format!("{}{{$0}}", name)),
                        insert_text_format: Some(InsertTextFormat::SNIPPET),
                        ..Default::default()
                    });
                }
            }
        }
    }

    // Add ingredients from aisle.conf (user's grocery list)
    for aisle_ingredient in state.get_aisle_ingredients() {
        if aisle_ingredient
            .name
            .to_lowercase()
            .starts_with(&prefix_lower)
            && !items.iter().any(|i| i.label == aisle_ingredient.name)
        {
            // Show alias info if this is not the common name
            let detail = if aisle_ingredient.name != aisle_ingredient.common_name {
                format!(
                    "{} (alias for {})",
                    aisle_ingredient.category, aisle_ingredient.common_name
                )
            } else {
                aisle_ingredient.category.clone()
            };

            items.push(CompletionItem {
                label: aisle_ingredient.name.clone(),
                kind: Some(CompletionItemKind::VARIABLE),
                detail: Some(detail),
                documentation: Some(Documentation::String(format!(
                    "From aisle.conf - {}",
                    aisle_ingredient.category
                ))),
                insert_text: Some(format!("{}{{$0}}", aisle_ingredient.name)),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                ..Default::default()
            });
        }
    }

    // Add common ingredients (lowest priority fallback)
    for &ingredient in COMMON_INGREDIENTS.iter() {
        if ingredient.to_lowercase().starts_with(&prefix_lower)
            && !items.iter().any(|i| i.label == ingredient)
        {
            items.push(CompletionItem {
                label: ingredient.into(),
                kind: Some(CompletionItemKind::VARIABLE),
                detail: Some("Common ingredient".into()),
                insert_text: Some(format!("{}{{$0}}", ingredient)),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                ..Default::default()
            });
        }
    }

    items
}

/// Scan a directory recursively for .cook and .menu files
fn scan_recipe_files(root: &Path) -> Vec<(String, &'static str)> {
    let mut files = Vec::new();
    scan_dir_recursive(root, root, &mut files);
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

fn scan_dir_recursive(root: &Path, dir: &Path, files: &mut Vec<(String, &'static str)>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Skip hidden directories
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if !name.starts_with('.') {
                    scan_dir_recursive(root, &path, files);
                }
            }
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let kind = match ext {
                "cook" => "Recipe",
                "menu" => "Menu",
                _ => continue,
            };
            if let Ok(rel) = path.strip_prefix(root) {
                // Normalize path separators to forward slash and remove extension
                let rel_str = rel.to_string_lossy();
                let without_ext = rel_str
                    .strip_suffix(&format!(".{}", ext))
                    .unwrap_or(&rel_str);
                // Use forward slashes for consistency
                let normalized = without_ext.replace('\\', "/");
                files.push((format!("./{}", normalized), kind));
            }
        }
    }
}

/// Fuzzy match for file paths. Splits on `/` — non-final query segments must
/// prefix-match target segments in order; the final query segment is
/// subsequence-matched against the target filename. Both strings should
/// already be lowercased.
fn fuzzy_match(query: &str, target: &str) -> bool {
    // Just "." means match everything (current directory prefix)
    if query == "." || query == "./" {
        return true;
    }

    let query_segments: Vec<&str> = query.split('/').collect();
    let target_segments: Vec<&str> = target.split('/').collect();

    if query_segments.is_empty() || target_segments.is_empty() {
        return query_segments.is_empty();
    }

    let last_q = query_segments.len() - 1;
    let mut t_idx = 0;

    for (q_idx, q_seg) in query_segments.iter().enumerate() {
        if q_idx == last_q {
            // Last query segment: subsequence match against the filename
            let filename = target_segments.last().unwrap_or(&"");
            return subsequence_match(q_seg, filename);
        }
        // Non-final segments must prefix-match a target segment in order
        let mut found = false;
        while t_idx < target_segments.len() {
            if target_segments[t_idx].starts_with(q_seg) {
                t_idx += 1;
                found = true;
                break;
            }
            t_idx += 1;
        }
        if !found {
            return false;
        }
    }
    true
}

/// Simple subsequence check: all chars of `needle` appear in order in `haystack`.
fn subsequence_match(needle: &str, haystack: &str) -> bool {
    let mut chars = needle.chars().peekable();
    for c in haystack.chars() {
        if chars.peek() == Some(&c) {
            chars.next();
        }
    }
    chars.peek().is_none()
}

fn complete_recipe_references(
    prefix: &str,
    workspace_root: &Path,
    replace_range: Range,
) -> Vec<CompletionItem> {
    let files = scan_recipe_files(workspace_root);
    let prefix_lower = prefix.to_lowercase();

    files
        .into_iter()
        .filter(|(path, _)| fuzzy_match(&prefix_lower, &path.to_lowercase()))
        .map(|(path, kind)| {
            let display_name = path.rsplit('/').next().unwrap_or(&path);
            CompletionItem {
                label: path.clone(),
                kind: Some(CompletionItemKind::FILE),
                detail: Some(format!("{} reference", kind)),
                documentation: Some(Documentation::String(display_name.to_string())),
                filter_text: Some(path.clone()),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                    range: replace_range,
                    new_text: format!("{}{{$0}}", path),
                })),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                ..Default::default()
            }
        })
        .collect()
}

fn complete_cookware(prefix: &str, doc: &Document) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    let prefix_lower = prefix.to_lowercase();

    // Add existing cookware from document
    if let Some(ref result) = doc.parse_result {
        for cookware in &result.recipe.cookware {
            let name = &cookware.name;
            if name.to_lowercase().starts_with(&prefix_lower) {
                items.push(CompletionItem {
                    label: name.clone(),
                    kind: Some(CompletionItemKind::CLASS),
                    detail: Some("Cookware (from recipe)".into()),
                    insert_text: Some(format!("{}{{$0}}", name)),
                    insert_text_format: Some(InsertTextFormat::SNIPPET),
                    ..Default::default()
                });
            }
        }
    }

    // Add common cookware
    for &cookware in COMMON_COOKWARE.iter() {
        if cookware.to_lowercase().starts_with(&prefix_lower)
            && !items.iter().any(|i| i.label == cookware)
        {
            items.push(CompletionItem {
                label: cookware.into(),
                kind: Some(CompletionItemKind::CLASS),
                detail: Some("Common cookware".into()),
                insert_text: Some(format!("{}{{$0}}", cookware)),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                ..Default::default()
            });
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
    let prefix_lower = prefix.to_lowercase();

    let mut items: Vec<_> = UNITS
        .iter()
        .filter(|(short, _)| short.to_lowercase().starts_with(&prefix_lower))
        .map(|(short, long)| CompletionItem {
            label: short.to_string(),
            kind: Some(CompletionItemKind::UNIT),
            detail: Some(long.to_string()),
            ..Default::default()
        })
        .collect();

    // Also add time units when completing units
    items.extend(
        TIME_UNITS
            .iter()
            .filter(|(short, _)| short.to_lowercase().starts_with(&prefix_lower))
            .map(|(short, long)| CompletionItem {
                label: short.to_string(),
                kind: Some(CompletionItemKind::UNIT),
                detail: Some(format!("{} (time)", long)),
                ..Default::default()
            }),
    );

    items
}

fn complete_quantity_snippets() -> Vec<CompletionItem> {
    vec![
        CompletionItem {
            label: "quantity with unit".into(),
            kind: Some(CompletionItemKind::SNIPPET),
            insert_text: Some("${1:amount}%${2:unit}".into()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            detail: Some("Insert quantity with unit".into()),
            ..Default::default()
        },
        CompletionItem {
            label: "quantity only".into(),
            kind: Some(CompletionItemKind::SNIPPET),
            insert_text: Some("${1:amount}".into()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            detail: Some("Insert quantity without unit".into()),
            ..Default::default()
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_context_recipe_reference_dot() {
        let ctx = find_completion_context("Pour over with @.").unwrap();
        assert!(matches!(ctx, CompletionContext::RecipeReference(ref p) if p == "."));
    }

    #[test]
    fn test_context_recipe_reference_dot_slash() {
        let ctx = find_completion_context("Pour over with @./").unwrap();
        assert!(matches!(ctx, CompletionContext::RecipeReference(ref p) if p == "./"));
    }

    #[test]
    fn test_context_recipe_reference_path() {
        let ctx = find_completion_context("Pour over with @./sauces/Hol").unwrap();
        assert!(matches!(ctx, CompletionContext::RecipeReference(ref p) if p == "./sauces/Hol"));
    }

    #[test]
    fn test_context_recipe_reference_parent() {
        let ctx = find_completion_context("@../other/Recipe").unwrap();
        assert!(matches!(ctx, CompletionContext::RecipeReference(ref p) if p == "../other/Recipe"));
    }

    #[test]
    fn test_context_recipe_reference_with_brace_is_quantity() {
        let ctx = find_completion_context("@./sauces/Hollandaise{").unwrap();
        assert!(matches!(ctx, CompletionContext::Quantity));
    }

    #[test]
    fn test_context_regular_ingredient_unchanged() {
        let ctx = find_completion_context("@sal").unwrap();
        assert!(matches!(ctx, CompletionContext::Ingredient(ref p) if p == "sal"));
    }

    #[test]
    fn test_scan_recipe_files() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        // Create directory structure
        fs::create_dir_all(root.join("sauces")).unwrap();
        fs::create_dir_all(root.join(".hidden")).unwrap();

        // Create recipe files
        fs::write(root.join("Pancakes.cook"), "").unwrap();
        fs::write(root.join("sauces/Hollandaise.cook"), "").unwrap();
        fs::write(root.join("sauces/Bechamel.cook"), "").unwrap();
        fs::write(root.join("WeeklyMenu.menu"), "").unwrap();
        fs::write(root.join("notes.txt"), "").unwrap();
        fs::write(root.join(".hidden/Secret.cook"), "").unwrap();

        let files = scan_recipe_files(root);
        let paths: Vec<&str> = files.iter().map(|(p, _)| p.as_str()).collect();

        assert!(paths.contains(&"./Pancakes"));
        assert!(paths.contains(&"./sauces/Hollandaise"));
        assert!(paths.contains(&"./sauces/Bechamel"));
        assert!(paths.contains(&"./WeeklyMenu"));

        // Should not include non-recipe files
        assert!(!paths.iter().any(|p| p.contains("notes")));
        // Should not include hidden directory files
        assert!(!paths.iter().any(|p| p.contains("Secret")));

        // Check kinds
        let menu = files.iter().find(|(p, _)| p == "./WeeklyMenu").unwrap();
        assert_eq!(menu.1, "Menu");

        let recipe = files.iter().find(|(p, _)| p == "./Pancakes").unwrap();
        assert_eq!(recipe.1, "Recipe");
    }

    #[test]
    fn test_complete_recipe_references_filtering() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        fs::create_dir_all(root.join("sauces")).unwrap();
        fs::write(root.join("sauces/Hollandaise.cook"), "").unwrap();
        fs::write(root.join("sauces/Bechamel.cook"), "").unwrap();
        fs::write(root.join("Pancakes.cook"), "").unwrap();

        // Dummy range for tests
        let range = Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 0,
            },
        };

        // Filter by directory + partial filename (fuzzy on filename)
        let items = complete_recipe_references("./sauces/Hol", root, range);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "./sauces/Hollandaise");
        // text_edit should contain the snippet
        match &items[0].text_edit {
            Some(CompletionTextEdit::Edit(edit)) => {
                assert_eq!(edit.new_text, "./sauces/Hollandaise{$0}");
            }
            _ => panic!("Expected text_edit"),
        }

        // All sauces
        let items = complete_recipe_references("./sauces/", root, range);
        assert_eq!(items.len(), 2);

        // Everything
        let items = complete_recipe_references("./", root, range);
        assert_eq!(items.len(), 3);

        // Just dot
        let items = complete_recipe_references(".", root, range);
        assert_eq!(items.len(), 3);

        // Fuzzy match across path segments
        let items = complete_recipe_references("./hol", root, range);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "./sauces/Hollandaise");

        // Fuzzy match - short query
        let items = complete_recipe_references("./pan", root, range);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "./Pancakes");
    }

    #[test]
    fn test_fuzzy_match() {
        // Fuzzy on filename (last segment of target)
        assert!(fuzzy_match("./hol", "./sauces/hollandaise"));
        assert!(fuzzy_match("./pan", "./pancakes"));
        assert!(fuzzy_match("./", "./anything"));
        assert!(fuzzy_match(".", "./anything"));
        // No match
        assert!(!fuzzy_match("./xyz", "./pancakes"));
        // Directory prefix + fuzzy filename
        assert!(fuzzy_match("./sauces/hol", "./sauces/hollandaise"));
        assert!(fuzzy_match("./sauces/b", "./sauces/bechamel"));
        // Wrong directory excludes results
        assert!(!fuzzy_match("./sauces/p", "./pancakes"));
        // Subsequence within filename
        assert!(fuzzy_match("./bml", "./sauces/bechamel"));
        assert!(!fuzzy_match("./zz", "./sauces/bechamel"));
    }
}
