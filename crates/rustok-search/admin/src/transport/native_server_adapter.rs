use leptos::prelude::*;
#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::model::{
    LaggingSearchDocumentPayload, SearchAdminBootstrap, SearchAnalyticsPayload,
    SearchConsistencyIssuePayload, SearchDictionaryMutationPayload,
    SearchDictionarySnapshotPayload, SearchFilterPresetPayload, SearchPreviewFilters,
    SearchPreviewPayload, SearchSettingsPayload, TrackSearchClickPayload,
    TriggerSearchRebuildPayload,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    Graphql(String),
    ServerFn(String),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Graphql(error) => write!(f, "{error}"),
            Self::ServerFn(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<GraphqlHttpError> for ApiError {
    fn from(value: GraphqlHttpError) -> Self {
        Self::Graphql(value.to_string())
    }
}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

#[cfg(feature = "ssr")]
const MAX_SEARCH_QUERY_LEN: usize = 256;
#[cfg(feature = "ssr")]
const MAX_FILTER_VALUES: usize = 10;
#[cfg(feature = "ssr")]
const MAX_FILTER_VALUE_LEN: usize = 64;
#[cfg(feature = "ssr")]
const MAX_LOCALE_LEN: usize = 16;

const SEARCH_ADMIN_BOOTSTRAP_QUERY: &str = "query SearchAdminBootstrap { availableSearchEngines { kind label providedBy enabled defaultEngine } searchSettingsPreview { tenantId activeEngine fallbackEngine config updatedAt } searchDiagnostics { tenantId totalDocuments publicDocuments contentDocuments productDocuments staleDocuments missingDocuments orphanedDocuments newestIndexedAt oldestIndexedAt maxLagSeconds state } }";
const SEARCH_PREVIEW_QUERY: &str = "query SearchPreview($input: SearchPreviewInput!) { searchPreview(input: $input) { queryLogId presetKey total tookMs engine rankingProfile items { id entityType sourceModule title snippet score locale url payload } facets { name buckets { value count } } } }";
const SEARCH_FILTER_PRESETS_QUERY: &str = "query SearchFilterPresets($input: SearchFilterPresetsInput!) { searchFilterPresets(input: $input) { key label entityTypes sourceModules statuses rankingProfile } }";
const SEARCH_LAGGING_DOCUMENTS_QUERY: &str = "query SearchLaggingDocuments($limit: Int) { searchLaggingDocuments(limit: $limit) { documentKey documentId sourceModule entityType locale status isPublic title updatedAt indexedAt lagSeconds } }";
const SEARCH_CONSISTENCY_ISSUES_QUERY: &str = "query SearchConsistencyIssues($limit: Int) { searchConsistencyIssues(limit: $limit) { issueKind documentKey documentId sourceModule entityType locale status title updatedAt indexedAt } }";
const SEARCH_ANALYTICS_QUERY: &str = "query SearchAnalytics($days: Int, $limit: Int) { searchAnalytics(days: $days, limit: $limit) { summary { windowDays totalQueries successfulQueries zeroResultQueries zeroResultRate slowQueries slowQueryRate avgTookMs avgResultsPerQuery uniqueQueries clickedQueries totalClicks clickThroughRate abandonmentQueries abandonmentRate lastQueryAt } topQueries { query hits zeroResultHits clicks avgTookMs avgResults clickThroughRate abandonmentRate lastSeenAt } zeroResultQueries { query hits zeroResultHits clicks avgTookMs avgResults clickThroughRate abandonmentRate lastSeenAt } slowQueries { query hits zeroResultHits clicks avgTookMs avgResults clickThroughRate abandonmentRate lastSeenAt } lowCtrQueries { query hits zeroResultHits clicks avgTookMs avgResults clickThroughRate abandonmentRate lastSeenAt } abandonmentQueries { query hits zeroResultHits clicks avgTookMs avgResults clickThroughRate abandonmentRate lastSeenAt } intelligenceCandidates { query hits zeroResultHits clicks clickThroughRate abandonmentRate recommendation } } }";
const SEARCH_DICTIONARY_SNAPSHOT_QUERY: &str = "query SearchDictionarySnapshot { searchDictionarySnapshot { synonyms { id term synonyms updatedAt } stopWords { id value updatedAt } queryRules { id queryText queryNormalized ruleKind documentId entityType sourceModule title pinnedPosition updatedAt } } }";
const TRIGGER_SEARCH_REBUILD_MUTATION: &str = "mutation TriggerSearchRebuild($input: TriggerSearchRebuildInput!) { triggerSearchRebuild(input: $input) { success queued tenantId targetType targetId } }";
const TRACK_SEARCH_CLICK_MUTATION: &str = "mutation TrackSearchClick($input: TrackSearchClickInput!) { trackSearchClick(input: $input) { success tracked } }";
const UPDATE_SEARCH_SETTINGS_MUTATION: &str = "mutation UpdateSearchSettings($input: UpdateSearchSettingsInput!) { updateSearchSettings(input: $input) { success settings { tenantId activeEngine fallbackEngine config updatedAt } } }";
const UPSERT_SEARCH_SYNONYM_MUTATION: &str = "mutation UpsertSearchSynonym($input: UpsertSearchSynonymInput!) { upsertSearchSynonym(input: $input) { success synonym { id term synonyms updatedAt } } }";
const DELETE_SEARCH_SYNONYM_MUTATION: &str = "mutation DeleteSearchSynonym($input: DeleteSearchSynonymInput!) { deleteSearchSynonym(input: $input) { success } }";
const ADD_SEARCH_STOP_WORD_MUTATION: &str = "mutation AddSearchStopWord($input: AddSearchStopWordInput!) { addSearchStopWord(input: $input) { success stopWord { id value updatedAt } } }";
const DELETE_SEARCH_STOP_WORD_MUTATION: &str = "mutation DeleteSearchStopWord($input: DeleteSearchStopWordInput!) { deleteSearchStopWord(input: $input) { success } }";
const UPSERT_SEARCH_PIN_RULE_MUTATION: &str = "mutation UpsertSearchPinRule($input: UpsertSearchPinRuleInput!) { upsertSearchPinRule(input: $input) { success queryRule { id queryText queryNormalized ruleKind documentId entityType sourceModule title pinnedPosition updatedAt } } }";
const DELETE_SEARCH_QUERY_RULE_MUTATION: &str = "mutation DeleteSearchQueryRule($input: DeleteSearchQueryRuleInput!) { deleteSearchQueryRule(input: $input) { success } }";

#[derive(Debug, Deserialize)]
struct SearchPreviewResponse {
    #[serde(rename = "searchPreview")]
    search_preview: SearchPreviewPayload,
}

#[derive(Debug, Deserialize)]
struct TriggerSearchRebuildResponse {
    #[serde(rename = "triggerSearchRebuild")]
    trigger_search_rebuild: TriggerSearchRebuildPayload,
}

#[derive(Debug, Deserialize)]
struct SearchLaggingDocumentsResponse {
    #[serde(rename = "searchLaggingDocuments")]
    search_lagging_documents: Vec<LaggingSearchDocumentPayload>,
}

#[derive(Debug, Deserialize)]
struct SearchConsistencyIssuesResponse {
    #[serde(rename = "searchConsistencyIssues")]
    search_consistency_issues: Vec<SearchConsistencyIssuePayload>,
}

#[derive(Debug, Deserialize)]
struct SearchAnalyticsResponse {
    #[serde(rename = "searchAnalytics")]
    search_analytics: SearchAnalyticsPayload,
}

#[derive(Debug, Deserialize)]
struct SearchFilterPresetsResponse {
    #[serde(rename = "searchFilterPresets")]
    search_filter_presets: Vec<SearchFilterPresetPayload>,
}

#[derive(Debug, Deserialize)]
struct SearchDictionarySnapshotResponse {
    #[serde(rename = "searchDictionarySnapshot")]
    search_dictionary_snapshot: SearchDictionarySnapshotPayload,
}

#[derive(Debug, Deserialize)]
struct TrackSearchClickResponse {
    #[serde(rename = "trackSearchClick")]
    track_search_click: TrackSearchClickPayload,
}

#[derive(Debug, Deserialize)]
struct UpdateSearchSettingsResponse {
    #[serde(rename = "updateSearchSettings")]
    update_search_settings: UpdateSearchSettingsEnvelope,
}

#[derive(Debug, Deserialize)]
struct SearchDictionaryMutationResponse {
    #[serde(rename = "success")]
    success: bool,
}

#[derive(Debug, Deserialize)]
struct UpsertSearchSynonymResponse {
    #[serde(rename = "upsertSearchSynonym")]
    upsert_search_synonym: SearchDictionaryMutationEnvelope,
}

#[derive(Debug, Deserialize)]
struct AddSearchStopWordResponse {
    #[serde(rename = "addSearchStopWord")]
    add_search_stop_word: SearchDictionaryMutationEnvelope,
}

#[derive(Debug, Deserialize)]
struct UpsertSearchPinRuleResponse {
    #[serde(rename = "upsertSearchPinRule")]
    upsert_search_pin_rule: SearchDictionaryMutationEnvelope,
}

#[derive(Debug, Deserialize)]
struct DeleteSearchSynonymResponse {
    #[serde(rename = "deleteSearchSynonym")]
    delete_search_synonym: SearchDictionaryMutationResponse,
}

#[derive(Debug, Deserialize)]
struct DeleteSearchStopWordResponse {
    #[serde(rename = "deleteSearchStopWord")]
    delete_search_stop_word: SearchDictionaryMutationResponse,
}

#[derive(Debug, Deserialize)]
struct DeleteSearchQueryRuleResponse {
    #[serde(rename = "deleteSearchQueryRule")]
    delete_search_query_rule: SearchDictionaryMutationResponse,
}

#[derive(Debug, Serialize)]
struct SearchPreviewVariables {
    input: SearchPreviewInput,
}

#[derive(Debug, Serialize)]
struct TriggerSearchRebuildVariables {
    input: TriggerSearchRebuildInput,
}

#[derive(Debug, Serialize)]
struct SearchLaggingDocumentsVariables {
    limit: Option<i32>,
}

#[derive(Debug, Serialize)]
struct SearchAnalyticsVariables {
    days: Option<i32>,
    limit: Option<i32>,
}

#[derive(Debug, Serialize)]
struct SearchFilterPresetsVariables {
    input: SearchFilterPresetsInput,
}

#[derive(Debug, Serialize)]
struct TrackSearchClickVariables {
    input: TrackSearchClickInput,
}

#[derive(Debug, Serialize)]
struct UpdateSearchSettingsVariables {
    input: UpdateSearchSettingsInput,
}

#[derive(Debug, Deserialize)]
struct SearchDictionaryMutationEnvelope {
    success: bool,
}

#[derive(Debug, Deserialize)]
struct UpdateSearchSettingsEnvelope {
    settings: SearchSettingsPayload,
}

#[derive(Debug, Serialize)]
struct SearchPreviewInput {
    query: String,
    locale: Option<String>,
    #[serde(rename = "tenantId")]
    tenant_id: Option<String>,
    limit: Option<i32>,
    offset: Option<i32>,
    #[serde(rename = "rankingProfile")]
    ranking_profile: Option<String>,
    #[serde(rename = "presetKey")]
    preset_key: Option<String>,
    #[serde(rename = "entityTypes")]
    entity_types: Option<Vec<String>>,
    #[serde(rename = "sourceModules")]
    source_modules: Option<Vec<String>>,
    statuses: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct TriggerSearchRebuildInput {
    #[serde(rename = "tenantId")]
    tenant_id: Option<String>,
    #[serde(rename = "targetType")]
    target_type: Option<String>,
    #[serde(rename = "targetId")]
    target_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct SearchFilterPresetsInput {
    #[serde(rename = "tenantId")]
    tenant_id: Option<String>,
    surface: String,
}

#[derive(Debug, Serialize)]
struct TrackSearchClickInput {
    #[serde(rename = "queryLogId")]
    query_log_id: String,
    #[serde(rename = "documentId")]
    document_id: String,
    position: Option<i32>,
    href: Option<String>,
}

#[derive(Debug, Serialize)]
struct UpdateSearchSettingsInput {
    #[serde(rename = "tenantId")]
    tenant_id: Option<String>,
    #[serde(rename = "activeEngine")]
    active_engine: String,
    #[serde(rename = "fallbackEngine")]
    fallback_engine: Option<String>,
    config: String,
}

#[derive(Debug, Serialize)]
struct UpsertSearchSynonymInput {
    #[serde(rename = "tenantId")]
    tenant_id: Option<String>,
    term: String,
    synonyms: Vec<String>,
}

#[derive(Debug, Serialize)]
struct DeleteSearchSynonymInput {
    #[serde(rename = "tenantId")]
    tenant_id: Option<String>,
    #[serde(rename = "synonymId")]
    synonym_id: String,
}

#[derive(Debug, Serialize)]
struct AddSearchStopWordInput {
    #[serde(rename = "tenantId")]
    tenant_id: Option<String>,
    value: String,
}

#[derive(Debug, Serialize)]
struct DeleteSearchStopWordInput {
    #[serde(rename = "tenantId")]
    tenant_id: Option<String>,
    #[serde(rename = "stopWordId")]
    stop_word_id: String,
}

#[derive(Debug, Serialize)]
struct UpsertSearchPinRuleInput {
    #[serde(rename = "tenantId")]
    tenant_id: Option<String>,
    #[serde(rename = "queryText")]
    query_text: String,
    #[serde(rename = "documentId")]
    document_id: String,
    #[serde(rename = "pinnedPosition")]
    pinned_position: Option<i32>,
}

#[derive(Debug, Serialize)]
struct DeleteSearchQueryRuleInput {
    #[serde(rename = "tenantId")]
    tenant_id: Option<String>,
    #[serde(rename = "queryRuleId")]
    query_rule_id: String,
}

#[derive(Debug, Serialize)]
struct UpsertSearchSynonymVariables {
    input: UpsertSearchSynonymInput,
}

#[derive(Debug, Serialize)]
struct DeleteSearchSynonymVariables {
    input: DeleteSearchSynonymInput,
}

#[derive(Debug, Serialize)]
struct AddSearchStopWordVariables {
    input: AddSearchStopWordInput,
}

#[derive(Debug, Serialize)]
struct DeleteSearchStopWordVariables {
    input: DeleteSearchStopWordInput,
}

#[derive(Debug, Serialize)]
struct UpsertSearchPinRuleVariables {
    input: UpsertSearchPinRuleInput,
}

#[derive(Debug, Serialize)]
struct DeleteSearchQueryRuleVariables {
    input: DeleteSearchQueryRuleInput,
}

fn graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }

    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{origin}/api/graphql")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{base}/api/graphql")
    }
}

