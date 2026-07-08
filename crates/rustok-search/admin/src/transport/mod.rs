mod graphql_adapter;
mod native_server_adapter;

use rustok_ui_transport::UiTransportPath;

use crate::model::{
    LaggingSearchDocumentPayload, SearchAdminBootstrap, SearchAnalyticsPayload,
    SearchConsistencyIssuePayload, SearchDictionaryMutationPayload,
    SearchDictionarySnapshotPayload, SearchFilterPresetPayload, SearchPreviewFilters,
    SearchPreviewPayload, SearchSettingsPayload, TrackSearchClickPayload,
    TriggerSearchRebuildPayload,
};

pub type TransportError = native_server_adapter::ApiError;

fn selected_transport_path() -> UiTransportPath {
    if cfg!(all(target_arch = "wasm32", not(feature = "hydrate"))) {
        UiTransportPath::Graphql
    } else {
        UiTransportPath::NativeServer
    }
}

pub async fn fetch_bootstrap(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<SearchAdminBootstrap, TransportError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            native_server_adapter::fetch_bootstrap(token, tenant_slug).await
        }
        UiTransportPath::Graphql => graphql_adapter::fetch_bootstrap(token, tenant_slug).await,
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
) -> Result<SearchPreviewPayload, TransportError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            native_server_adapter::fetch_search_preview(
                token,
                tenant_slug,
                query,
                locale,
                ranking_profile,
                preset_key,
                filters,
            )
            .await
        }
        UiTransportPath::Graphql => {
            graphql_adapter::fetch_search_preview(
                token,
                tenant_slug,
                query,
                locale,
                ranking_profile,
                preset_key,
                filters,
            )
            .await
        }
    }
}

pub async fn fetch_filter_presets(
    token: Option<String>,
    tenant_slug: Option<String>,
    surface: &str,
) -> Result<Vec<SearchFilterPresetPayload>, TransportError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            native_server_adapter::fetch_filter_presets(token, tenant_slug, surface).await
        }
        UiTransportPath::Graphql => {
            graphql_adapter::fetch_filter_presets(token, tenant_slug, surface).await
        }
    }
}

pub async fn trigger_search_rebuild(
    token: Option<String>,
    tenant_slug: Option<String>,
    target_type: Option<String>,
    target_id: Option<String>,
) -> Result<TriggerSearchRebuildPayload, TransportError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            native_server_adapter::trigger_search_rebuild(
                token,
                tenant_slug,
                target_type,
                target_id,
            )
            .await
        }
        UiTransportPath::Graphql => {
            graphql_adapter::trigger_search_rebuild(token, tenant_slug, target_type, target_id)
                .await
        }
    }
}

pub async fn fetch_lagging_documents(
    token: Option<String>,
    tenant_slug: Option<String>,
    limit: Option<i32>,
) -> Result<Vec<LaggingSearchDocumentPayload>, TransportError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            native_server_adapter::fetch_lagging_documents(token, tenant_slug, limit).await
        }
        UiTransportPath::Graphql => {
            graphql_adapter::fetch_lagging_documents(token, tenant_slug, limit).await
        }
    }
}

pub async fn fetch_consistency_issues(
    token: Option<String>,
    tenant_slug: Option<String>,
    limit: Option<i32>,
) -> Result<Vec<SearchConsistencyIssuePayload>, TransportError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            native_server_adapter::fetch_consistency_issues(token, tenant_slug, limit).await
        }
        UiTransportPath::Graphql => {
            graphql_adapter::fetch_consistency_issues(token, tenant_slug, limit).await
        }
    }
}

pub async fn fetch_search_analytics(
    token: Option<String>,
    tenant_slug: Option<String>,
    days: Option<i32>,
    limit: Option<i32>,
) -> Result<SearchAnalyticsPayload, TransportError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            native_server_adapter::fetch_search_analytics(token, tenant_slug, days, limit).await
        }
        UiTransportPath::Graphql => {
            graphql_adapter::fetch_search_analytics(token, tenant_slug, days, limit).await
        }
    }
}

