use tower_lsp::lsp_types::{DocumentSymbol, DocumentSymbolResponse, Range, SymbolKind};

use crate::document::Document;

#[allow(deprecated)] // DocumentSymbol::deprecated is deprecated but required
pub fn get_document_symbols(doc: &Document) -> Option<DocumentSymbolResponse> {
    let parse_result = doc.parse_result.as_ref()?;
    let recipe = &parse_result.recipe;

    let mut symbols = Vec::new();

    // Add metadata as a symbol if present
    let metadata = &recipe.metadata;
    if !metadata.map.is_empty() {
        let meta_count = metadata.map.len();
        symbols.push(DocumentSymbol {
            name: "Metadata".into(),
            kind: SymbolKind::NAMESPACE,
            range: Range::default(),
            selection_range: Range::default(),
            detail: Some(format!("{} properties", meta_count)),
            children: None,
            tags: None,
            deprecated: None,
        });
    }

    // Add sections
    for section in &recipe.sections {
        let section_name = section
            .name
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Steps".into());

        let step_count = section
            .content
            .iter()
            .filter(|c| matches!(c, cooklang::model::Content::Step(_)))
            .count();

        symbols.push(DocumentSymbol {
            name: section_name,
            kind: SymbolKind::NAMESPACE,
            range: Range::default(),
            selection_range: Range::default(),
            detail: Some(format!("{} steps", step_count)),
            children: None,
            tags: None,
            deprecated: None,
        });
    }

    // Add ingredients section
    if !recipe.ingredients.is_empty() {
        let ingredient_symbols: Vec<_> = recipe
            .ingredients
            .iter()
            .map(|ing| {
                let detail = ing.quantity.as_ref().map(|q| q.to_string());
                DocumentSymbol {
                    name: ing.name.to_string(),
                    kind: SymbolKind::VARIABLE,
                    range: Range::default(),
                    selection_range: Range::default(),
                    detail,
                    children: None,
                    tags: None,
                    deprecated: None,
                }
            })
            .collect();

        symbols.push(DocumentSymbol {
            name: "Ingredients".into(),
            kind: SymbolKind::NAMESPACE,
            range: Range::default(),
            selection_range: Range::default(),
            detail: Some(format!("{} items", ingredient_symbols.len())),
            children: Some(ingredient_symbols),
            tags: None,
            deprecated: None,
        });
    }

    // Add cookware section
    if !recipe.cookware.is_empty() {
        let cookware_symbols: Vec<_> = recipe
            .cookware
            .iter()
            .map(|cw| DocumentSymbol {
                name: cw.name.to_string(),
                kind: SymbolKind::CLASS,
                range: Range::default(),
                selection_range: Range::default(),
                detail: cw.quantity.as_ref().map(|q| q.to_string()),
                children: None,
                tags: None,
                deprecated: None,
            })
            .collect();

        symbols.push(DocumentSymbol {
            name: "Cookware".into(),
            kind: SymbolKind::NAMESPACE,
            range: Range::default(),
            selection_range: Range::default(),
            detail: Some(format!("{} items", cookware_symbols.len())),
            children: Some(cookware_symbols),
            tags: None,
            deprecated: None,
        });
    }

    // Add timers section
    if !recipe.timers.is_empty() {
        let timer_symbols: Vec<_> = recipe
            .timers
            .iter()
            .map(|t| {
                let name = t
                    .name
                    .as_ref()
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "Timer".into());
                DocumentSymbol {
                    name,
                    kind: SymbolKind::FUNCTION,
                    range: Range::default(),
                    selection_range: Range::default(),
                    detail: t.quantity.as_ref().map(|q| q.to_string()),
                    children: None,
                    tags: None,
                    deprecated: None,
                }
            })
            .collect();

        symbols.push(DocumentSymbol {
            name: "Timers".into(),
            kind: SymbolKind::NAMESPACE,
            range: Range::default(),
            selection_range: Range::default(),
            detail: Some(format!("{} items", timer_symbols.len())),
            children: Some(timer_symbols),
            tags: None,
            deprecated: None,
        });
    }

    Some(DocumentSymbolResponse::Nested(symbols))
}
