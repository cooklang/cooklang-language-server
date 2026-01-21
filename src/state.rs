use std::path::Path;
use std::sync::RwLock;

use dashmap::DashMap;
use tower_lsp::lsp_types::Url;

use crate::document::Document;

/// An ingredient from the aisle configuration with its category
#[derive(Debug, Clone)]
pub struct AisleIngredient {
    /// The ingredient name (or alias)
    pub name: String,
    /// The common/canonical name for this ingredient
    pub common_name: String,
    /// The category/aisle this ingredient belongs to
    pub category: String,
}

/// Owned version of parsed aisle configuration for storage
#[derive(Debug, Default)]
pub struct AisleConfig {
    /// All ingredients with their category info
    pub ingredients: Vec<AisleIngredient>,
}

impl AisleConfig {
    /// Parse an aisle.conf file content and create an owned AisleConfig
    /// Uses lenient parsing to skip errors and continue with valid entries
    pub fn parse(content: &str) -> Option<Self> {
        let result = cooklang::aisle::parse_lenient(content);
        let (aisle_conf, warnings) = result.into_result().ok()?;

        // Log any warnings from lenient parsing
        for warning in warnings.iter() {
            tracing::warn!("aisle.conf warning: {}", warning);
        }

        let mut ingredients = Vec::new();
        for category in &aisle_conf.categories {
            for ingredient in &category.ingredients {
                if let Some(common_name) = ingredient.names.first() {
                    for name in &ingredient.names {
                        ingredients.push(AisleIngredient {
                            name: name.to_string(),
                            common_name: common_name.to_string(),
                            category: category.name.to_string(),
                        });
                    }
                }
            }
        }
        Some(AisleConfig { ingredients })
    }

    /// Load aisle.conf from a workspace path
    pub fn load_from_workspace(workspace_path: &Path) -> Option<Self> {
        // Check for config/aisle.conf (standard cooklang location)
        let config_path = workspace_path.join("config").join("aisle.conf");
        if config_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                tracing::info!("Loading aisle.conf from {:?}", config_path);
                return Self::parse(&content);
            }
        }

        // Also check root aisle.conf
        let root_path = workspace_path.join("aisle.conf");
        if root_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&root_path) {
                tracing::info!("Loading aisle.conf from {:?}", root_path);
                return Self::parse(&content);
            }
        }

        None
    }
}

/// Thread-safe server state
pub struct ServerState {
    pub documents: DashMap<Url, Document>,
    /// Parsed aisle configuration for ingredient suggestions
    pub aisle_config: RwLock<Option<AisleConfig>>,
}

impl ServerState {
    pub fn new() -> Self {
        Self {
            documents: DashMap::new(),
            aisle_config: RwLock::new(None),
        }
    }

    /// Load aisle configuration from a workspace path
    pub fn load_aisle_config(&self, workspace_path: &Path) {
        if let Some(config) = AisleConfig::load_from_workspace(workspace_path) {
            let count = config.ingredients.len();
            if let Ok(mut guard) = self.aisle_config.write() {
                *guard = Some(config);
                tracing::info!("Loaded {} ingredients from aisle.conf", count);
            }
        }
    }

    /// Get a reference to the aisle config if loaded
    pub fn get_aisle_ingredients(&self) -> Vec<AisleIngredient> {
        if let Ok(guard) = self.aisle_config.read() {
            if let Some(ref config) = *guard {
                return config.ingredients.clone();
            }
        }
        Vec::new()
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

impl Default for ServerState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aisle_config_parse() {
        let content = r#"
[produce]
potatoes
carrots
onions|yellow onion|white onion

[dairy]
milk
butter
cheese|cheddar|parmesan
"#;
        let config = AisleConfig::parse(content).unwrap();
        assert!(!config.ingredients.is_empty());

        // Check potatoes
        let potatoes = config
            .ingredients
            .iter()
            .find(|i| i.name == "potatoes")
            .unwrap();
        assert_eq!(potatoes.category, "produce");
        assert_eq!(potatoes.common_name, "potatoes");

        // Check onion aliases
        let yellow_onion = config
            .ingredients
            .iter()
            .find(|i| i.name == "yellow onion")
            .unwrap();
        assert_eq!(yellow_onion.category, "produce");
        assert_eq!(yellow_onion.common_name, "onions");

        // Check cheese aliases
        let cheddar = config
            .ingredients
            .iter()
            .find(|i| i.name == "cheddar")
            .unwrap();
        assert_eq!(cheddar.category, "dairy");
        assert_eq!(cheddar.common_name, "cheese");
    }

    #[test]
    fn test_aisle_config_lenient_parsing() {
        // This content has errors: duplicate ingredient 'apple', orphan ingredient, duplicate category
        let content = r#"
orphan ingredient before any category
[produce]
apple
banana
apple

[dairy]
milk

[produce]
carrot
"#;
        // With lenient parsing, this should still succeed and return valid entries
        let config = AisleConfig::parse(content).unwrap();

        // Should have parsed the valid categories and ingredients
        assert!(!config.ingredients.is_empty());

        // Check that apple is present (first occurrence kept)
        let apple = config.ingredients.iter().find(|i| i.name == "apple");
        assert!(apple.is_some());
        assert_eq!(apple.unwrap().category, "produce");

        // Check that banana is present
        let banana = config.ingredients.iter().find(|i| i.name == "banana");
        assert!(banana.is_some());

        // Check that milk is present
        let milk = config.ingredients.iter().find(|i| i.name == "milk");
        assert!(milk.is_some());
        assert_eq!(milk.unwrap().category, "dairy");

        // Duplicate apple entries should be skipped (only one apple)
        let apple_count = config.ingredients.iter().filter(|i| i.name == "apple").count();
        assert_eq!(apple_count, 1);
    }
}