pub async fn fetch_dictionary_snapshot(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<SearchDictionarySnapshotPayload, TransportError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            native_server_adapter::fetch_dictionary_snapshot(token, tenant_slug).await
        }
        UiTransportPath::Graphql => {
            graphql_adapter::fetch_dictionary_snapshot(token, tenant_slug).await
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
) -> Result<TrackSearchClickPayload, TransportError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            native_server_adapter::track_search_click(
                token,
                tenant_slug,
                query_log_id,
                document_id,
                position,
                href,
            )
            .await
        }
        UiTransportPath::Graphql => {
            graphql_adapter::track_search_click(
                token,
                tenant_slug,
                query_log_id,
                document_id,
                position,
                href,
            )
            .await
        }
    }
}

pub async fn update_search_settings(
    token: Option<String>,
    tenant_slug: Option<String>,
    active_engine: String,
    fallback_engine: Option<String>,
    config: String,
) -> Result<SearchSettingsPayload, TransportError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            native_server_adapter::update_search_settings(
                token,
                tenant_slug,
                active_engine,
                fallback_engine,
                config,
            )
            .await
        }
        UiTransportPath::Graphql => {
            graphql_adapter::update_search_settings(
                token,
                tenant_slug,
                active_engine,
                fallback_engine,
                config,
            )
            .await
        }
    }
}

pub async fn upsert_search_synonym(
    token: Option<String>,
    tenant_slug: Option<String>,
    term: String,
    synonyms: Vec<String>,
) -> Result<SearchDictionaryMutationPayload, TransportError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            native_server_adapter::upsert_search_synonym(token, tenant_slug, term, synonyms).await
        }
        UiTransportPath::Graphql => {
            graphql_adapter::upsert_search_synonym(token, tenant_slug, term, synonyms).await
        }
    }
}

pub async fn delete_search_synonym(
    token: Option<String>,
    tenant_slug: Option<String>,
    synonym_id: String,
) -> Result<SearchDictionaryMutationPayload, TransportError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            native_server_adapter::delete_search_synonym(token, tenant_slug, synonym_id).await
        }
        UiTransportPath::Graphql => {
            graphql_adapter::delete_search_synonym(token, tenant_slug, synonym_id).await
        }
    }
}

pub async fn add_search_stop_word(
    token: Option<String>,
    tenant_slug: Option<String>,
    value: String,
) -> Result<SearchDictionaryMutationPayload, TransportError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            native_server_adapter::add_search_stop_word(token, tenant_slug, value).await
        }
        UiTransportPath::Graphql => {
            graphql_adapter::add_search_stop_word(token, tenant_slug, value).await
        }
    }
}

pub async fn delete_search_stop_word(
    token: Option<String>,
    tenant_slug: Option<String>,
    stop_word_id: String,
) -> Result<SearchDictionaryMutationPayload, TransportError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            native_server_adapter::delete_search_stop_word(token, tenant_slug, stop_word_id).await
        }
        UiTransportPath::Graphql => {
            graphql_adapter::delete_search_stop_word(token, tenant_slug, stop_word_id).await
        }
    }
}

pub async fn upsert_search_pin_rule(
    token: Option<String>,
    tenant_slug: Option<String>,
    query_text: String,
    document_id: String,
    pinned_position: Option<i32>,
) -> Result<SearchDictionaryMutationPayload, TransportError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            native_server_adapter::upsert_search_pin_rule(
                token,
                tenant_slug,
                query_text,
                document_id,
                pinned_position,
            )
            .await
        }
        UiTransportPath::Graphql => {
            graphql_adapter::upsert_search_pin_rule(
                token,
                tenant_slug,
                query_text,
                document_id,
                pinned_position,
            )
            .await
        }
    }
}

pub async fn delete_search_query_rule(
    token: Option<String>,
    tenant_slug: Option<String>,
    query_rule_id: String,
) -> Result<SearchDictionaryMutationPayload, TransportError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            native_server_adapter::delete_search_query_rule(token, tenant_slug, query_rule_id).await
        }
        UiTransportPath::Graphql => {
            graphql_adapter::delete_search_query_rule(token, tenant_slug, query_rule_id).await
        }
    }
}
