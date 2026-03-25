use async_trait::async_trait;
use rustok_core::{module::HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod analytics;
pub mod diagnostics;
pub mod dictionaries;
pub mod engine;
pub mod ingestion;
pub mod migrations;
pub mod models;
pub mod pg_engine;
pub mod projector;
pub mod search_settings;

pub use analytics::{
    SearchAnalyticsInsightRow, SearchAnalyticsQueryRow, SearchAnalyticsService,
    SearchAnalyticsSnapshot, SearchAnalyticsSummary, SearchClickRecord, SearchQueryLogRecord,
};
pub use diagnostics::{LaggingSearchDocument, SearchDiagnosticsService, SearchDiagnosticsSnapshot};
pub use dictionaries::{
    SearchDictionaryService, SearchDictionarySnapshot, SearchQueryRuleRecord, SearchQueryTransform,
    SearchStopWordRecord, SearchSynonymRecord,
};
pub use engine::{SearchConnectorDescriptor, SearchEngine, SearchEngineKind, SearchQuery};
pub use engine::{SearchResult, SearchResultItem};
pub use ingestion::SearchIngestionHandler;
pub use models::SearchSettingsRecord;
pub use pg_engine::PgSearchEngine;
pub use projector::SearchProjector;
pub use search_settings::SearchSettingsService;

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

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
}
