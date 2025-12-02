use dashmap::DashMap;
use tower_lsp::lsp_types::Url;

use crate::document::Document;

/// Thread-safe server state
pub struct ServerState {
    pub documents: DashMap<Url, Document>,
}

impl ServerState {
    pub fn new() -> Self {
        Self {
            documents: DashMap::new(),
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

impl Default for ServerState {
    fn default() -> Self {
        Self::new()
    }
}