async fn request<V, T>(
    query: &str,
    variables: Option<V>,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, variables),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(ApiError::from)
}

pub async fn fetch_bootstrap(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<SearchAdminBootstrap, ApiError> {
    match search_admin_bootstrap_native().await {
        Ok(payload) => Ok(payload),
        Err(_) => {
            request::<serde_json::Value, SearchAdminBootstrap>(
                SEARCH_ADMIN_BOOTSTRAP_QUERY,
                None,
                token,
                tenant_slug,
            )
            .await
        }
    }
}

pub async fn fetch_search_preview(
    token: Option<String>,
    tenant_slug: Option<String>,
    query: String,
    locale: Option<String>,
    ranking_profile: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
) -> Result<SearchPreviewPayload, ApiError> {
    match search_admin_preview_native(
        query.clone(),
        locale.clone(),
        ranking_profile.clone(),
        preset_key.clone(),
        filters.clone(),
    )
    .await
    {
        Ok(payload) => Ok(payload),
        Err(_) => {
            let response: SearchPreviewResponse = request(
                SEARCH_PREVIEW_QUERY,
                Some(SearchPreviewVariables {
                    input: SearchPreviewInput {
                        query,
                        locale,
                        tenant_id: None,
                        limit: Some(12),
                        offset: Some(0),
                        ranking_profile,
                        preset_key,
                        entity_types: (!filters.entity_types.is_empty())
                            .then_some(filters.entity_types),
                        source_modules: (!filters.source_modules.is_empty())
                            .then_some(filters.source_modules),
                        statuses: (!filters.statuses.is_empty()).then_some(filters.statuses),
                    },
                }),
                token,
                tenant_slug,
            )
            .await?;

            Ok(response.search_preview)
        }
    }
}

pub async fn fetch_filter_presets(
    token: Option<String>,
    tenant_slug: Option<String>,
    surface: &str,
) -> Result<Vec<SearchFilterPresetPayload>, ApiError> {
    match search_admin_filter_presets_native(surface.to_string()).await {
        Ok(payload) => Ok(payload),
        Err(_) => {
            let response: SearchFilterPresetsResponse = request(
                SEARCH_FILTER_PRESETS_QUERY,
                Some(SearchFilterPresetsVariables {
                    input: SearchFilterPresetsInput {
                        tenant_id: None,
                        surface: surface.to_string(),
                    },
                }),
                token,
                tenant_slug,
            )
            .await?;

            Ok(response.search_filter_presets)
        }
    }
}

pub async fn trigger_search_rebuild(
    token: Option<String>,
    tenant_slug: Option<String>,
    target_type: Option<String>,
    target_id: Option<String>,
) -> Result<TriggerSearchRebuildPayload, ApiError> {
    match trigger_search_rebuild_native(target_type.clone(), target_id.clone()).await {
        Ok(payload) => Ok(payload),
        Err(_) => {
            let response: TriggerSearchRebuildResponse = request(
                TRIGGER_SEARCH_REBUILD_MUTATION,
                Some(TriggerSearchRebuildVariables {
                    input: TriggerSearchRebuildInput {
                        tenant_id: None,
                        target_type,
                        target_id,
                    },
                }),
                token,
                tenant_slug,
            )
            .await?;

            Ok(response.trigger_search_rebuild)
        }
    }
}

pub async fn fetch_lagging_documents(
    token: Option<String>,
    tenant_slug: Option<String>,
    limit: Option<i32>,
) -> Result<Vec<LaggingSearchDocumentPayload>, ApiError> {
    match search_admin_lagging_documents_native(limit).await {
        Ok(payload) => Ok(payload),
        Err(_) => {
            let response: SearchLaggingDocumentsResponse = request(
                SEARCH_LAGGING_DOCUMENTS_QUERY,
                Some(SearchLaggingDocumentsVariables { limit }),
                token,
                tenant_slug,
            )
            .await?;

            Ok(response.search_lagging_documents)
        }
    }
}

pub async fn fetch_consistency_issues(
    token: Option<String>,
    tenant_slug: Option<String>,
    limit: Option<i32>,
) -> Result<Vec<SearchConsistencyIssuePayload>, ApiError> {
    match search_admin_consistency_issues_native(limit).await {
        Ok(payload) => Ok(payload),
        Err(_) => {
            let response: SearchConsistencyIssuesResponse = request(
                SEARCH_CONSISTENCY_ISSUES_QUERY,
                Some(SearchLaggingDocumentsVariables { limit }),
                token,
                tenant_slug,
            )
            .await?;

            Ok(response.search_consistency_issues)
        }
    }
}

pub async fn fetch_search_analytics(
    token: Option<String>,
    tenant_slug: Option<String>,
    days: Option<i32>,
    limit: Option<i32>,
) -> Result<SearchAnalyticsPayload, ApiError> {
    match search_admin_analytics_native(days, limit).await {
        Ok(payload) => Ok(payload),
        Err(_) => {
            let response: SearchAnalyticsResponse = request(
                SEARCH_ANALYTICS_QUERY,
                Some(SearchAnalyticsVariables { days, limit }),
                token,
                tenant_slug,
            )
            .await?;

            Ok(response.search_analytics)
        }
    }
}

pub async fn fetch_dictionary_snapshot(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<SearchDictionarySnapshotPayload, ApiError> {
    match search_admin_dictionary_snapshot_native().await {
        Ok(payload) => Ok(payload),
        Err(_) => {
            let response: SearchDictionarySnapshotResponse = request::<serde_json::Value, _>(
                SEARCH_DICTIONARY_SNAPSHOT_QUERY,
                None,
                token,
                tenant_slug,
            )
            .await?;

            Ok(response.search_dictionary_snapshot)
        }
    }
}

pub async fn track_search_click(
    token: Option<String>,
    tenant_slug: Option<String>,
    query_log_id: String,
    document_id: String,
    position: Option<i32>,
    href: Option<String>,
) -> Result<TrackSearchClickPayload, ApiError> {
    match track_search_click_native(
        query_log_id.clone(),
        document_id.clone(),
        position,
        href.clone(),
    )
    .await
    {
        Ok(payload) => Ok(payload),
        Err(_) => {
            let response: TrackSearchClickResponse = request(
                TRACK_SEARCH_CLICK_MUTATION,
                Some(TrackSearchClickVariables {
                    input: TrackSearchClickInput {
                        query_log_id,
                        document_id,
                        position,
                        href,
                    },
                }),
                token,
                tenant_slug,
            )
            .await?;

            Ok(response.track_search_click)
        }
    }
}

pub async fn update_search_settings(
    token: Option<String>,
    tenant_slug: Option<String>,
    active_engine: String,
    fallback_engine: Option<String>,
    config: String,
) -> Result<SearchSettingsPayload, ApiError> {
    match update_search_settings_native(
        active_engine.clone(),
        fallback_engine.clone(),
        config.clone(),
    )
    .await
    {
        Ok(payload) => Ok(payload),
        Err(_) => {
            let response: UpdateSearchSettingsResponse = request(
                UPDATE_SEARCH_SETTINGS_MUTATION,
                Some(UpdateSearchSettingsVariables {
                    input: UpdateSearchSettingsInput {
                        tenant_id: None,
                        active_engine,
                        fallback_engine,
                        config,
                    },
                }),
                token,
                tenant_slug,
            )
            .await?;

            Ok(response.update_search_settings.settings)
        }
    }
}

pub async fn upsert_search_synonym(
    token: Option<String>,
    tenant_slug: Option<String>,
    term: String,
    synonyms: Vec<String>,
) -> Result<SearchDictionaryMutationPayload, ApiError> {
    match upsert_search_synonym_native(term.clone(), synonyms.clone()).await {
        Ok(payload) => Ok(payload),
        Err(_) => {
            let response: UpsertSearchSynonymResponse = request(
                UPSERT_SEARCH_SYNONYM_MUTATION,
                Some(UpsertSearchSynonymVariables {
                    input: UpsertSearchSynonymInput {
                        tenant_id: None,
                        term,
                        synonyms,
                    },
                }),
                token,
                tenant_slug,
            )
            .await?;

            Ok(SearchDictionaryMutationPayload {
                success: response.upsert_search_synonym.success,
            })
        }
    }
}

pub async fn delete_search_synonym(
    token: Option<String>,
    tenant_slug: Option<String>,
    synonym_id: String,
) -> Result<SearchDictionaryMutationPayload, ApiError> {
    match delete_search_synonym_native(synonym_id.clone()).await {
        Ok(payload) => Ok(payload),
        Err(_) => {
            let response: DeleteSearchSynonymResponse = request(
                DELETE_SEARCH_SYNONYM_MUTATION,
                Some(DeleteSearchSynonymVariables {
                    input: DeleteSearchSynonymInput {
                        tenant_id: None,
                        synonym_id,
                    },
                }),
                token,
                tenant_slug,
            )
            .await?;

            Ok(SearchDictionaryMutationPayload {
                success: response.delete_search_synonym.success,
            })
        }
    }
}

pub async fn add_search_stop_word(
    token: Option<String>,
    tenant_slug: Option<String>,
    value: String,
) -> Result<SearchDictionaryMutationPayload, ApiError> {
    match add_search_stop_word_native(value.clone()).await {
        Ok(payload) => Ok(payload),
        Err(_) => {
            let response: AddSearchStopWordResponse = request(
                ADD_SEARCH_STOP_WORD_MUTATION,
                Some(AddSearchStopWordVariables {
                    input: AddSearchStopWordInput {
                        tenant_id: None,
                        value,
                    },
                }),
                token,
                tenant_slug,
            )
            .await?;

            Ok(SearchDictionaryMutationPayload {
                success: response.add_search_stop_word.success,
            })
        }
    }
}

pub async fn delete_search_stop_word(
    token: Option<String>,
    tenant_slug: Option<String>,
    stop_word_id: String,
) -> Result<SearchDictionaryMutationPayload, ApiError> {
    match delete_search_stop_word_native(stop_word_id.clone()).await {
        Ok(payload) => Ok(payload),
        Err(_) => {
            let response: DeleteSearchStopWordResponse = request(
                DELETE_SEARCH_STOP_WORD_MUTATION,
                Some(DeleteSearchStopWordVariables {
                    input: DeleteSearchStopWordInput {
                        tenant_id: None,
                        stop_word_id,
                    },
                }),
                token,
                tenant_slug,
            )
            .await?;

            Ok(SearchDictionaryMutationPayload {
                success: response.delete_search_stop_word.success,
            })
        }
    }
}

pub async fn upsert_search_pin_rule(
    token: Option<String>,
    tenant_slug: Option<String>,
    query_text: String,
    document_id: String,
    pinned_position: Option<i32>,
) -> Result<SearchDictionaryMutationPayload, ApiError> {
    match upsert_search_pin_rule_native(query_text.clone(), document_id.clone(), pinned_position)
        .await
    {
        Ok(payload) => Ok(payload),
        Err(_) => {
            let response: UpsertSearchPinRuleResponse = request(
                UPSERT_SEARCH_PIN_RULE_MUTATION,
                Some(UpsertSearchPinRuleVariables {
                    input: UpsertSearchPinRuleInput {
                        tenant_id: None,
                        query_text,
                        document_id,
                        pinned_position,
                    },
                }),
                token,
                tenant_slug,
            )
            .await?;

            Ok(SearchDictionaryMutationPayload {
                success: response.upsert_search_pin_rule.success,
            })
        }
    }
}

pub async fn delete_search_query_rule(
    token: Option<String>,
    tenant_slug: Option<String>,
    query_rule_id: String,
) -> Result<SearchDictionaryMutationPayload, ApiError> {
    match delete_search_query_rule_native(query_rule_id.clone()).await {
        Ok(payload) => Ok(payload),
        Err(_) => {
            let response: DeleteSearchQueryRuleResponse = request(
                DELETE_SEARCH_QUERY_RULE_MUTATION,
                Some(DeleteSearchQueryRuleVariables {
                    input: DeleteSearchQueryRuleInput {
                        tenant_id: None,
                        query_rule_id,
                    },
                }),
                token,
                tenant_slug,
            )
            .await?;

            Ok(SearchDictionaryMutationPayload {
                success: response.delete_search_query_rule.success,
            })
        }
    }
}

#[server(prefix = "/api/fn", endpoint = "search/bootstrap")]
async fn search_admin_bootstrap_native() -> Result<SearchAdminBootstrap, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_settings_read_permission(&auth.permissions)?;

        let module = rustok_search::SearchModule;
        let settings =
            rustok_search::SearchSettingsService::load_effective(&app_ctx.db, Some(tenant.id))
                .await
                .map_err(ServerFnError::new)?;
        let diagnostics = rustok_search::SearchDiagnosticsService::snapshot(&app_ctx.db, tenant.id)
            .await
            .map_err(map_core_error)?;

        Ok(SearchAdminBootstrap {
            available_search_engines: module
                .available_engines()
                .into_iter()
                .map(map_search_engine_descriptor)
                .collect(),
            search_settings_preview: map_search_settings_payload(settings),
            search_diagnostics: map_diagnostics_payload(diagnostics),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "search/bootstrap requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/preview")]
async fn search_admin_preview_native(
    query: String,
    locale: Option<String>,
    ranking_profile: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
) -> Result<SearchPreviewPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};
        use std::time::Instant;

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_settings_read_permission(&auth.permissions)?;

        let input = normalize_search_preview_input(SearchPreviewInput {
            query,
            locale,
            tenant_id: None,
            limit: Some(12),
            offset: Some(0),
            ranking_profile,
            preset_key,
            entity_types: Some(filters.entity_types),
            source_modules: Some(filters.source_modules),
            statuses: Some(filters.statuses),
        })?;
        let transform = rustok_search::SearchDictionaryService::transform_query(
            &app_ctx.db,
            tenant.id,
            &input.query,
        )
        .await
        .map_err(map_core_error)?;
        let settings =
            rustok_search::SearchSettingsService::load_effective(&app_ctx.db, Some(tenant.id))
                .await
                .map_err(ServerFnError::new)?;
        let resolved = resolve_preset_and_ranking(
            &settings.config,
            "search_preview",
            input.preset_key.as_deref(),
            input.ranking_profile.as_deref(),
            input.entity_types.unwrap_or_default(),
            input.source_modules.unwrap_or_default(),
            input.statuses.unwrap_or_default(),
        )?;

        let search_query = rustok_search::SearchQuery {
            tenant_id: Some(tenant.id),
            locale: input.locale,
            original_query: transform.original_query,
            query: transform.effective_query,
            ranking_profile: resolved.ranking_profile,
            preset_key: resolved.preset_key,
            limit: 12,
            offset: 0,
            published_only: false,
            entity_types: resolved.entity_types,
            source_modules: resolved.source_modules,
            statuses: resolved.statuses,
        };
        let engine = rustok_search::PgSearchEngine::new(app_ctx.db.clone());
        let started_at = Instant::now();
        let result = run_search_with_dictionaries(&app_ctx.db, &engine, search_query.clone()).await;

        finalize_search_result(
            &app_ctx.db,
            "search_preview",
            &search_query,
            started_at,
            result,
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (query, locale, ranking_profile, preset_key, filters);
        Err(ServerFnError::new(
            "search/preview requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/filter-presets")]
async fn search_admin_filter_presets_native(
    surface: String,
) -> Result<Vec<SearchFilterPresetPayload>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_settings_read_permission(&auth.permissions)?;

        let surface = normalize_surface(&surface)?;
        let settings =
            rustok_search::SearchSettingsService::load_effective(&app_ctx.db, Some(tenant.id))
                .await
                .map_err(ServerFnError::new)?;

        Ok(
            rustok_search::SearchFilterPresetService::list(&settings.config, &surface)
                .into_iter()
                .map(|value| SearchFilterPresetPayload {
                    key: value.key,
                    label: value.label,
                    entity_types: value.entity_types,
                    source_modules: value.source_modules,
                    statuses: value.statuses,
                    ranking_profile: value
                        .ranking_profile
                        .map(|value| value.as_str().to_string()),
                })
                .collect(),
        )
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = surface;
        Err(ServerFnError::new(
            "search/filter-presets requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/lagging-documents")]
async fn search_admin_lagging_documents_native(
    limit: Option<i32>,
) -> Result<Vec<LaggingSearchDocumentPayload>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_settings_read_permission(&auth.permissions)?;

        let rows = rustok_search::SearchDiagnosticsService::lagging_documents(
            &app_ctx.db,
            tenant.id,
            normalize_limit(limit, 25, 100),
        )
        .await
        .map_err(map_core_error)?;

        Ok(map_lagging_documents(rows))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = limit;
        Err(ServerFnError::new(
            "search/lagging-documents requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/consistency-issues")]
async fn search_admin_consistency_issues_native(
    limit: Option<i32>,
) -> Result<Vec<SearchConsistencyIssuePayload>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_settings_read_permission(&auth.permissions)?;

        let rows = rustok_search::SearchDiagnosticsService::consistency_issues(
            &app_ctx.db,
            tenant.id,
            normalize_limit(limit, 25, 100),
        )
        .await
        .map_err(map_core_error)?;

        Ok(map_consistency_issues(rows))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = limit;
        Err(ServerFnError::new(
            "search/consistency-issues requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/analytics")]
async fn search_admin_analytics_native(
    days: Option<i32>,
    limit: Option<i32>,
) -> Result<SearchAnalyticsPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_settings_read_permission(&auth.permissions)?;

        let snapshot = rustok_search::SearchAnalyticsService::snapshot(
            &app_ctx.db,
            tenant.id,
            normalize_analytics_days(days),
            normalize_analytics_limit(limit),
        )
        .await
        .map_err(map_core_error)?;

        Ok(map_analytics_payload(snapshot))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (days, limit);
        Err(ServerFnError::new(
            "search/analytics requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/dictionary-snapshot")]
async fn search_admin_dictionary_snapshot_native(
) -> Result<SearchDictionarySnapshotPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_settings_read_permission(&auth.permissions)?;

        let snapshot = rustok_search::SearchDictionaryService::snapshot(&app_ctx.db, tenant.id)
            .await
            .map_err(map_core_error)?;

        Ok(map_dictionary_snapshot(snapshot))
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "search/dictionary-snapshot requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/track-click")]
async fn track_search_click_native(
    query_log_id: String,
    document_id: String,
    position: Option<i32>,
    href: Option<String>,
) -> Result<TrackSearchClickPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::TenantContext;

        let app_ctx = expect_context::<AppContext>();
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        let query_log_id = query_log_id
            .trim()
            .parse::<i64>()
            .map_err(|_| ServerFnError::new("Invalid query_log_id"))?;
        let document_id = parse_required_uuid(&document_id, "document_id")?;

        rustok_search::SearchAnalyticsService::record_click(
            &app_ctx.db,
            rustok_search::SearchClickRecord {
                tenant_id: tenant.id,
                query_log_id,
                document_id,
                position: position.map(|value| value.max(0) as u32),
                href: href.and_then(|value| {
                    let trimmed = value.trim().to_string();
                    (!trimmed.is_empty()).then_some(trimmed)
                }),
            },
        )
        .await
        .map_err(map_core_error)?;

        Ok(TrackSearchClickPayload {
            success: true,
            tracked: true,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (query_log_id, document_id, position, href);
        Err(ServerFnError::new(
            "search/track-click requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/update-settings")]
async fn update_search_settings_native(
    active_engine: String,
    fallback_engine: Option<String>,
    config: String,
) -> Result<SearchSettingsPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::loco::transactional_event_bus_from_context;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_events::DomainEvent;

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_settings_manage_permission(&auth.permissions)?;

        let active_engine = parse_engine(&active_engine, "active_engine")?;
        let fallback_engine = fallback_engine
            .as_deref()
            .map(|value| parse_engine(value, "fallback_engine"))
            .transpose()?
            .unwrap_or(rustok_search::SearchEngineKind::Postgres);
        ensure_engine_available(active_engine)?;
        ensure_engine_available(fallback_engine)?;
        let config: serde_json::Value = serde_json::from_str(&config)
            .map_err(|err| ServerFnError::new(format!("Invalid JSON in config: {err}")))?;

        let settings = rustok_search::SearchSettingsService::save(
            &app_ctx.db,
            Some(tenant.id),
            active_engine,
            fallback_engine,
            config,
        )
        .await
        .map_err(ServerFnError::new)?;

        let event_bus = transactional_event_bus_from_context(&app_ctx);
        let _ = event_bus
            .publish(
                tenant.id,
                Some(auth.user_id),
                DomainEvent::SearchSettingsChanged {
                    active_engine: active_engine.as_str().to_string(),
                    fallback_engine: fallback_engine.as_str().to_string(),
                    changed_by: auth.user_id,
                },
            )
            .await;

        Ok(map_search_settings_payload(settings))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (active_engine, fallback_engine, config);
        Err(ServerFnError::new(
            "search/update-settings requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/trigger-rebuild")]
async fn trigger_search_rebuild_native(
    target_type: Option<String>,
    target_id: Option<String>,
) -> Result<TriggerSearchRebuildPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::loco::transactional_event_bus_from_context;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_events::DomainEvent;

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_settings_manage_permission(&auth.permissions)?;

        let target_type = target_type
            .unwrap_or_else(|| "search".to_string())
            .trim()
            .to_ascii_lowercase();
        if !matches!(target_type.as_str(), "search" | "content" | "product") {
            return Err(ServerFnError::new(
                "Invalid target_type. Expected one of: search, content, product",
            ));
        }

        let parsed_target_id = target_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(|value| parse_required_uuid(value, "target_id"))
            .transpose()?;
        let event_bus = transactional_event_bus_from_context(&app_ctx);
        event_bus
            .publish(
                tenant.id,
                Some(auth.user_id),
                DomainEvent::ReindexRequested {
                    target_type: target_type.clone(),
                    target_id: parsed_target_id,
                },
            )
            .await
            .map_err(ServerFnError::new)?;
        let _ = event_bus
            .publish(
                tenant.id,
                Some(auth.user_id),
                DomainEvent::SearchRebuildQueued {
                    target_type: target_type.clone(),
                    target_id: parsed_target_id,
                    queued_by: auth.user_id,
                },
            )
            .await;

        Ok(TriggerSearchRebuildPayload {
            success: true,
            queued: true,
            tenant_id: tenant.id.to_string(),
            target_type,
            target_id: parsed_target_id.map(|value| value.to_string()),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (target_type, target_id);
        Err(ServerFnError::new(
            "search/trigger-rebuild requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/upsert-synonym")]
async fn upsert_search_synonym_native(
    term: String,
    synonyms: Vec<String>,
) -> Result<SearchDictionaryMutationPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_settings_manage_permission(&auth.permissions)?;

        rustok_search::SearchDictionaryService::upsert_synonym(
            &app_ctx.db,
            tenant.id,
            &term,
            synonyms,
        )
        .await
        .map_err(map_core_error)?;

        Ok(map_dictionary_mutation_payload(true))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (term, synonyms);
        Err(ServerFnError::new(
            "search/upsert-synonym requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/delete-synonym")]
async fn delete_search_synonym_native(
    synonym_id: String,
) -> Result<SearchDictionaryMutationPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_settings_manage_permission(&auth.permissions)?;

        rustok_search::SearchDictionaryService::delete_synonym(
            &app_ctx.db,
            tenant.id,
            parse_required_uuid(&synonym_id, "synonym_id")?,
        )
        .await
        .map_err(map_core_error)?;

        Ok(map_dictionary_mutation_payload(true))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = synonym_id;
        Err(ServerFnError::new(
            "search/delete-synonym requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/add-stop-word")]
async fn add_search_stop_word_native(
    value: String,
) -> Result<SearchDictionaryMutationPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_settings_manage_permission(&auth.permissions)?;

        rustok_search::SearchDictionaryService::add_stop_word(&app_ctx.db, tenant.id, &value)
            .await
            .map_err(map_core_error)?;

        Ok(map_dictionary_mutation_payload(true))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = value;
        Err(ServerFnError::new(
            "search/add-stop-word requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/delete-stop-word")]
async fn delete_search_stop_word_native(
    stop_word_id: String,
) -> Result<SearchDictionaryMutationPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_settings_manage_permission(&auth.permissions)?;

        rustok_search::SearchDictionaryService::delete_stop_word(
            &app_ctx.db,
            tenant.id,
            parse_required_uuid(&stop_word_id, "stop_word_id")?,
        )
        .await
        .map_err(map_core_error)?;

        Ok(map_dictionary_mutation_payload(true))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = stop_word_id;
        Err(ServerFnError::new(
            "search/delete-stop-word requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/upsert-pin-rule")]
async fn upsert_search_pin_rule_native(
    query_text: String,
    document_id: String,
    pinned_position: Option<i32>,
) -> Result<SearchDictionaryMutationPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_settings_manage_permission(&auth.permissions)?;

        rustok_search::SearchDictionaryService::upsert_pin_rule(
            &app_ctx.db,
            tenant.id,
            &query_text,
            parse_required_uuid(&document_id, "document_id")?,
            pinned_position.unwrap_or(1).clamp(1, 50) as u32,
        )
        .await
        .map_err(map_core_error)?;

        Ok(map_dictionary_mutation_payload(true))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (query_text, document_id, pinned_position);
        Err(ServerFnError::new(
            "search/upsert-pin-rule requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "search/delete-query-rule")]
async fn delete_search_query_rule_native(
    query_rule_id: String,
) -> Result<SearchDictionaryMutationPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_settings_manage_permission(&auth.permissions)?;

        rustok_search::SearchDictionaryService::delete_query_rule(
            &app_ctx.db,
            tenant.id,
            parse_required_uuid(&query_rule_id, "query_rule_id")?,
        )
        .await
        .map_err(map_core_error)?;

        Ok(map_dictionary_mutation_payload(true))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = query_rule_id;
        Err(ServerFnError::new(
            "search/delete-query-rule requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
struct ResolvedSearchInput {
    preset_key: Option<String>,
    entity_types: Vec<String>,
    source_modules: Vec<String>,
    statuses: Vec<String>,
    ranking_profile: rustok_search::SearchRankingProfile,
}

#[cfg(feature = "ssr")]
fn normalize_search_preview_input(
    input: SearchPreviewInput,
) -> Result<SearchPreviewInput, ServerFnError> {
    Ok(SearchPreviewInput {
        query: normalize_query(&input.query)?,
        locale: normalize_locale(input.locale.as_deref())?,
        tenant_id: input.tenant_id,
        limit: input.limit,
        offset: input.offset,
        ranking_profile: normalize_ranking_profile(input.ranking_profile)?,
        preset_key: normalize_preset_key(input.preset_key)?,
        entity_types: Some(normalize_filter_values("entity_types", input.entity_types)?),
        source_modules: Some(normalize_filter_values(
            "source_modules",
            input.source_modules,
        )?),
        statuses: Some(normalize_filter_values("statuses", input.statuses)?),
    })
}

#[cfg(feature = "ssr")]
fn normalize_query(value: &str) -> Result<String, ServerFnError> {
    let trimmed = value.trim();
    if trimmed.len() > MAX_SEARCH_QUERY_LEN {
        return Err(ServerFnError::new(format!(
            "Search query exceeds the maximum length of {MAX_SEARCH_QUERY_LEN} characters"
        )));
    }

    if trimmed.chars().any(|ch| ch.is_control()) {
        return Err(ServerFnError::new(
            "Search query contains unsupported control characters",
        ));
    }

    Ok(trimmed.to_string())
}

#[cfg(feature = "ssr")]
fn normalize_locale(value: Option<&str>) -> Result<Option<String>, ServerFnError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    if value.len() > MAX_LOCALE_LEN
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err(ServerFnError::new("Invalid locale format"));
    }

    Ok(Some(value.to_ascii_lowercase()))
}

#[cfg(feature = "ssr")]
fn normalize_filter_values(
    field_name: &str,
    values: Option<Vec<String>>,
) -> Result<Vec<String>, ServerFnError> {
    let values = values.unwrap_or_default();
    if values.len() > MAX_FILTER_VALUES {
        return Err(ServerFnError::new(format!(
            "{field_name} exceeds the maximum size of {MAX_FILTER_VALUES} values"
        )));
    }

    values
        .into_iter()
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            if normalized.is_empty() {
                return Err(ServerFnError::new(format!(
                    "{field_name} contains an empty value"
                )));
            }
            if normalized.len() > MAX_FILTER_VALUE_LEN
                || !normalized
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == ':')
            {
                return Err(ServerFnError::new(format!(
                    "{field_name} contains an invalid value"
                )));
            }
            Ok(normalized)
        })
        .collect()
}

#[cfg(feature = "ssr")]
fn normalize_ranking_profile(value: Option<String>) -> Result<Option<String>, ServerFnError> {
    let Some(value) = value
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    rustok_search::SearchRankingProfile::try_from_str(&value)
        .map(|_| Some(value))
        .ok_or_else(|| ServerFnError::new("Unsupported ranking profile"))
}

#[cfg(feature = "ssr")]
fn normalize_preset_key(value: Option<String>) -> Result<Option<String>, ServerFnError> {
    let Some(value) = value
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    if value.len() > MAX_FILTER_VALUE_LEN
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == ':')
    {
        return Err(ServerFnError::new("Invalid preset key"));
    }

    Ok(Some(value))
}

#[cfg(feature = "ssr")]
fn normalize_surface(value: &str) -> Result<String, ServerFnError> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() || normalized.len() > 64 {
        return Err(ServerFnError::new("Invalid search surface"));
    }
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err(ServerFnError::new("Invalid search surface"));
    }
    Ok(normalized)
}

