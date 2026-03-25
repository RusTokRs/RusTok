use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};

use crate::model::{
    LaggingSearchDocumentPayload, SearchAdminBootstrap, SearchAnalyticsPayload,
    SearchDictionaryMutationPayload, SearchDictionarySnapshotPayload, SearchPreviewFilters,
    SearchPreviewPayload, SearchSettingsPayload, TrackSearchClickPayload,
    TriggerSearchRebuildPayload,
};

pub type ApiError = GraphqlHttpError;

const SEARCH_ADMIN_BOOTSTRAP_QUERY: &str = "query SearchAdminBootstrap { availableSearchEngines { kind label providedBy enabled defaultEngine } searchSettingsPreview { tenantId activeEngine fallbackEngine config updatedAt } searchDiagnostics { tenantId totalDocuments publicDocuments contentDocuments productDocuments staleDocuments newestIndexedAt oldestIndexedAt maxLagSeconds state } }";
const SEARCH_PREVIEW_QUERY: &str = "query SearchPreview($input: SearchPreviewInput!) { searchPreview(input: $input) { queryLogId total tookMs engine items { id entityType sourceModule title snippet score locale url payload } facets { name buckets { value count } } } }";
const SEARCH_LAGGING_DOCUMENTS_QUERY: &str = "query SearchLaggingDocuments($limit: Int) { searchLaggingDocuments(limit: $limit) { documentKey documentId sourceModule entityType locale status isPublic title updatedAt indexedAt lagSeconds } }";
const SEARCH_ANALYTICS_QUERY: &str = "query SearchAnalytics($days: Int, $limit: Int) { searchAnalytics(days: $days, limit: $limit) { summary { windowDays totalQueries successfulQueries zeroResultQueries zeroResultRate avgTookMs avgResultsPerQuery uniqueQueries clickedQueries totalClicks clickThroughRate abandonmentQueries abandonmentRate lastQueryAt } topQueries { query hits zeroResultHits clicks avgTookMs avgResults clickThroughRate abandonmentRate lastSeenAt } zeroResultQueries { query hits zeroResultHits clicks avgTookMs avgResults clickThroughRate abandonmentRate lastSeenAt } lowCtrQueries { query hits zeroResultHits clicks avgTookMs avgResults clickThroughRate abandonmentRate lastSeenAt } abandonmentQueries { query hits zeroResultHits clicks avgTookMs avgResults clickThroughRate abandonmentRate lastSeenAt } intelligenceCandidates { query hits zeroResultHits clicks clickThroughRate abandonmentRate recommendation } } }";
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
struct SearchAnalyticsResponse {
    #[serde(rename = "searchAnalytics")]
    search_analytics: SearchAnalyticsPayload,
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
}

pub async fn fetch_bootstrap(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<SearchAdminBootstrap, ApiError> {
    request::<serde_json::Value, SearchAdminBootstrap>(
        SEARCH_ADMIN_BOOTSTRAP_QUERY,
        None,
        token,
        tenant_slug,
    )
    .await
}

pub async fn fetch_search_preview(
    token: Option<String>,
    tenant_slug: Option<String>,
    query: String,
    locale: Option<String>,
    filters: SearchPreviewFilters,
) -> Result<SearchPreviewPayload, ApiError> {
    let response: SearchPreviewResponse = request(
        SEARCH_PREVIEW_QUERY,
        Some(SearchPreviewVariables {
            input: SearchPreviewInput {
                query,
                locale,
                tenant_id: None,
                limit: Some(12),
                offset: Some(0),
                entity_types: (!filters.entity_types.is_empty()).then_some(filters.entity_types),
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

pub async fn trigger_search_rebuild(
    token: Option<String>,
    tenant_slug: Option<String>,
    target_type: Option<String>,
    target_id: Option<String>,
) -> Result<TriggerSearchRebuildPayload, ApiError> {
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

pub async fn fetch_lagging_documents(
    token: Option<String>,
    tenant_slug: Option<String>,
    limit: Option<i32>,
) -> Result<Vec<LaggingSearchDocumentPayload>, ApiError> {
    let response: SearchLaggingDocumentsResponse = request(
        SEARCH_LAGGING_DOCUMENTS_QUERY,
        Some(SearchLaggingDocumentsVariables { limit }),
        token,
        tenant_slug,
    )
    .await?;

    Ok(response.search_lagging_documents)
}

pub async fn fetch_search_analytics(
    token: Option<String>,
    tenant_slug: Option<String>,
    days: Option<i32>,
    limit: Option<i32>,
) -> Result<SearchAnalyticsPayload, ApiError> {
    let response: SearchAnalyticsResponse = request(
        SEARCH_ANALYTICS_QUERY,
        Some(SearchAnalyticsVariables { days, limit }),
        token,
        tenant_slug,
    )
    .await?;

    Ok(response.search_analytics)
}

pub async fn fetch_dictionary_snapshot(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<SearchDictionarySnapshotPayload, ApiError> {
    let response: SearchDictionarySnapshotResponse =
        request::<serde_json::Value, _>(SEARCH_DICTIONARY_SNAPSHOT_QUERY, None, token, tenant_slug)
            .await?;

    Ok(response.search_dictionary_snapshot)
}

pub async fn track_search_click(
    token: Option<String>,
    tenant_slug: Option<String>,
    query_log_id: String,
    document_id: String,
    position: Option<i32>,
    href: Option<String>,
) -> Result<TrackSearchClickPayload, ApiError> {
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

pub async fn update_search_settings(
    token: Option<String>,
    tenant_slug: Option<String>,
    active_engine: String,
    fallback_engine: Option<String>,
    config: String,
) -> Result<SearchSettingsPayload, ApiError> {
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

pub async fn upsert_search_synonym(
    token: Option<String>,
    tenant_slug: Option<String>,
    term: String,
    synonyms: Vec<String>,
) -> Result<SearchDictionaryMutationPayload, ApiError> {
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

pub async fn delete_search_synonym(
    token: Option<String>,
    tenant_slug: Option<String>,
    synonym_id: String,
) -> Result<SearchDictionaryMutationPayload, ApiError> {
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

pub async fn add_search_stop_word(
    token: Option<String>,
    tenant_slug: Option<String>,
    value: String,
) -> Result<SearchDictionaryMutationPayload, ApiError> {
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

pub async fn delete_search_stop_word(
    token: Option<String>,
    tenant_slug: Option<String>,
    stop_word_id: String,
) -> Result<SearchDictionaryMutationPayload, ApiError> {
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

pub async fn upsert_search_pin_rule(
    token: Option<String>,
    tenant_slug: Option<String>,
    query_text: String,
    document_id: String,
    pinned_position: Option<i32>,
) -> Result<SearchDictionaryMutationPayload, ApiError> {
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

pub async fn delete_search_query_rule(
    token: Option<String>,
    tenant_slug: Option<String>,
    query_rule_id: String,
) -> Result<SearchDictionaryMutationPayload, ApiError> {
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
