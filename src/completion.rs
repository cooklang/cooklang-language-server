use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionList, CompletionParams, CompletionResponse,
    Documentation, InsertTextFormat,
};

use crate::document::Document;
use crate::state::ServerState;
use crate::utils::position::position_to_offset;

/// Common cooking units
const UNITS: &[(&str, &str)] = &[
    ("g", "grams"),
    ("kg", "kilograms"),
    ("mg", "milligrams"),
    ("ml", "milliliters"),
    ("l", "liters"),
    ("oz", "ounces"),
    ("lb", "pounds"),
    ("cup", "cups"),
    ("cups", "cups"),
    ("tbsp", "tablespoons"),
    ("tsp", "teaspoons"),
    ("pinch", "pinch"),
    ("clove", "cloves"),
    ("cloves", "cloves"),
    ("slice", "slices"),
    ("slices", "slices"),
    ("piece", "pieces"),
    ("pieces", "pieces"),
    ("bunch", "bunches"),
    ("sprig", "sprigs"),
    ("can", "cans"),
    ("jar", "jars"),
    ("packet", "packets"),
    ("head", "heads"),
    ("stalk", "stalks"),
];

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
    let chars: Vec<char> = text.chars().collect();
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
                                let after_percent: String = inside.split('%').last().unwrap_or("").to_string();
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

    // Add existing ingredients from current document
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

    // Add common ingredients
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
