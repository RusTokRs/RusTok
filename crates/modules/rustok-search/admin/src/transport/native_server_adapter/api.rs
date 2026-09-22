use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[cfg(feature = "ssr")]
use crate::model::SearchAttributeFilter;
use crate::model::{
    LaggingSearchDocumentPayload, SearchAdminBootstrap, SearchAnalyticsPayload,
    SearchConsistencyIssuePayload, SearchDictionaryMutationPayload,
    SearchDictionarySnapshotPayload, SearchFilterPresetPayload, SearchPreviewFilters,
    SearchPreviewPayload, SearchSettingsPayload, TrackSearchClickPayload,
    TriggerSearchRebuildPayload,
};
#[cfg(feature = "ssr")]
use rustok_api::HostRuntimeContext;

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
const MAX_ATTRIBUTE_FILTERS: usize = 10;
#[cfg(feature = "ssr")]
const MAX_LOCALE_LEN: usize = 16;

#[cfg(feature = "ssr")]
struct SearchAdminRuntime {
    db: sea_orm::DatabaseConnection,
    host: HostRuntimeContext,
}

#[cfg(feature = "ssr")]
impl SearchAdminRuntime {
    fn from_host(host: HostRuntimeContext) -> Self {
        Self {
            db: host.db_clone(),
            host,
        }
    }

    fn transactional_event_bus(
        &self,
    ) -> Result<rustok_outbox::TransactionalEventBus, ServerFnError> {
        self.host
            .shared_get::<rustok_outbox::TransactionalEventBus>()
            .ok_or_else(|| {
                ServerFnError::new(
                    "Search admin requires TransactionalEventBus in host runtime context",
                )
            })
    }
}

#[cfg(feature = "ssr")]
#[derive(Debug, Serialize)]
struct SearchPreviewInput {
    query: String,
    locale: Option<String>,
    #[serde(rename = "channelId")]
    channel_id: Option<String>,
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
    #[serde(rename = "categoryIds")]
    category_ids: Option<Vec<String>>,
    #[serde(rename = "attributeFilters")]
    attribute_filters: Option<Vec<SearchAttributeFilterInput>>,
    #[serde(rename = "sortAttributeCode")]
    sort_attribute_code: Option<String>,
    #[serde(rename = "sortDesc")]
    sort_desc: Option<bool>,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Serialize)]
struct SearchAttributeFilterInput {
    #[serde(rename = "attributeCode")]
    attribute_code: String,
    values: Option<Vec<String>>,
    min: Option<String>,
    max: Option<String>,
}

#[cfg(feature = "ssr")]
fn search_attribute_filter_inputs(
    filters: Vec<SearchAttributeFilter>,
) -> Vec<SearchAttributeFilterInput> {
    filters
        .into_iter()
        .map(|filter| SearchAttributeFilterInput {
            attribute_code: filter.attribute_code,
            values: (!filter.values.is_empty()).then_some(filter.values),
            min: filter.min,
            max: filter.max,
        })
        .collect()
}
pub async fn fetch_bootstrap(
    _token: Option<String>,
    _tenant_slug: Option<String>,
) -> Result<SearchAdminBootstrap, ApiError> {
    search_admin_bootstrap_native()
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_search_preview(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    query: String,
    locale: Option<String>,
    ranking_profile: Option<String>,
    preset_key: Option<String>,
    filters: SearchPreviewFilters,
) -> Result<SearchPreviewPayload, ApiError> {
    search_admin_preview_native(query, locale, ranking_profile, preset_key, filters)
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_filter_presets(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    surface: &str,
) -> Result<Vec<SearchFilterPresetPayload>, ApiError> {
    search_admin_filter_presets_native(surface.to_string())
        .await
        .map_err(ApiError::from)
}

pub async fn trigger_search_rebuild(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    target_type: Option<String>,
    target_id: Option<String>,
) -> Result<TriggerSearchRebuildPayload, ApiError> {
    trigger_search_rebuild_native(target_type, target_id)
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_lagging_documents(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    limit: Option<i32>,
) -> Result<Vec<LaggingSearchDocumentPayload>, ApiError> {
    search_admin_lagging_documents_native(limit)
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_consistency_issues(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    limit: Option<i32>,
) -> Result<Vec<SearchConsistencyIssuePayload>, ApiError> {
    search_admin_consistency_issues_native(limit)
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_search_analytics(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    days: Option<i32>,
    limit: Option<i32>,
) -> Result<SearchAnalyticsPayload, ApiError> {
    search_admin_analytics_native(days, limit)
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_dictionary_snapshot(
    _token: Option<String>,
    _tenant_slug: Option<String>,
) -> Result<SearchDictionarySnapshotPayload, ApiError> {
    search_admin_dictionary_snapshot_native()
        .await
        .map_err(ApiError::from)
}

pub async fn track_search_click(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    query_log_id: String,
    document_id: String,
    position: Option<i32>,
    href: Option<String>,
) -> Result<TrackSearchClickPayload, ApiError> {
    track_search_click_native(query_log_id, document_id, position, href)
        .await
        .map_err(ApiError::from)
}

pub async fn update_search_settings(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    active_engine: String,
    fallback_engine: Option<String>,
    config: String,
) -> Result<SearchSettingsPayload, ApiError> {
    update_search_settings_native(active_engine, fallback_engine, config)
        .await
        .map_err(ApiError::from)
}

pub async fn upsert_search_synonym(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    term: String,
    synonyms: Vec<String>,
) -> Result<SearchDictionaryMutationPayload, ApiError> {
    upsert_search_synonym_native(term, synonyms)
        .await
        .map_err(ApiError::from)
}

pub async fn delete_search_synonym(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    synonym_id: String,
) -> Result<SearchDictionaryMutationPayload, ApiError> {
    delete_search_synonym_native(synonym_id)
        .await
        .map_err(ApiError::from)
}

pub async fn add_search_stop_word(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    value: String,
) -> Result<SearchDictionaryMutationPayload, ApiError> {
    add_search_stop_word_native(value)
        .await
        .map_err(ApiError::from)
}

pub async fn delete_search_stop_word(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    stop_word_id: String,
) -> Result<SearchDictionaryMutationPayload, ApiError> {
    delete_search_stop_word_native(stop_word_id)
        .await
        .map_err(ApiError::from)
}

pub async fn upsert_search_pin_rule(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    query_text: String,
    document_id: String,
    pinned_position: Option<i32>,
) -> Result<SearchDictionaryMutationPayload, ApiError> {
    upsert_search_pin_rule_native(query_text, document_id, pinned_position)
        .await
        .map_err(ApiError::from)
}

pub async fn delete_search_query_rule(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    query_rule_id: String,
) -> Result<SearchDictionaryMutationPayload, ApiError> {
    delete_search_query_rule_native(query_rule_id)
        .await
        .map_err(ApiError::from)
}