#[cfg(feature = "ssr")]
fn resolve_preset_and_ranking(
    config: &serde_json::Value,
    surface: &str,
    preset_key: Option<&str>,
    requested_ranking_profile: Option<&str>,
    entity_types: Vec<String>,
    source_modules: Vec<String>,
    statuses: Vec<String>,
) -> Result<ResolvedSearchInput, ServerFnError> {
    let resolved_preset = rustok_search::SearchFilterPresetService::resolve(
        config,
        surface,
        preset_key,
        entity_types,
        source_modules,
        statuses,
    )
    .map_err(map_core_error)?;
    let ranking_profile = rustok_search::SearchRankingProfile::resolve(
        config,
        surface,
        requested_ranking_profile,
        resolved_preset.ranking_profile,
    )
    .map_err(map_core_error)?;

    Ok(ResolvedSearchInput {
        preset_key: resolved_preset.preset.map(|preset| preset.key),
        entity_types: resolved_preset.entity_types,
        source_modules: resolved_preset.source_modules,
        statuses: resolved_preset.statuses,
        ranking_profile,
    })
}

#[cfg(feature = "ssr")]
async fn run_search_with_dictionaries(
    db: &sea_orm::DatabaseConnection,
    engine: &rustok_search::PgSearchEngine,
    search_query: rustok_search::SearchQuery,
) -> rustok_core::Result<rustok_search::SearchResult> {
    let result = rustok_search::SearchEngine::search(engine, search_query.clone()).await?;
    rustok_search::SearchDictionaryService::apply_query_rules(db, &search_query, result).await
}

