use async_graphql::{InputObject, SimpleObject};
use rustok_search::{
    LaggingSearchDocument, SearchAnalyticsInsightRow, SearchAnalyticsQueryRow,
    SearchAnalyticsSnapshot, SearchAnalyticsSummary, SearchConnectorDescriptor,
    SearchConsistencyIssue, SearchDiagnosticsSnapshot, SearchDictionarySnapshot,
    SearchFilterPreset, SearchQueryRuleRecord, SearchResult, SearchResultItem,
    SearchSettingsRecord, SearchStopWordRecord, SearchSuggestion, SearchSynonymRecord,
};

#[derive(Debug, Clone, SimpleObject)]
pub struct SearchEngineDescriptor {
    pub kind: String,
    pub label: String,
    pub provided_by: String,
    pub enabled: bool,
    pub default_engine: bool,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct SearchSettingsPayload {
    pub tenant_id: Option<String>,
    pub active_engine: String,
    pub fallback_engine: String,
    pub config: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, InputObject)]
pub struct UpdateSearchSettingsInput {
    pub tenant_id: Option<String>,
    pub active_engine: String,
    pub fallback_engine: Option<String>,
    pub config: String,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct UpdateSearchSettingsPayload {
    pub success: bool,
    pub settings: SearchSettingsPayload,
}

#[derive(Debug, Clone, InputObject)]
pub struct TriggerSearchRebuildInput {
    pub tenant_id: Option<String>,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct TriggerSearchRebuildPayload {
    pub success: bool,
    pub queued: bool,
    pub tenant_id: String,
    pub target_type: String,
    pub target_id: Option<String>,
}

#[derive(Debug, Clone, InputObject)]
pub struct TrackSearchClickInput {
    pub query_log_id: String,
    pub document_id: String,
    pub position: Option<i32>,
    pub href: Option<String>,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct TrackSearchClickPayload {
    pub success: bool,
    pub tracked: bool,
}

#[derive(Debug, Clone, InputObject)]
pub struct SearchPreviewInput {
    pub query: String,
    pub locale: Option<String>,
    pub tenant_id: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    pub ranking_profile: Option<String>,
    pub preset_key: Option<String>,
    pub entity_types: Option<Vec<String>>,
    pub source_modules: Option<Vec<String>>,
    pub statuses: Option<Vec<String>>,
}

#[derive(Debug, Clone, InputObject)]
pub struct SearchFilterPresetsInput {
    pub tenant_id: Option<String>,
    pub surface: String,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct SearchFilterPresetPayload {
    pub key: String,
    pub label: String,
    pub entity_types: Vec<String>,
    pub source_modules: Vec<String>,
    pub statuses: Vec<String>,
    pub ranking_profile: Option<String>,
}

#[derive(Debug, Clone, InputObject)]
pub struct SearchSuggestionsInput {
    pub query: String,
    pub locale: Option<String>,
    pub tenant_id: Option<String>,
    pub limit: Option<i32>,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct SearchSuggestionPayload {
    pub text: String,
    pub kind: String,
    pub document_id: Option<String>,
    pub entity_type: Option<String>,
    pub source_module: Option<String>,
    pub locale: Option<String>,
    pub url: Option<String>,
    pub score: f64,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct SearchFacetBucketPayload {
    pub value: String,
    pub count: u64,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct SearchFacetGroupPayload {
    pub name: String,
    pub buckets: Vec<SearchFacetBucketPayload>,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct SearchPreviewResultItem {
    pub id: String,
    pub entity_type: String,
    pub source_module: String,
    pub title: String,
    pub snippet: Option<String>,
    pub score: f64,
    pub locale: Option<String>,
    pub url: Option<String>,
    pub payload: String,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct SearchPreviewPayload {
    pub query_log_id: Option<String>,
    pub preset_key: Option<String>,
    pub items: Vec<SearchPreviewResultItem>,
    pub total: u64,
    pub took_ms: u64,
    pub engine: String,
    pub ranking_profile: String,
    pub facets: Vec<SearchFacetGroupPayload>,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct SearchDiagnosticsPayload {
    pub tenant_id: String,
    pub total_documents: u64,
    pub public_documents: u64,
    pub content_documents: u64,
    pub product_documents: u64,
    pub stale_documents: u64,
    pub missing_documents: u64,
    pub orphaned_documents: u64,
    pub newest_indexed_at: Option<String>,
    pub oldest_indexed_at: Option<String>,
    pub max_lag_seconds: u64,
    pub state: String,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct SearchConsistencyIssuePayload {
    pub issue_kind: String,
    pub document_key: String,
    pub document_id: String,
    pub source_module: String,
    pub entity_type: String,
    pub locale: String,
    pub status: String,
    pub title: String,
    pub updated_at: String,
    pub indexed_at: Option<String>,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct LaggingSearchDocumentPayload {
    pub document_key: String,
    pub document_id: String,
    pub source_module: String,
    pub entity_type: String,
    pub locale: String,
    pub status: String,
    pub is_public: bool,
    pub title: String,
    pub updated_at: String,
    pub indexed_at: String,
    pub lag_seconds: u64,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct SearchAnalyticsSummaryPayload {
    pub window_days: u32,
    pub total_queries: u64,
    pub successful_queries: u64,
    pub zero_result_queries: u64,
    pub zero_result_rate: f64,
    pub slow_queries: u64,
    pub slow_query_rate: f64,
    pub avg_took_ms: f64,
    pub avg_results_per_query: f64,
    pub unique_queries: u64,
    pub clicked_queries: u64,
    pub total_clicks: u64,
    pub click_through_rate: f64,
    pub abandonment_queries: u64,
    pub abandonment_rate: f64,
    pub last_query_at: Option<String>,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct SearchAnalyticsQueryRowPayload {
    pub query: String,
    pub hits: u64,
    pub zero_result_hits: u64,
    pub clicks: u64,
    pub avg_took_ms: f64,
    pub avg_results: f64,
    pub click_through_rate: f64,
    pub abandonment_rate: f64,
    pub last_seen_at: String,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct SearchAnalyticsInsightRowPayload {
    pub query: String,
    pub hits: u64,
    pub zero_result_hits: u64,
    pub clicks: u64,
    pub click_through_rate: f64,
    pub abandonment_rate: f64,
    pub recommendation: String,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct SearchAnalyticsPayload {
    pub summary: SearchAnalyticsSummaryPayload,
    pub top_queries: Vec<SearchAnalyticsQueryRowPayload>,
    pub zero_result_queries: Vec<SearchAnalyticsQueryRowPayload>,
    pub slow_queries: Vec<SearchAnalyticsQueryRowPayload>,
    pub low_ctr_queries: Vec<SearchAnalyticsQueryRowPayload>,
    pub abandonment_queries: Vec<SearchAnalyticsQueryRowPayload>,
    pub intelligence_candidates: Vec<SearchAnalyticsInsightRowPayload>,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct SearchSynonymPayload {
    pub id: String,
    pub term: String,
    pub synonyms: Vec<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct SearchStopWordPayload {
    pub id: String,
    pub value: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct SearchQueryRulePayload {
    pub id: String,
    pub query_text: String,
    pub query_normalized: String,
    pub rule_kind: String,
    pub document_id: String,
    pub entity_type: String,
    pub source_module: String,
    pub title: String,
    pub pinned_position: u32,
    pub updated_at: String,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct SearchDictionarySnapshotPayload {
    pub synonyms: Vec<SearchSynonymPayload>,
    pub stop_words: Vec<SearchStopWordPayload>,
    pub query_rules: Vec<SearchQueryRulePayload>,
}

#[derive(Debug, Clone, InputObject)]
pub struct UpsertSearchSynonymInput {
    pub tenant_id: Option<String>,
    pub term: String,
    pub synonyms: Vec<String>,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct UpsertSearchSynonymPayload {
    pub success: bool,
    pub synonym: SearchSynonymPayload,
}

#[derive(Debug, Clone, InputObject)]
pub struct DeleteSearchSynonymInput {
    pub tenant_id: Option<String>,
    pub synonym_id: String,
}

#[derive(Debug, Clone, InputObject)]
pub struct AddSearchStopWordInput {
    pub tenant_id: Option<String>,
    pub value: String,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct AddSearchStopWordPayload {
    pub success: bool,
    pub stop_word: SearchStopWordPayload,
}

#[derive(Debug, Clone, InputObject)]
pub struct DeleteSearchStopWordInput {
    pub tenant_id: Option<String>,
    pub stop_word_id: String,
}

#[derive(Debug, Clone, InputObject)]
pub struct UpsertSearchPinRuleInput {
    pub tenant_id: Option<String>,
    pub query_text: String,
    pub document_id: String,
    pub pinned_position: Option<i32>,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct UpsertSearchPinRulePayload {
    pub success: bool,
    pub query_rule: SearchQueryRulePayload,
}

#[derive(Debug, Clone, InputObject)]
pub struct DeleteSearchQueryRuleInput {
    pub tenant_id: Option<String>,
    pub query_rule_id: String,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct DeleteSearchDictionaryEntryPayload {
    pub success: bool,
}

impl From<SearchConnectorDescriptor> for SearchEngineDescriptor {
    fn from(value: SearchConnectorDescriptor) -> Self {
        Self {
            kind: value.kind.as_str().to_string(),
            label: value.label,
            provided_by: value.provided_by,
            enabled: value.enabled,
            default_engine: value.default_engine,
        }
    }
}

impl From<SearchSettingsRecord> for SearchSettingsPayload {
    fn from(value: SearchSettingsRecord) -> Self {
        Self {
            tenant_id: value.tenant_id.map(|tenant_id| tenant_id.to_string()),
            active_engine: value.active_engine.as_str().to_string(),
            fallback_engine: value.fallback_engine.as_str().to_string(),
            config: value.config.to_string(),
            updated_at: value.updated_at.to_rfc3339(),
        }
    }
}

impl From<SearchResultItem> for SearchPreviewResultItem {
    fn from(value: SearchResultItem) -> Self {
        let url = derive_search_result_url(&value);
        Self {
            id: value.id.to_string(),
            entity_type: value.entity_type,
            source_module: value.source_module,
            title: value.title,
            snippet: value.snippet,
            score: value.score,
            locale: value.locale,
            url,
            payload: value.payload.to_string(),
        }
    }
}

impl From<SearchResult> for SearchPreviewPayload {
    fn from(value: SearchResult) -> Self {
        Self {
            query_log_id: None,
            preset_key: None,
            items: value.items.into_iter().map(Into::into).collect(),
            total: value.total,
            took_ms: value.took_ms,
            engine: value.engine.as_str().to_string(),
            ranking_profile: value.ranking_profile.as_str().to_string(),
            facets: value
                .facets
                .into_iter()
                .map(|facet| SearchFacetGroupPayload {
                    name: facet.name,
                    buckets: facet
                        .buckets
                        .into_iter()
                        .map(|bucket| SearchFacetBucketPayload {
                            value: bucket.value,
                            count: bucket.count,
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

impl From<SearchFilterPreset> for SearchFilterPresetPayload {
    fn from(value: SearchFilterPreset) -> Self {
        Self {
            key: value.key,
            label: value.label,
            entity_types: value.entity_types,
            source_modules: value.source_modules,
            statuses: value.statuses,
            ranking_profile: value
                .ranking_profile
                .map(|value| value.as_str().to_string()),
        }
    }
}

impl From<SearchDiagnosticsSnapshot> for SearchDiagnosticsPayload {
    fn from(value: SearchDiagnosticsSnapshot) -> Self {
        Self {
            tenant_id: value.tenant_id.to_string(),
            total_documents: value.total_documents,
            public_documents: value.public_documents,
            content_documents: value.content_documents,
            product_documents: value.product_documents,
            stale_documents: value.stale_documents,
            missing_documents: value.missing_documents,
            orphaned_documents: value.orphaned_documents,
            newest_indexed_at: value.newest_indexed_at.map(|value| value.to_rfc3339()),
            oldest_indexed_at: value.oldest_indexed_at.map(|value| value.to_rfc3339()),
            max_lag_seconds: value.max_lag_seconds,
            state: value.state,
        }
    }
}

impl From<SearchConsistencyIssue> for SearchConsistencyIssuePayload {
    fn from(value: SearchConsistencyIssue) -> Self {
        Self {
            issue_kind: value.issue_kind,
            document_key: value.document_key,
            document_id: value.document_id.to_string(),
            source_module: value.source_module,
            entity_type: value.entity_type,
            locale: value.locale,
            status: value.status,
            title: value.title,
            updated_at: value.updated_at.to_rfc3339(),
            indexed_at: value.indexed_at.map(|value| value.to_rfc3339()),
        }
    }
}

impl From<LaggingSearchDocument> for LaggingSearchDocumentPayload {
    fn from(value: LaggingSearchDocument) -> Self {
        Self {
            document_key: value.document_key,
            document_id: value.document_id.to_string(),
            source_module: value.source_module,
            entity_type: value.entity_type,
            locale: value.locale,
            status: value.status,
            is_public: value.is_public,
            title: value.title,
            updated_at: value.updated_at.to_rfc3339(),
            indexed_at: value.indexed_at.to_rfc3339(),
            lag_seconds: value.lag_seconds,
        }
    }
}

impl From<SearchAnalyticsSummary> for SearchAnalyticsSummaryPayload {
    fn from(value: SearchAnalyticsSummary) -> Self {
        Self {
            window_days: value.window_days,
            total_queries: value.total_queries,
            successful_queries: value.successful_queries,
            zero_result_queries: value.zero_result_queries,
            zero_result_rate: value.zero_result_rate,
            slow_queries: value.slow_queries,
            slow_query_rate: value.slow_query_rate,
            avg_took_ms: value.avg_took_ms,
            avg_results_per_query: value.avg_results_per_query,
            unique_queries: value.unique_queries,
            clicked_queries: value.clicked_queries,
            total_clicks: value.total_clicks,
            click_through_rate: value.click_through_rate,
            abandonment_queries: value.abandonment_queries,
            abandonment_rate: value.abandonment_rate,
            last_query_at: value.last_query_at.map(|value| value.to_rfc3339()),
        }
    }
}

impl From<SearchAnalyticsQueryRow> for SearchAnalyticsQueryRowPayload {
    fn from(value: SearchAnalyticsQueryRow) -> Self {
        Self {
            query: value.query,
            hits: value.hits,
            zero_result_hits: value.zero_result_hits,
            clicks: value.clicks,
            avg_took_ms: value.avg_took_ms,
            avg_results: value.avg_results,
            click_through_rate: value.click_through_rate,
            abandonment_rate: value.abandonment_rate,
            last_seen_at: value.last_seen_at.to_rfc3339(),
        }
    }
}

impl From<SearchAnalyticsInsightRow> for SearchAnalyticsInsightRowPayload {
    fn from(value: SearchAnalyticsInsightRow) -> Self {
        Self {
            query: value.query,
            hits: value.hits,
            zero_result_hits: value.zero_result_hits,
            clicks: value.clicks,
            click_through_rate: value.click_through_rate,
            abandonment_rate: value.abandonment_rate,
            recommendation: value.recommendation,
        }
    }
}

impl From<SearchAnalyticsSnapshot> for SearchAnalyticsPayload {
    fn from(value: SearchAnalyticsSnapshot) -> Self {
        Self {
            summary: value.summary.into(),
            top_queries: value.top_queries.into_iter().map(Into::into).collect(),
            zero_result_queries: value
                .zero_result_queries
                .into_iter()
                .map(Into::into)
                .collect(),
            slow_queries: value.slow_queries.into_iter().map(Into::into).collect(),
            low_ctr_queries: value.low_ctr_queries.into_iter().map(Into::into).collect(),
            abandonment_queries: value
                .abandonment_queries
                .into_iter()
                .map(Into::into)
                .collect(),
            intelligence_candidates: value
                .intelligence_candidates
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

impl From<SearchSynonymRecord> for SearchSynonymPayload {
    fn from(value: SearchSynonymRecord) -> Self {
        Self {
            id: value.id.to_string(),
            term: value.term,
            synonyms: value.synonyms,
            updated_at: value.updated_at.to_rfc3339(),
        }
    }
}

impl From<SearchStopWordRecord> for SearchStopWordPayload {
    fn from(value: SearchStopWordRecord) -> Self {
        Self {
            id: value.id.to_string(),
            value: value.value,
            updated_at: value.updated_at.to_rfc3339(),
        }
    }
}

impl From<SearchQueryRuleRecord> for SearchQueryRulePayload {
    fn from(value: SearchQueryRuleRecord) -> Self {
        Self {
            id: value.id.to_string(),
            query_text: value.query_text,
            query_normalized: value.query_normalized,
            rule_kind: value.rule_kind,
            document_id: value.document_id.to_string(),
            entity_type: value.entity_type,
            source_module: value.source_module,
            title: value.title,
            pinned_position: value.pinned_position,
            updated_at: value.updated_at.to_rfc3339(),
        }
    }
}

impl From<SearchDictionarySnapshot> for SearchDictionarySnapshotPayload {
    fn from(value: SearchDictionarySnapshot) -> Self {
        Self {
            synonyms: value.synonyms.into_iter().map(Into::into).collect(),
            stop_words: value.stop_words.into_iter().map(Into::into).collect(),
            query_rules: value.query_rules.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<SearchSuggestion> for SearchSuggestionPayload {
    fn from(value: SearchSuggestion) -> Self {
        Self {
            text: value.text,
            kind: value.kind.as_str().to_string(),
            document_id: value.document_id.map(|value| value.to_string()),
            entity_type: value.entity_type,
            source_module: value.source_module,
            locale: value.locale,
            url: value.url,
            score: value.score,
        }
    }
}

fn derive_search_result_url(value: &SearchResultItem) -> Option<String> {
    match value.entity_type.as_str() {
        "product" => Some(format!("/store/products/{}", value.id)),
        "node" => Some(format!(
            "/modules/content?id={}{}",
            value.id,
            if value.source_module.is_empty() || value.source_module == "content" {
                String::new()
            } else {
                format!("&kind={}", value.source_module)
            }
        )),
        _ => None,
    }
}
