//! # RusToK Search Module
//!
//! Postgres-first search capability with settings-driven engine selection and connector registry.
//!
//! Provides projection ingestion, faceted search, analytics, dictionary/synonym management,
//! search presets, suggestions, and transport-neutral search query ports.

pub mod analytics;
pub mod audience_security;
pub mod diagnostics;
pub mod dictionaries;
pub mod engine;
mod forum_contract_ingress;
mod forum_inbox;
mod forum_owner_checkpoint;
mod forum_reconciliation;
#[cfg(feature = "graphql")]
pub mod graphql;
pub mod ingestion;
pub mod migrations;
pub mod models;
pub mod module;
pub mod pg_engine;
pub mod ports;
pub mod presets;
mod product_channel_reconciliation;
mod projection_driver;
pub mod projection_source;
pub mod projector;
mod projector_core;
pub mod ranking;
pub mod search_settings;
pub mod storefront_category_scope;
pub mod storefront_channel_authority;
mod storefront_product_channel_visibility;
pub mod suggestions;

#[cfg(test)]
mod contract_tests;

pub use module::SearchModule;

pub use analytics::{
    SLOW_QUERY_THRESHOLD_MS, SearchAnalyticsInsightRow, SearchAnalyticsQueryRow,
    SearchAnalyticsService, SearchAnalyticsSnapshot, SearchAnalyticsSummary, SearchClickRecord,
    SearchQueryLogRecord,
};
pub use audience_security::{
    AUDIENCE_CANDIDATE_SCAN_PAGE_SIZE, MAX_SEARCH_AUDIENCE_CANDIDATES,
    MAX_SEARCH_AUDIENCE_SCAN_DOCUMENTS, SearchAudienceClass, SearchAudiencePolicyPort,
    SearchAudienceRegistry, SearchAudienceRequest, SearchCandidateDescriptor,
    SharedSearchAudiencePolicyPort, StorefrontSearchAudienceContext, empty_storefront_result,
    execute_audience_eligible_search, resolve_audience_eligible_candidates,
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
pub use forum_contract_ingress::{
    FORUM_SEARCH_CONTRACT_CONSUMER_GROUP, FORUM_SEARCH_CONTRACT_EVENT_TYPE,
    FORUM_SEARCH_CONTRACT_TOPIC, ForumSearchContractIngress, ForumSearchContractIngressError,
    ForumSearchContractIngressOutcome,
};
pub use forum_owner_checkpoint::{
    ForumProjectionOwnerTenantHead, ForumProjectionOwnerTenantPageRequest,
    MAX_FORUM_OWNER_TENANT_PAGE_LIMIT, resolve_forum_projection_owner_tenant_heads,
};
pub use forum_reconciliation::{
    DEFAULT_FORUM_OWNER_REVISION_PAGE_LIMIT, DEFAULT_FORUM_SWEEP_EVENT_LIMIT,
    DEFAULT_FORUM_SWEEP_TENANT_LIMIT, ForumProjectionOwnerRevisionImpact,
    ForumProjectionOwnerRevisionRecord, ForumProjectionOwnerRevisionRequest,
    ForumProjectionOwnerRevisionSourcePort, ForumProjectionReconciler, ForumProjectionSweepReport,
    MAX_FORUM_OWNER_REVISION_PAGE_LIMIT, SharedForumProjectionOwnerRevisionSourcePort,
    resolve_forum_projection_owner_revisions,
};
pub use ingestion::SearchIngestionHandler;
pub use models::SearchSettingsRecord;
pub use pg_engine::{PgSearchEngine, SearchAudienceDocumentScope};
pub use ports::{SearchQueryPort, SearchSuggestionPort};
pub use presets::{ResolvedSearchFilterPreset, SearchFilterPreset, SearchFilterPresetService};
pub use product_channel_reconciliation::{
    DEFAULT_PRODUCT_CHANNEL_REPAIR_TENANT_LIMIT, ProductChannelProjectionReconciler,
    ProductChannelProjectionSweepReport,
};
pub use projection_source::{
    MAX_SEARCH_PROJECTION_PAGE_SIZE, SearchProjectionDocument, SearchProjectionPage,
    SearchProjectionSource, SearchProjectionSourceFactory, SearchProjectionSourceRegistry,
    register_search_projection_source, search_projection_source_registry_from_extensions,
};
pub use projector::SearchProjector;
pub use ranking::SearchRankingProfile;
pub use search_settings::SearchSettingsService;
pub use storefront_category_scope::{
    SharedStorefrontSearchCategoryScopePort, StorefrontSearchCategoryScopePort,
    StorefrontSearchCategoryScopeRequest, StorefrontSearchTransport,
    resolve_storefront_search_category_ids, scope_is_exactly,
};
pub use storefront_channel_authority::{
    StorefrontChannelAuthorityError, TrustedStorefrontChannel, resolve_trusted_storefront_channel,
    resolve_trusted_storefront_channel_input,
};
pub use suggestions::{
    SearchSuggestion, SearchSuggestionKind, SearchSuggestionQuery, SearchSuggestionService,
    StorefrontSuggestionAudience,
};