#[cfg(feature = "ssr")]
fn classify_search_error(error: &rustok_core::Error) -> &'static str {
    match error {
        rustok_core::Error::Database(_) => "database",
        rustok_core::Error::Validation(_) => "validation",
        rustok_core::Error::External(_) => "external",
        rustok_core::Error::NotFound(_) => "not_found",
        rustok_core::Error::Forbidden(_) => "forbidden",
        rustok_core::Error::Auth(_) => "auth",
        rustok_core::Error::Cache(_) => "cache",
        rustok_core::Error::Serialization(_) => "serialization",
        rustok_core::Error::Scripting(_) => "scripting",
        rustok_core::Error::InvalidIdFormat(_) => "invalid_id",
    }
}

#[cfg(feature = "ssr")]
async fn record_search_query_log(
    db: &sea_orm::DatabaseConnection,
    surface: &str,
    search_query: &rustok_search::SearchQuery,
    engine: &str,
    result_count: u64,
    took_ms: u64,
    status: &str,
) -> Option<i64> {
    let tenant_id = search_query.tenant_id?;
    let engine_kind = rustok_search::SearchEngineKind::try_from_str(engine)?;

    rustok_search::SearchAnalyticsService::record_query(
        db,
        rustok_search::SearchQueryLogRecord {
            tenant_id,
            surface: surface.to_string(),
            query: search_query.original_query.clone(),
            locale: search_query.locale.clone(),
            engine: engine_kind,
            result_count,
            took_ms,
            status: status.to_string(),
            entity_types: search_query.entity_types.clone(),
            source_modules: search_query.source_modules.clone(),
            statuses: search_query.statuses.clone(),
        },
    )
    .await
    .ok()
    .flatten()
}

