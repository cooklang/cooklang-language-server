use std::sync::LazyLock;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionList, CompletionParams, CompletionResponse,
    Documentation, InsertTextFormat,
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
static UNITS: LazyLock<Vec<(&'static str, &'static str)>> = LazyLock::new(|| {
    parse_unit_pairs(include_str!("../data/units.txt"))
});

/// Common time units
const TIME_UNITS: &[(&str, &str)] = &[
    ("s", "seconds"),
    ("sec", "seconds"),
    ("secs", "seconds"),
    ("second", "seconds"),
    ("seconds", "seconds"),
    ("min", "minutes"),
    ("mins", "minutes"),
    ("minute", "minutes"),
    ("minutes", "minutes"),
    ("h", "hours"),
    ("hr", "hours"),
    ("hrs", "hours"),
    ("hour", "hours"),
    ("hours", "hours"),
];

/// Common cookware items
const COMMON_COOKWARE: &[&str] = &[
    "pot",
    "pan",
    "skillet",
    "saucepan",
    "wok",
    "dutch oven",
    "stockpot",
    "frying pan",
    "bowl",
    "mixing bowl",
    "large bowl",
    "small bowl",
    "cutting board",
    "knife",
    "chef's knife",
    "paring knife",
    "oven",
    "stove",
    "grill",
    "blender",
    "food processor",
    "mixer",
    "stand mixer",
    "whisk",
    "spatula",
    "wooden spoon",
    "ladle",
    "tongs",
    "colander",
    "strainer",
    "sieve",
    "baking sheet",
    "baking dish",
    "roasting pan",
    "casserole dish",
    "measuring cup",
    "measuring spoons",
    "rolling pin",
    "grater",
    "peeler",
    "can opener",
    "thermometer",
    "timer",
    "foil",
    "parchment paper",
    "plastic wrap",
];

/// Common ingredients for suggestions
const COMMON_INGREDIENTS: &[&str] = &[
    "salt",
    "pepper",
    "olive oil",
    "vegetable oil",
    "butter",
    "garlic",
    "onion",
    "water",
    "chicken broth",
    "beef broth",
    "flour",
    "sugar",
    "eggs",
    "milk",
    "cream",
    "cheese",
    "tomato",
    "lemon",
    "lime",
    "parsley",
    "cilantro",
    "basil",
    "oregano",
    "thyme",
    "rosemary",
    "cumin",
    "paprika",
    "cinnamon",
    "vanilla",
    "honey",
    "soy sauce",
    "vinegar",
    "wine",
];

pub fn get_completions(
    doc: &Document,
    params: &CompletionParams,
    state: &ServerState,
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
    };

    Some(CompletionResponse::List(CompletionList {
        is_incomplete: false,
        items,
    }))
}

#[derive(Debug)]
enum CompletionContext {
    Ingredient(String), // After @
    Cookware(String),   // After #
    Timer,              // After ~
    Unit(String),       // After % or in quantity
    Quantity,           // Inside {} after number
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
                    return Some(CompletionContext::Ingredient(
                        prefix.split('{').next().unwrap_or("").to_string(),
                    ));
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
                                let after_percent: String = inside.split('%').next_back().unwrap_or("").to_string();
                                return Some(CompletionContext::Unit(after_percent.trim().to_string()));
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
                    insert_text: Some(format!("{}{{}}", name)),
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
                if name.to_lowercase().starts_with(&prefix_lower)
                    && !items.iter().any(|i| &i.label == name)
                {
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

    // Add ingredients from aisle.conf (user's grocery list)
    for aisle_ingredient in state.get_aisle_ingredients() {
        if aisle_ingredient.name.to_lowercase().starts_with(&prefix_lower)
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
                ..Default::default()
            });
        }
    }

    // Add common ingredients (lowest priority fallback)
    for &ingredient in COMMON_INGREDIENTS {
        if ingredient.to_lowercase().starts_with(&prefix_lower)
            && !items.iter().any(|i| i.label == ingredient)
        {
            items.push(CompletionItem {
                label: ingredient.into(),
                kind: Some(CompletionItemKind::VARIABLE),
                detail: Some("Common ingredient".into()),
                ..Default::default()
            });
        }
    }

    items
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
                    ..Default::default()
                });
            }
        }
    }

    // Add common cookware
    for &cookware in COMMON_COOKWARE {
        if cookware.to_lowercase().starts_with(&prefix_lower)
            && !items.iter().any(|i| i.label == cookware)
        {
            items.push(CompletionItem {
                label: cookware.into(),
                kind: Some(CompletionItemKind::CLASS),
                detail: Some("Common cookware".into()),
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
