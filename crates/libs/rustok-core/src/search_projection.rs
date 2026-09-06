use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{Error, ModuleRuntimeExtensions, Result};

pub const MAX_SEARCH_PROJECTION_PAGE_SIZE: usize = 100;

/// One already-authorized source document ready for Search-owned persistence.
///
/// Source modules own discovery and visibility. Search owns storage, indexing,
/// ranking and retrieval, so a source must never return a document that the
/// public storefront is not currently allowed to discover.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchProjectionDocument {
    pub document_key: String,
    pub tenant_id: Uuid,
    pub document_id: Uuid,
    pub source_module: String,
    pub entity_type: String,
    pub locale: String,
    pub status: String,
    pub is_public: bool,
    pub title: String,
    pub subtitle: Option<String>,
    pub slug: Option<String>,
    pub handle: Option<String>,
    pub body: String,
    pub keywords_text: String,
    pub facets: Value,
    pub payload: Value,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SearchProjectionPage {
    pub documents: Vec<SearchProjectionDocument>,
    pub next_cursor: Option<String>,
}

/// Runtime source boundary used by the Search ingestion owner.
///
/// Cursors advance over bounded raw owner candidates, not only visible output,
/// so a page may be sparse or empty while still carrying `next_cursor`.
#[async_trait]
pub trait SearchProjectionSource: Send + Sync {
    fn source_module(&self) -> &'static str;

    async fn list_public_documents(
        &self,
        tenant_id: Uuid,
        after: Option<String>,
        limit: usize,
    ) -> Result<SearchProjectionPage>;

    async fn load_public_entity(
        &self,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
    ) -> Result<Vec<SearchProjectionDocument>>;
}

/// Module-published factory. Registration needs no database handle; Search
/// materializes the source later from the event-listener runtime database.
pub trait SearchProjectionSourceFactory: Send + Sync {
    fn source_module(&self) -> &'static str;

    fn build(&self, db: DatabaseConnection) -> Arc<dyn SearchProjectionSource>;
}

#[derive(Clone, Default)]
pub struct SearchProjectionSourceRegistry {
    factories: BTreeMap<String, Arc<dyn SearchProjectionSourceFactory>>,
}

impl SearchProjectionSourceRegistry {
    pub fn register<F>(&mut self, factory: F) -> Result<()>
    where
        F: SearchProjectionSourceFactory + 'static,
    {
        let source_module = normalize_source_module(factory.source_module())?;
        if self.factories.contains_key(&source_module) {
            return Err(Error::Validation(format!(
                "Search projection source `{source_module}` is already registered"
            )));
        }
        self.factories.insert(source_module, Arc::new(factory));
        Ok(())
    }

    pub fn build(
        &self,
        source_module: &str,
        db: DatabaseConnection,
    ) -> Option<Arc<dyn SearchProjectionSource>> {
        let source_module = normalize_source_module(source_module).ok()?;
        self.factories
            .get(&source_module)
            .map(|factory| factory.build(db))
    }

    pub fn source_modules(&self) -> Vec<String> {
        self.factories.keys().cloned().collect()
    }
}

pub fn register_search_projection_source<F>(
    extensions: &mut ModuleRuntimeExtensions,
    factory: F,
) -> Result<()>
where
    F: SearchProjectionSourceFactory + 'static,
{
    let registry = extensions.get_or_insert_with::<Arc<SearchProjectionSourceRegistry>, _>(|| {
        Arc::new(SearchProjectionSourceRegistry::default())
    });
    Arc::make_mut(registry).register(factory)
}

pub fn search_projection_source_registry_from_extensions(
    extensions: &ModuleRuntimeExtensions,
) -> Option<Arc<SearchProjectionSourceRegistry>> {
    extensions
        .get::<Arc<SearchProjectionSourceRegistry>>()
        .cloned()
}

fn normalize_source_module(value: &str) -> Result<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty()
        || normalized.len() > 64
        || !normalized.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '-' | '_')
        })
    {
        return Err(Error::Validation(format!(
            "Invalid Search projection source module `{value}`"
        )));
    }
    Ok(normalized)
}