#[cfg(feature = "ssr")]
async fn finalize_search_result(
    db: &sea_orm::DatabaseConnection,
    surface: &str,
    search_query: &rustok_search::SearchQuery,
    started_at: std::time::Instant,
    result: rustok_core::Result<rustok_search::SearchResult>,
) -> Result<SearchPreviewPayload, ServerFnError> {
    match result {
        Ok(result) => {
            let query_log_id = record_search_query_log(
                db,
                surface,
                search_query,
                result.engine.as_str(),
                result.total,
                result.took_ms,
                "success",
            )
            .await;
            Ok(map_search_preview_payload(
                result,
                search_query.preset_key.clone(),
                query_log_id,
            ))
        }
        Err(error) => {
            let _ = record_search_query_log(
                db,
                surface,
                search_query,
                "postgres",
                0,
                started_at.elapsed().as_millis() as u64,
                classify_search_error(&error),
            )
            .await;
            Err(map_core_error(error))
        }
    }
}

#[cfg(feature = "ssr")]
fn map_search_engine_descriptor(
    value: rustok_search::SearchConnectorDescriptor,
) -> crate::model::SearchEngineDescriptor {
    crate::model::SearchEngineDescriptor {
        kind: value.kind.as_str().to_string(),
        label: value.label,
        provided_by: value.provided_by,
        enabled: value.enabled,
        default_engine: value.default_engine,
    }
}

