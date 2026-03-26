use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchEngineDescriptor {
    pub kind: String,
    pub label: String,
    #[serde(rename = "providedBy")]
    pub provided_by: String,
    pub enabled: bool,
    #[serde(rename = "defaultEngine")]
    pub default_engine: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchSettingsPayload {
    #[serde(rename = "tenantId")]
    pub tenant_id: Option<String>,
    #[serde(rename = "activeEngine")]
    pub active_engine: String,
    #[serde(rename = "fallbackEngine")]
    pub fallback_engine: String,
    pub config: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchFacetBucket {
    pub value: String,
    pub count: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchFacetGroup {
    pub name: String,
    pub buckets: Vec<SearchFacetBucket>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchPreviewResultItem {
    pub id: String,
    #[serde(rename = "entityType")]
    pub entity_type: String,
    #[serde(rename = "sourceModule")]
    pub source_module: String,
    pub title: String,
    pub snippet: Option<String>,
    pub score: f64,
    pub locale: Option<String>,
    pub url: Option<String>,
    pub payload: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchPreviewPayload {
    #[serde(rename = "queryLogId")]
    pub query_log_id: Option<String>,
    #[serde(rename = "presetKey")]
    pub preset_key: Option<String>,
    pub items: Vec<SearchPreviewResultItem>,
    pub total: u64,
    #[serde(rename = "tookMs")]
    pub took_ms: u64,
    pub engine: String,
    #[serde(rename = "rankingProfile")]
    pub ranking_profile: String,
    pub facets: Vec<SearchFacetGroup>,
}

#[derive(Clone, Debug)]
pub struct SearchPreviewFilters {
    pub entity_types: Vec<String>,
    pub source_modules: Vec<String>,
    pub statuses: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchFilterPresetPayload {
    pub key: String,
    pub label: String,
    #[serde(rename = "entityTypes")]
    pub entity_types: Vec<String>,
    #[serde(rename = "sourceModules")]
    pub source_modules: Vec<String>,
    pub statuses: Vec<String>,
    #[serde(rename = "rankingProfile")]
    pub ranking_profile: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchAdminBootstrap {
    #[serde(rename = "availableSearchEngines")]
    pub available_search_engines: Vec<SearchEngineDescriptor>,
    #[serde(rename = "searchSettingsPreview")]
    pub search_settings_preview: SearchSettingsPayload,
    #[serde(rename = "searchDiagnostics")]
    pub search_diagnostics: SearchDiagnosticsPayload,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchDiagnosticsPayload {
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    #[serde(rename = "totalDocuments")]
    pub total_documents: u64,
    #[serde(rename = "publicDocuments")]
    pub public_documents: u64,
    #[serde(rename = "contentDocuments")]
    pub content_documents: u64,
    #[serde(rename = "productDocuments")]
    pub product_documents: u64,
    #[serde(rename = "staleDocuments")]
    pub stale_documents: u64,
    #[serde(rename = "missingDocuments")]
    pub missing_documents: u64,
    #[serde(rename = "orphanedDocuments")]
    pub orphaned_documents: u64,
    #[serde(rename = "newestIndexedAt")]
    pub newest_indexed_at: Option<String>,
    #[serde(rename = "oldestIndexedAt")]
    pub oldest_indexed_at: Option<String>,
    #[serde(rename = "maxLagSeconds")]
    pub max_lag_seconds: u64,
    pub state: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchConsistencyIssuePayload {
    #[serde(rename = "issueKind")]
    pub issue_kind: String,
    #[serde(rename = "documentKey")]
    pub document_key: String,
    #[serde(rename = "documentId")]
    pub document_id: String,
    #[serde(rename = "sourceModule")]
    pub source_module: String,
    #[serde(rename = "entityType")]
    pub entity_type: String,
    pub locale: String,
    pub status: String,
    pub title: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "indexedAt")]
    pub indexed_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TriggerSearchRebuildPayload {
    pub success: bool,
    pub queued: bool,
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    #[serde(rename = "targetType")]
    pub target_type: String,
    #[serde(rename = "targetId")]
    pub target_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TrackSearchClickPayload {
    pub success: bool,
    pub tracked: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LaggingSearchDocumentPayload {
    #[serde(rename = "documentKey")]
    pub document_key: String,
    #[serde(rename = "documentId")]
    pub document_id: String,
    #[serde(rename = "sourceModule")]
    pub source_module: String,
    #[serde(rename = "entityType")]
    pub entity_type: String,
    pub locale: String,
    pub status: String,
    #[serde(rename = "isPublic")]
    pub is_public: bool,
    pub title: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "indexedAt")]
    pub indexed_at: String,
    #[serde(rename = "lagSeconds")]
    pub lag_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchAnalyticsSummaryPayload {
    #[serde(rename = "windowDays")]
    pub window_days: u32,
    #[serde(rename = "totalQueries")]
    pub total_queries: u64,
    #[serde(rename = "successfulQueries")]
    pub successful_queries: u64,
    #[serde(rename = "zeroResultQueries")]
    pub zero_result_queries: u64,
    #[serde(rename = "zeroResultRate")]
    pub zero_result_rate: f64,
    #[serde(rename = "slowQueries")]
    pub slow_queries: u64,
    #[serde(rename = "slowQueryRate")]
    pub slow_query_rate: f64,
    #[serde(rename = "avgTookMs")]
    pub avg_took_ms: f64,
    #[serde(rename = "avgResultsPerQuery")]
    pub avg_results_per_query: f64,
    #[serde(rename = "uniqueQueries")]
    pub unique_queries: u64,
    #[serde(rename = "clickedQueries")]
    pub clicked_queries: u64,
    #[serde(rename = "totalClicks")]
    pub total_clicks: u64,
    #[serde(rename = "clickThroughRate")]
    pub click_through_rate: f64,
    #[serde(rename = "abandonmentQueries")]
    pub abandonment_queries: u64,
    #[serde(rename = "abandonmentRate")]
    pub abandonment_rate: f64,
    #[serde(rename = "lastQueryAt")]
    pub last_query_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchAnalyticsQueryRowPayload {
    pub query: String,
    pub hits: u64,
    #[serde(rename = "zeroResultHits")]
    pub zero_result_hits: u64,
    pub clicks: u64,
    #[serde(rename = "avgTookMs")]
    pub avg_took_ms: f64,
    #[serde(rename = "avgResults")]
    pub avg_results: f64,
    #[serde(rename = "clickThroughRate")]
    pub click_through_rate: f64,
    #[serde(rename = "abandonmentRate")]
    pub abandonment_rate: f64,
    #[serde(rename = "lastSeenAt")]
    pub last_seen_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchAnalyticsInsightRowPayload {
    pub query: String,
    pub hits: u64,
    #[serde(rename = "zeroResultHits")]
    pub zero_result_hits: u64,
    pub clicks: u64,
    #[serde(rename = "clickThroughRate")]
    pub click_through_rate: f64,
    #[serde(rename = "abandonmentRate")]
    pub abandonment_rate: f64,
    pub recommendation: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchAnalyticsPayload {
    pub summary: SearchAnalyticsSummaryPayload,
    #[serde(rename = "topQueries")]
    pub top_queries: Vec<SearchAnalyticsQueryRowPayload>,
    #[serde(rename = "zeroResultQueries")]
    pub zero_result_queries: Vec<SearchAnalyticsQueryRowPayload>,
    #[serde(rename = "slowQueries")]
    pub slow_queries: Vec<SearchAnalyticsQueryRowPayload>,
    #[serde(rename = "lowCtrQueries")]
    pub low_ctr_queries: Vec<SearchAnalyticsQueryRowPayload>,
    #[serde(rename = "abandonmentQueries")]
    pub abandonment_queries: Vec<SearchAnalyticsQueryRowPayload>,
    #[serde(rename = "intelligenceCandidates")]
    pub intelligence_candidates: Vec<SearchAnalyticsInsightRowPayload>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchSynonymPayload {
    pub id: String,
    pub term: String,
    pub synonyms: Vec<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchStopWordPayload {
    pub id: String,
    pub value: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchQueryRulePayload {
    pub id: String,
    #[serde(rename = "queryText")]
    pub query_text: String,
    #[serde(rename = "queryNormalized")]
    pub query_normalized: String,
    #[serde(rename = "ruleKind")]
    pub rule_kind: String,
    #[serde(rename = "documentId")]
    pub document_id: String,
    #[serde(rename = "entityType")]
    pub entity_type: String,
    #[serde(rename = "sourceModule")]
    pub source_module: String,
    pub title: String,
    #[serde(rename = "pinnedPosition")]
    pub pinned_position: u32,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchDictionarySnapshotPayload {
    pub synonyms: Vec<SearchSynonymPayload>,
    #[serde(rename = "stopWords")]
    pub stop_words: Vec<SearchStopWordPayload>,
    #[serde(rename = "queryRules")]
    pub query_rules: Vec<SearchQueryRulePayload>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchDictionaryMutationPayload {
    pub success: bool,
}
