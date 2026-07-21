use async_trait::async_trait;
use rustok_core::{
    module::HealthStatus, MigrationSource, ModuleEventListenerContext, ModuleEventListenerRegistry,
    ModuleKind, RusToKModule,
};
use sea_orm_migration::MigrationTrait;

pub mod analytics;
mod blog_projector;
pub mod diagnostics;
pub mod dictionaries;
pub mod engine;
#[cfg(feature = "graphql")]
pub mod graphql;
pub mod ingestion;
pub mod migrations;
pub mod models;
pub mod pg_engine;
pub mod ports;
pub mod presets;
pub mod projector;
pub mod ranking;
pub mod search_settings;
pub mod suggestions;

pub use analytics::{
    SearchAnalyticsInsightRow, SearchAnalyticsQueryRow, SearchAnalyticsService,
    SearchAnalyticsSnapshot, SearchAnalyticsSummary, SearchClickRecord, SearchQueryLogRecord,
    SLOW_QUERY_THRESHOLD_MS,
};
pub use diagnostics::{
    LaggingSearchDocument, SearchConsistencyIssue, SearchDiagnosticsService,
    SearchDiagnosticsSnapshot,
};
pub use dictionaries::{
    SearchDictionaryService, SearchDictionarySnapshot, SearchQueryRuleRecord, SearchQueryTransform,
    SearchStopWordRecord, SearchSynonymRecord,
};
pub use engine::{
    SearchAttributeFilter, SearchConnectorDescriptor, SearchEngine, SearchEngineKind, SearchQuery,
    canonical_search_result_url,
};
pub use engine::{SearchResult, SearchResultItem};
pub use ingestion::SearchIngestionHandler;
pub use models::SearchSettingsRecord;
pub use pg_engine::PgSearchEngine;
pub use ports::*;
pub use presets::{ResolvedSearchFilterPreset, SearchFilterPreset, SearchFilterPresetService};
pub use projector::SearchProjector;
pub use ranking::SearchRankingProfile;
pub use search_settings::SearchSettingsService;
pub use suggestions::{
    SearchSuggestion, SearchSuggestionKind, SearchSuggestionQuery, SearchSuggestionService,
};

/// Core search module that owns engine selection and connector-facing contracts.
pub struct SearchModule;

impl SearchModule {
    pub fn available_engines(&self) -> Vec<SearchConnectorDescriptor> {
        vec![SearchConnectorDescriptor::postgres_default()]
    }
}

impl MigrationSource for SearchModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}

#[async_trait]
impl RusToKModule for SearchModule {
    fn slug(&self) -> &'static str {
        "search"
    }

    fn name(&self) -> &'static str {
        "Search"
    }

    fn description(&self) -> &'static str {
        "Postgres-first search capability with settings-driven engine selection."
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Core
    }

    fn register_event_listeners(
        &self,
        registry: &mut ModuleEventListenerRegistry,
        ctx: &ModuleEventListenerContext<'_>,
    ) {
        registry.register(SearchIngestionHandler::new(ctx.db.clone()));
    }

    async fn health(&self) -> HealthStatus {
        // Module-level health has no host AppContext, so it cannot validate
        // search_documents, indexing lag, query plans or connector reachability.
        // The server readiness layer owns the concrete search backend/lag checks.
        HealthStatus::Degraded
    }
}

#[cfg(test)]
mod tests {
    use super::SearchModule;
    use rustok_core::module::{HealthStatus, RusToKModule};

    #[tokio::test]
    async fn search_module_health_defers_to_host_readiness() {
        assert_eq!(SearchModule.health().await, HealthStatus::Degraded);
    }
}