#[cfg(feature = "ssr")]
fn map_search_settings_payload(
    value: rustok_search::SearchSettingsRecord,
) -> SearchSettingsPayload {
    SearchSettingsPayload {
        tenant_id: value.tenant_id.map(|tenant_id| tenant_id.to_string()),
        active_engine: value.active_engine.as_str().to_string(),
        fallback_engine: value.fallback_engine.as_str().to_string(),
        config: value.config.to_string(),
        updated_at: value.updated_at.to_rfc3339(),
    }
}

#[cfg(feature = "ssr")]
fn map_search_preview_payload(
    value: rustok_search::SearchResult,
    preset_key: Option<String>,
    query_log_id: Option<i64>,
) -> SearchPreviewPayload {
    SearchPreviewPayload {
        query_log_id: query_log_id.map(|value| value.to_string()),
        preset_key,
        items: value
            .items
            .into_iter()
            .map(|item| {
                let url = derive_search_result_url(&item);
                crate::model::SearchPreviewResultItem {
                    id: item.id.to_string(),
                    entity_type: item.entity_type,
                    source_module: item.source_module,
                    title: item.title,
                    snippet: item.snippet,
                    score: item.score,
                    locale: item.locale,
                    url,
                    payload: item.payload.to_string(),
                }
            })
            .collect(),
        total: value.total,
        took_ms: value.took_ms,
        engine: value.engine.as_str().to_string(),
        ranking_profile: value.ranking_profile.as_str().to_string(),
        facets: value
            .facets
            .into_iter()
            .map(|facet| crate::model::SearchFacetGroup {
                name: facet.name,
                buckets: facet
                    .buckets
                    .into_iter()
                    .map(|bucket| crate::model::SearchFacetBucket {
                        value: bucket.value,
                        count: bucket.count,
                    })
                    .collect(),
            })
            .collect(),
    }
}

