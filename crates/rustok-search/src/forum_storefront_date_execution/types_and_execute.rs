use std::collections::{BTreeMap, HashSet};
use std::time::Instant;

use chrono::{DateTime, Utc};
use rustok_api::{AuthContext, RequestContext};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::engine::{SearchFacetBucket, SearchFacetGroup};
use crate::{
    FORUM_SEARCH_SOURCE_MODULE, ForumStorefrontDocumentFilters, ForumStorefrontLocaleDateFilters,
    ForumStorefrontSearchExecution, ForumStorefrontSearchExecutionError,
    ForumStorefrontSearchRequest, MAX_FORUM_SEARCH_RESULT_CANDIDATES, PgSearchEngine,
    SearchAnalyticsService, SearchAttributeFilter, SearchDictionaryService, SearchEngine,
    SearchFilterPresetService, SearchQuery, SearchQueryLogRecord, SearchRankingProfile,
    SearchResult, SearchResultItem, SearchSettingsService, SharedStorefrontSearchCategoryScopePort,
    SharedStorefrontSearchResultEligibilityPort, StorefrontSearchCategoryScopeRequest,
    StorefrontSearchResultCandidate, StorefrontSearchResultCandidateKind,
    StorefrontSearchResultEligibilityRequest, StorefrontSearchTransport, TrustedStorefrontChannel,
    resolve_storefront_search_category_ids, resolve_storefront_search_result_candidates,
    resolve_trusted_storefront_channel,
};

const STOREFRONT_SEARCH_SURFACE: &str = "storefront_search";
const MAX_SEARCH_QUERY_LEN: usize = 256;
const MAX_FILTER_VALUES: usize = 10;
const MAX_FILTER_VALUE_LEN: usize = 64;
const MAX_ATTRIBUTE_FILTERS: usize = 10;
const MAX_LOCALE_LEN: usize = 16;
const FORUM_RESULT_SCAN_PAGE_SIZE: usize = 50;

#[derive(Clone)]
pub struct ForumStorefrontSearchDateWindowRequest {
    pub request: ForumStorefrontSearchRequest,
    pub published_from: Option<String>,
    pub published_to: Option<String>,
}

struct NormalizedForumStorefrontDateWindowRequest {
    tenant_id: Uuid,
    query: String,
    locale: Option<String>,
    fallback_locale: String,
    channel_id: Option<Uuid>,
    limit: usize,
    offset: usize,
    ranking_profile: Option<String>,
    preset_key: Option<String>,
    entity_types: Vec<String>,
    source_modules: Vec<String>,
    statuses: Vec<String>,
    category_ids: Vec<Uuid>,
    document_filters: ForumStorefrontDocumentFilters,
    locale_date_filters: ForumStorefrontLocaleDateFilters,
    attribute_filters: Vec<SearchAttributeFilter>,
    sort_attribute_code: Option<String>,
    sort_desc: bool,
    auth: Option<AuthContext>,
    request_context: Option<RequestContext>,
    trusted_channel: TrustedStorefrontChannel,
    transport: StorefrontSearchTransport,
}

pub async fn execute_forum_storefront_search_with_date_window(
    db: &DatabaseConnection,
    category_scope_port: Option<SharedStorefrontSearchCategoryScopePort>,
    result_eligibility_port: Option<SharedStorefrontSearchResultEligibilityPort>,
    request: ForumStorefrontSearchDateWindowRequest,
) -> Result<ForumStorefrontSearchExecution, ForumStorefrontSearchExecutionError> {
    let input = normalize_date_window_request(request)?;
    let trusted_channel = input.trusted_channel.clone();
    let document_filters = input.document_filters.clone();
    let locale_date_filters = input.locale_date_filters.clone();
    let started_at = Instant::now();
    let transform =
        SearchDictionaryService::transform_query(db, input.tenant_id, &input.query).await?;
    let settings = SearchSettingsService::load_effective(db, Some(input.tenant_id)).await?;
    let resolved_preset = SearchFilterPresetService::resolve(
        &settings.config,
        STOREFRONT_SEARCH_SURFACE,
        input.preset_key.as_deref(),
        input.entity_types,
        input.source_modules,
        input.statuses,
    )?;
    if resolved_preset.source_modules.as_slice() != [FORUM_SEARCH_SOURCE_MODULE] {
        return validation(
            "Forum storefront Search requires an explicit Forum-only resolved source scope",
        );
    }
    let ranking_profile = SearchRankingProfile::resolve(
        &settings.config,
        STOREFRONT_SEARCH_SURFACE,
        input.ranking_profile.as_deref(),
        resolved_preset.ranking_profile,
    )?;
    let effective_locale = input
        .locale
        .clone()
        .unwrap_or_else(|| input.fallback_locale.clone());
    let auth = input.auth.clone();
    let request_context = input.request_context.clone();
    let category_ids = resolve_storefront_search_category_ids(
        category_scope_port,
        StorefrontSearchCategoryScopeRequest {
            tenant_id: input.tenant_id,
            locale: effective_locale.clone(),
            fallback_locale: Some(input.fallback_locale.clone()),
            source_modules: resolved_preset.source_modules.clone(),
            category_ids: input.category_ids,
            auth: auth.clone(),
            request_context: request_context.clone(),
            transport: input.transport,
        },
    )
    .await?;
    let search_query = SearchQuery {
        tenant_id: Some(input.tenant_id),
        locale: Some(effective_locale.clone()),
        channel_id: input.channel_id,
        original_query: transform.original_query,
        query: transform.effective_query,
        ranking_profile,
        preset_key: resolved_preset.preset.map(|preset| preset.key),
        limit: input.limit,
        offset: input.offset,
        published_only: true,
        entity_types: resolved_preset.entity_types,
        source_modules: resolved_preset.source_modules,
        statuses: resolved_preset.statuses,
        category_ids,
        attribute_filters: input.attribute_filters,
        sort_attribute_code: input.sort_attribute_code,
        sort_desc: input.sort_desc,
    };
    let result = execute_date_window_result_eligible_search(
        db,
        &search_query,
        result_eligibility_port,
        effective_locale,
        auth,
        request_context,
        &trusted_channel,
        &document_filters,
        &locale_date_filters,
        input.transport,
    )
    .await?;
    let result = if document_filters.is_empty() && !locale_date_filters.has_date_window() {
        SearchDictionaryService::apply_storefront_query_rules(
            db,
            &search_query,
            result,
            &trusted_channel,
        )
        .await?
    } else {
        result
    };
    let query_log_id = SearchAnalyticsService::record_query(
        db,
        SearchQueryLogRecord {
            tenant_id: input.tenant_id,
            surface: STOREFRONT_SEARCH_SURFACE.to_string(),
            query: search_query.original_query.clone(),
            locale: search_query.locale.clone(),
            engine: result.engine,
            result_count: result.total,
            took_ms: result.took_ms,
            status: "success".to_string(),
            entity_types: search_query.entity_types.clone(),
            source_modules: search_query.source_modules.clone(),
            statuses: search_query.statuses.clone(),
        },
    )
    .await
    .ok()
    .flatten();

    Ok(ForumStorefrontSearchExecution {
        result,
        query_log_id,
        preset_key: search_query.preset_key,
        elapsed_ms: started_at.elapsed().as_millis() as u64,
    })
}