#[cfg(feature = "ssr")]
fn map_diagnostics_payload(
    value: rustok_search::SearchDiagnosticsSnapshot,
) -> crate::model::SearchDiagnosticsPayload {
    crate::model::SearchDiagnosticsPayload {
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

#[cfg(feature = "ssr")]
fn map_lagging_documents(
    rows: Vec<rustok_search::LaggingSearchDocument>,
) -> Vec<LaggingSearchDocumentPayload> {
    rows.into_iter()
        .map(|value| LaggingSearchDocumentPayload {
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
        })
        .collect()
}

#[cfg(feature = "ssr")]
fn map_consistency_issues(
    rows: Vec<rustok_search::SearchConsistencyIssue>,
) -> Vec<SearchConsistencyIssuePayload> {
    rows.into_iter()
        .map(|value| SearchConsistencyIssuePayload {
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
        })
        .collect()
}

#[cfg(feature = "ssr")]
fn map_analytics_payload(value: rustok_search::SearchAnalyticsSnapshot) -> SearchAnalyticsPayload {
    SearchAnalyticsPayload {
        summary: crate::model::SearchAnalyticsSummaryPayload {
            window_days: value.summary.window_days,
            total_queries: value.summary.total_queries,
            successful_queries: value.summary.successful_queries,
            zero_result_queries: value.summary.zero_result_queries,
            zero_result_rate: value.summary.zero_result_rate,
            slow_queries: value.summary.slow_queries,
            slow_query_rate: value.summary.slow_query_rate,
            avg_took_ms: value.summary.avg_took_ms,
            avg_results_per_query: value.summary.avg_results_per_query,
            unique_queries: value.summary.unique_queries,
            clicked_queries: value.summary.clicked_queries,
            total_clicks: value.summary.total_clicks,
            click_through_rate: value.summary.click_through_rate,
            abandonment_queries: value.summary.abandonment_queries,
            abandonment_rate: value.summary.abandonment_rate,
            last_query_at: value.summary.last_query_at.map(|value| value.to_rfc3339()),
        },
        top_queries: map_analytics_rows(value.top_queries),
        zero_result_queries: map_analytics_rows(value.zero_result_queries),
        slow_queries: map_analytics_rows(value.slow_queries),
        low_ctr_queries: map_analytics_rows(value.low_ctr_queries),
        abandonment_queries: map_analytics_rows(value.abandonment_queries),
        intelligence_candidates: value
            .intelligence_candidates
            .into_iter()
            .map(|value| crate::model::SearchAnalyticsInsightRowPayload {
                query: value.query,
                hits: value.hits,
                zero_result_hits: value.zero_result_hits,
                clicks: value.clicks,
                click_through_rate: value.click_through_rate,
                abandonment_rate: value.abandonment_rate,
                recommendation: value.recommendation,
            })
            .collect(),
    }
}

#[cfg(feature = "ssr")]
fn map_analytics_rows(
    rows: Vec<rustok_search::SearchAnalyticsQueryRow>,
) -> Vec<crate::model::SearchAnalyticsQueryRowPayload> {
    rows.into_iter()
        .map(|value| crate::model::SearchAnalyticsQueryRowPayload {
            query: value.query,
            hits: value.hits,
            zero_result_hits: value.zero_result_hits,
            clicks: value.clicks,
            avg_took_ms: value.avg_took_ms,
            avg_results: value.avg_results,
            click_through_rate: value.click_through_rate,
            abandonment_rate: value.abandonment_rate,
            last_seen_at: value.last_seen_at.to_rfc3339(),
        })
        .collect()
}

#[cfg(feature = "ssr")]
fn map_dictionary_snapshot(
    value: rustok_search::SearchDictionarySnapshot,
) -> SearchDictionarySnapshotPayload {
    SearchDictionarySnapshotPayload {
        synonyms: value
            .synonyms
            .into_iter()
            .map(|value| crate::model::SearchSynonymPayload {
                id: value.id.to_string(),
                term: value.term,
                synonyms: value.synonyms,
                updated_at: value.updated_at.to_rfc3339(),
            })
            .collect(),
        stop_words: value
            .stop_words
            .into_iter()
            .map(|value| crate::model::SearchStopWordPayload {
                id: value.id.to_string(),
                value: value.value,
                updated_at: value.updated_at.to_rfc3339(),
            })
            .collect(),
        query_rules: value
            .query_rules
            .into_iter()
            .map(|value| crate::model::SearchQueryRulePayload {
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
            })
            .collect(),
    }
}

#[cfg(feature = "ssr")]
fn map_dictionary_mutation_payload(success: bool) -> SearchDictionaryMutationPayload {
    SearchDictionaryMutationPayload { success }
}

#[cfg(feature = "ssr")]
fn derive_search_result_url(value: &rustok_search::SearchResultItem) -> Option<String> {
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

#[cfg(feature = "ssr")]
fn map_core_error(error: rustok_core::Error) -> ServerFnError {
    ServerFnError::new(error.to_string())
}

#[cfg(feature = "ssr")]
fn normalize_analytics_days(value: Option<i32>) -> u32 {
    value.unwrap_or(7).clamp(1, 30) as u32
}

#[cfg(feature = "ssr")]
fn normalize_analytics_limit(value: Option<i32>) -> usize {
    value.unwrap_or(10).clamp(1, 25) as usize
}

#[cfg(feature = "ssr")]
fn normalize_limit(value: Option<i32>, default: i32, max: i32) -> usize {
    value.unwrap_or(default).clamp(1, max) as usize
}

#[cfg(feature = "ssr")]
fn ensure_settings_read_permission(
    permissions: &[rustok_core::Permission],
) -> Result<(), ServerFnError> {
    if !rustok_api::has_effective_permission(permissions, &rustok_core::Permission::SETTINGS_READ) {
        return Err(ServerFnError::new("settings:read required"));
    }
    Ok(())
}

#[cfg(feature = "ssr")]
fn ensure_settings_manage_permission(
    permissions: &[rustok_core::Permission],
) -> Result<(), ServerFnError> {
    if !rustok_api::has_effective_permission(permissions, &rustok_core::Permission::SETTINGS_MANAGE)
    {
        return Err(ServerFnError::new("settings:manage required"));
    }
    Ok(())
}

#[cfg(feature = "ssr")]
fn parse_required_uuid(value: &str, field_name: &str) -> Result<uuid::Uuid, ServerFnError> {
    uuid::Uuid::parse_str(value.trim())
        .map_err(|_| ServerFnError::new(format!("Invalid {field_name}")))
}

#[cfg(feature = "ssr")]
fn parse_engine(
    value: &str,
    field_name: &str,
) -> Result<rustok_search::SearchEngineKind, ServerFnError> {
    rustok_search::SearchEngineKind::try_from_str(value)
        .ok_or_else(|| ServerFnError::new(format!("Invalid {field_name}: unsupported engine")))
}

#[cfg(feature = "ssr")]
fn ensure_engine_available(engine: rustok_search::SearchEngineKind) -> Result<(), ServerFnError> {
    let module = rustok_search::SearchModule;
    if module
        .available_engines()
        .into_iter()
        .any(|descriptor| descriptor.enabled && descriptor.kind == engine)
    {
        Ok(())
    } else {
        Err(ServerFnError::new(format!(
            "Engine '{}' is not installed in the current runtime",
            engine.as_str()
        )))
    }
}
