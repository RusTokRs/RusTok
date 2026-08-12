use std::collections::{BTreeMap, HashSet};
use std::fmt::{Display, Formatter};
use std::time::Instant;

use chrono::{DateTime, Utc};
use rustok_api::{AuthContext, PortError, RequestContext};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::engine::{SearchFacetBucket, SearchFacetGroup};
use crate::forum_current_channel_filter::ForumStorefrontCurrentChannelFilter;
use crate::{
    FORUM_SEARCH_SOURCE_MODULE, ForumStorefrontDocumentFilters, MAX_FORUM_SEARCH_RESULT_CANDIDATES,
    PgSearchEngine, SearchAnalyticsService, SearchAttributeFilter, SearchDictionaryService,
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
const FORUM_STOREFRONT_SEARCH_UNAVAILABLE: &str =
    "Forum storefront Search is temporarily unavailable";

#[derive(Clone, Debug)]
pub struct ForumStorefrontSearchAttributeFilter {
    pub attribute_code: String,
    pub values: Vec<String>,
    pub min: Option<String>,
    pub max: Option<String>,
}

#[derive(Clone)]
pub struct ForumStorefrontSearchRequest {
    pub tenant_id: Uuid,
    pub query: String,
    pub locale: Option<String>,
    pub fallback_locale: String,
    pub channel_id: Option<String>,
    pub current_channel_only: Option<bool>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    pub ranking_profile: Option<String>,
    pub preset_key: Option<String>,
    pub entity_types: Vec<String>,
    pub source_modules: Vec<String>,
    pub statuses: Vec<String>,
    pub category_ids: Vec<String>,
    pub author_ids: Vec<String>,
    pub tags: Vec<String>,
    pub solved: Option<bool>,
    pub published_from: Option<String>,
    pub published_to: Option<String>,
    pub attribute_filters: Vec<ForumStorefrontSearchAttributeFilter>,
    pub sort_attribute_code: Option<String>,
    pub sort_desc: bool,
    pub auth: Option<AuthContext>,
    pub request_context: Option<RequestContext>,
    pub transport: StorefrontSearchTransport,
}

pub struct ForumStorefrontSearchExecution {
    pub result: SearchResult,
    pub query_log_id: Option<i64>,
    pub preset_key: Option<String>,
    pub elapsed_ms: u64,
}

#[derive(Debug)]
pub enum ForumStorefrontSearchExecutionError {
    Validation(String),
    Scope(PortError),
    Search(rustok_core::Error),
    Database(sea_orm::DbErr),
    Invariant(&'static str),
}

impl Display for ForumStorefrontSearchExecutionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation(message) => formatter.write_str(message),
            Self::Scope(error) => formatter.write_str(&error.message),
            Self::Search(error) => Display::fmt(error, formatter),
            Self::Database(_) | Self::Invariant(_) => {
                formatter.write_str(FORUM_STOREFRONT_SEARCH_UNAVAILABLE)
            }
        }
    }
}

impl std::error::Error for ForumStorefrontSearchExecutionError {}

impl From<rustok_core::Error> for ForumStorefrontSearchExecutionError {
    fn from(error: rustok_core::Error) -> Self {
        Self::Search(error)
    }
}

impl From<sea_orm::DbErr> for ForumStorefrontSearchExecutionError {
    fn from(error: sea_orm::DbErr) -> Self {
        Self::Database(error)
    }
}

impl From<PortError> for ForumStorefrontSearchExecutionError {
    fn from(error: PortError) -> Self {
        Self::Scope(error)
    }
}

struct NormalizedForumStorefrontSearchRequest {
    tenant_id: Uuid,
    query: String,
    locale: Option<String>,
    fallback_locale: String,
    channel_id: Option<Uuid>,
    current_channel_filter: ForumStorefrontCurrentChannelFilter,
    limit: usize,
    offset: usize,
    ranking_profile: Option<String>,
    preset_key: Option<String>,
    entity_types: Vec<String>,
    source_modules: Vec<String>,
    statuses: Vec<String>,
    category_ids: Vec<Uuid>,
    document_filters: ForumStorefrontDocumentFilters,
    attribute_filters: Vec<SearchAttributeFilter>,
    sort_attribute_code: Option<String>,
    sort_desc: bool,
    auth: Option<AuthContext>,
    request_context: Option<RequestContext>,
    trusted_channel: TrustedStorefrontChannel,
    transport: StorefrontSearchTransport,
}

pub async fn execute_forum_storefront_search(
    db: &DatabaseConnection,
    category_scope_port: Option<SharedStorefrontSearchCategoryScopePort>,
    result_eligibility_port: Option<SharedStorefrontSearchResultEligibilityPort>,
    request: ForumStorefrontSearchRequest,
) -> Result<ForumStorefrontSearchExecution, ForumStorefrontSearchExecutionError> {
    let input = normalize_request(request)?;
    let trusted_channel = input.trusted_channel.clone();
    let document_filters = input.document_filters.clone();
    let current_channel_filter = input.current_channel_filter.clone();
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
        return Err(ForumStorefrontSearchExecutionError::Validation(
            "Forum storefront Search requires an explicit Forum-only resolved source scope"
                .to_string(),
        ));
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
    let result = execute_result_eligible_search(
        db,
        &search_query,
        result_eligibility_port,
        effective_locale,
        auth,
        request_context,
        &trusted_channel,
        &document_filters,
        &current_channel_filter,
        input.transport,
    )
    .await?;
    let result = if document_filters.is_empty() && current_channel_filter.is_empty() {
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

async fn execute_result_eligible_search(
    db: &DatabaseConnection,
    query: &SearchQuery,
    eligibility_port: Option<SharedStorefrontSearchResultEligibilityPort>,
    effective_locale: String,
    auth: Option<AuthContext>,
    request_context: Option<RequestContext>,
    trusted_channel: &TrustedStorefrontChannel,
    document_filters: &ForumStorefrontDocumentFilters,
    current_channel_filter: &ForumStorefrontCurrentChannelFilter,
    transport: StorefrontSearchTransport,
) -> Result<SearchResult, ForumStorefrontSearchExecutionError> {
    let started_at = Instant::now();
    let engine = PgSearchEngine::new(db.clone());
    let mut scan_query = query.clone();
    scan_query.limit = FORUM_RESULT_SCAN_PAGE_SIZE;
    scan_query.offset = 0;

    let first_page = engine
        .search_storefront(scan_query.clone(), trusted_channel)
        .await?;
    let raw_total = usize::try_from(first_page.total).map_err(|_| {
        ForumStorefrontSearchExecutionError::Validation(
            "Forum storefront Search candidate count is too large".to_string(),
        )
    })?;
    if raw_total > MAX_FORUM_SEARCH_RESULT_CANDIDATES {
        return validation(format!(
            "Forum storefront Search matched more than {MAX_FORUM_SEARCH_RESULT_CANDIDATES} candidates; narrow the query or category scope"
        ));
    }

    let engine_kind = first_page.engine;
    let ranking_profile = first_page.ranking_profile;
    let mut all_items = first_page.items;
    let mut raw_rows = HashSet::with_capacity(raw_total);
    register_raw_rows(&mut raw_rows, &all_items)?;
    while all_items.len() < raw_total {
        scan_query.offset = all_items.len();
        let page = engine
            .search_storefront(scan_query.clone(), trusted_channel)
            .await?;
        if page.total != raw_total as u64 || page.items.is_empty() {
            return Err(ForumStorefrontSearchExecutionError::Invariant(
                "Forum storefront Search candidate snapshot changed during bounded eligibility evaluation",
            ));
        }
        register_raw_rows(&mut raw_rows, &page.items)?;
        all_items.extend(page.items);
        if all_items.len() > raw_total {
            return Err(ForumStorefrontSearchExecutionError::Invariant(
                "Forum storefront Search candidate scan exceeded its initial bounded total",
            ));
        }
    }
    if all_items.len() != raw_total || raw_rows.len() != raw_total {
        return Err(ForumStorefrontSearchExecutionError::Invariant(
            "Forum storefront Search candidate scan did not resolve one unique row per result",
        ));
    }

    all_items.retain(|item| document_filters.matches(item) && current_channel_filter.matches(item));

    let mut seen_candidates = HashSet::new();
    let candidates = all_items
        .iter()
        .filter_map(result_candidate)
        .filter(|candidate| seen_candidates.insert(*candidate))
        .collect::<Vec<_>>();
    let allowed = resolve_storefront_search_result_candidates(
        eligibility_port,
        StorefrontSearchResultEligibilityRequest {
            tenant_id: query.tenant_id.expect("validated Forum Search tenant"),
            locale: effective_locale,
            candidates,
            auth,
            request_context,
            transport,
        },
    )
    .await?;
    let allowed = allowed.into_iter().collect::<HashSet<_>>();

    let visible_items = all_items
        .into_iter()
        .filter(|item| match item.entity_type.as_str() {
            "forum_category" => true,
            "forum_topic" | "forum_reply" => {
                result_candidate(item).is_some_and(|candidate| allowed.contains(&candidate))
            }
            _ => false,
        })
        .collect::<Vec<_>>();
    let total = visible_items.len() as u64;
    let facets = build_forum_result_facets(&visible_items);
    let items = visible_items
        .into_iter()
        .skip(query.offset)
        .take(query.limit)
        .collect();

    Ok(SearchResult {
        items,
        total,
        took_ms: started_at.elapsed().as_millis() as u64,
        engine: engine_kind,
        ranking_profile,
        facets,
    })
}

fn register_raw_rows(
    seen: &mut HashSet<(String, String, Uuid, Option<String>)>,
    items: &[SearchResultItem],
) -> Result<(), ForumStorefrontSearchExecutionError> {
    for item in items {
        if !seen.insert((
            item.source_module.clone(),
            item.entity_type.clone(),
            item.id,
            item.locale.clone(),
        )) {
            return Err(ForumStorefrontSearchExecutionError::Invariant(
                "Forum storefront Search candidate scan returned a duplicate raw row",
            ));
        }
    }
    Ok(())
}

fn result_candidate(item: &SearchResultItem) -> Option<StorefrontSearchResultCandidate> {
    StorefrontSearchResultCandidateKind::from_entity_type(&item.entity_type).map(|kind| {
        StorefrontSearchResultCandidate {
            document_id: item.id,
            kind,
        }
    })
}

fn build_forum_result_facets(items: &[SearchResultItem]) -> Vec<SearchFacetGroup> {
    let mut entity_types = BTreeMap::<String, u64>::new();
    let mut source_modules = BTreeMap::<String, u64>::new();
    let mut statuses = BTreeMap::<String, u64>::new();
    for item in items {
        *entity_types.entry(item.entity_type.clone()).or_default() += 1;
        *source_modules
            .entry(item.source_module.clone())
            .or_default() += 1;
        if let Some(status) = forum_visible_status(&item.entity_type) {
            *statuses.entry(status.to_string()).or_default() += 1;
        }
    }
    vec![
        SearchFacetGroup {
            name: "entity_type".to_string(),
            buckets: facet_buckets(entity_types),
        },
        SearchFacetGroup {
            name: "source_module".to_string(),
            buckets: facet_buckets(source_modules),
        },
        SearchFacetGroup {
            name: "status".to_string(),
            buckets: facet_buckets(statuses),
        },
    ]
}

fn facet_buckets(values: BTreeMap<String, u64>) -> Vec<SearchFacetBucket> {
    let mut buckets = values
        .into_iter()
        .map(|(value, count)| SearchFacetBucket {
            value,
            label: None,
            count,
        })
        .collect::<Vec<_>>();
    buckets.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.value.cmp(&right.value))
    });
    buckets
}

fn forum_visible_status(entity_type: &str) -> Option<&'static str> {
    match entity_type {
        "forum_category" => Some("public"),
        "forum_topic" => Some("open"),
        "forum_reply" => Some("approved"),
        _ => None,
    }
}

fn normalize_request(
    request: ForumStorefrontSearchRequest,
) -> Result<NormalizedForumStorefrontSearchRequest, ForumStorefrontSearchExecutionError> {
    if request.tenant_id.is_nil() {
        return validation("Forum storefront Search requires a tenant");
    }
    let query = normalize_query(&request.query)?;
    let locale = normalize_locale(request.locale.as_deref())?;
    let fallback_locale = normalize_required_locale(&request.fallback_locale)?;
    let exact_locale = locale.clone().unwrap_or_else(|| fallback_locale.clone());
    let published_from =
        normalize_optional_rfc3339("published_from", request.published_from.as_deref())?;
    let published_to = normalize_optional_rfc3339("published_to", request.published_to.as_deref())?;
    if published_from
        .as_ref()
        .zip(published_to.as_ref())
        .is_some_and(|(from, to)| from > to)
    {
        return validation("published_from must not be after published_to");
    }
    let requested_channel_id = parse_optional_uuid("channel_id", request.channel_id.as_deref())?;
    let request_context = request.request_context.ok_or_else(|| {
        ForumStorefrontSearchExecutionError::Validation(
            "Forum storefront Search requires trusted request context".to_string(),
        )
    })?;
    let trusted_channel = resolve_trusted_storefront_channel(
        &request_context,
        request.tenant_id,
        requested_channel_id,
    )
    .map_err(|error| ForumStorefrontSearchExecutionError::Validation(error.to_string()))?;
    let current_channel_filter =
        resolve_current_channel_filter(request.current_channel_only, &trusted_channel)?;
    let source_modules = normalize_filter_values("source_modules", request.source_modules)?;
    if source_modules.as_slice() != [FORUM_SEARCH_SOURCE_MODULE] {
        return validation("Forum storefront Search requires source_modules: [forum]");
    }
    let category_ids = normalize_uuid_values("category_ids", request.category_ids)?;
    if category_ids.is_empty() {
        return validation("Forum storefront Search requires at least one category_id");
    }
    let ranking_profile = request
        .ranking_profile
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    if let Some(value) = ranking_profile.as_deref() {
        if SearchRankingProfile::try_from_str(value).is_none() {
            return validation("Unsupported ranking profile");
        }
    }

    Ok(NormalizedForumStorefrontSearchRequest {
        tenant_id: request.tenant_id,
        query,
        locale,
        fallback_locale,
        channel_id: trusted_channel.channel_id,
        current_channel_filter,
        limit: request.limit.unwrap_or(12).clamp(1, 50) as usize,
        offset: request.offset.unwrap_or(0).max(0) as usize,
        ranking_profile,
        preset_key: normalize_preset_key(request.preset_key)?,
        entity_types: normalize_filter_values("entity_types", request.entity_types)?,
        source_modules,
        statuses: normalize_filter_values("statuses", request.statuses)?,
        category_ids,
        document_filters: ForumStorefrontDocumentFilters {
            exact_locale: Some(exact_locale),
            author_ids: normalize_uuid_values("author_ids", request.author_ids)?,
            tags: normalize_tag_values("tags", request.tags)?,
            solved: request.solved,
            published_from,
            published_to,
        },
        attribute_filters: normalize_attribute_filters(request.attribute_filters)?,
        sort_attribute_code: normalize_attribute_code(request.sort_attribute_code)?,
        sort_desc: request.sort_desc,
        auth: request.auth,
        request_context: Some(request_context),
        trusted_channel,
        transport: request.transport,
    })
}

fn resolve_current_channel_filter(
    requested: Option<bool>,
    trusted_channel: &TrustedStorefrontChannel,
) -> Result<ForumStorefrontCurrentChannelFilter, ForumStorefrontSearchExecutionError> {
    if requested != Some(true) {
        return Ok(ForumStorefrontCurrentChannelFilter::default());
    }
    let channel_slug = trusted_channel.channel_slug.clone().ok_or_else(|| {
        ForumStorefrontSearchExecutionError::Validation(
            "current_channel_only requires a trusted storefront channel".to_string(),
        )
    })?;
    Ok(ForumStorefrontCurrentChannelFilter {
        channel_slug: Some(channel_slug),
    })
}

fn normalize_query(value: &str) -> Result<String, ForumStorefrontSearchExecutionError> {
    let value = value.trim();
    if value.len() > MAX_SEARCH_QUERY_LEN {
        return validation("Search query exceeds the maximum length of 256 characters");
    }
    if value.chars().any(char::is_control) {
        return validation("Search query contains unsupported control characters");
    }
    Ok(value.to_string())
}

fn normalize_locale(
    value: Option<&str>,
) -> Result<Option<String>, ForumStorefrontSearchExecutionError> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(normalize_required_locale)
        .transpose()
}

fn normalize_required_locale(value: &str) -> Result<String, ForumStorefrontSearchExecutionError> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > MAX_LOCALE_LEN
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return validation("Invalid locale format");
    }
    Ok(value.to_ascii_lowercase())
}

fn normalize_optional_rfc3339(
    field: &str,
    value: Option<&str>,
) -> Result<Option<DateTime<Utc>>, ForumStorefrontSearchExecutionError> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            DateTime::parse_from_rfc3339(value)
                .map(|timestamp| timestamp.with_timezone(&Utc))
                .map_err(|_| {
                    ForumStorefrontSearchExecutionError::Validation(format!(
                        "{field} must be RFC3339"
                    ))
                })
        })
        .transpose()
}

fn normalize_filter_values(
    field: &str,
    values: Vec<String>,
) -> Result<Vec<String>, ForumStorefrontSearchExecutionError> {
    if values.len() > MAX_FILTER_VALUES {
        return validation(format!(
            "{field} exceeds the maximum size of {MAX_FILTER_VALUES} values"
        ));
    }
    values
        .into_iter()
        .map(|value| {
            let value = value.trim().to_ascii_lowercase();
            if value.is_empty()
                || value.len() > MAX_FILTER_VALUE_LEN
                || !value
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == ':')
            {
                return validation(format!("{field} contains an invalid value"));
            }
            Ok(value)
        })
        .collect()
}

fn normalize_tag_values(
    field: &str,
    values: Vec<String>,
) -> Result<Vec<String>, ForumStorefrontSearchExecutionError> {
    if values.len() > MAX_FILTER_VALUES {
        return validation(format!(
            "{field} exceeds the maximum size of {MAX_FILTER_VALUES} values"
        ));
    }
    let mut normalized = values
        .into_iter()
        .map(|value| {
            let value = value.trim();
            if value.is_empty()
                || value.chars().count() > MAX_FILTER_VALUE_LEN
                || value.chars().any(char::is_control)
            {
                return validation(format!("{field} contains an invalid value"));
            }
            Ok(value.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    normalized.sort();
    normalized.dedup();
    Ok(normalized)
}

fn normalize_uuid_values(
    field: &str,
    values: Vec<String>,
) -> Result<Vec<Uuid>, ForumStorefrontSearchExecutionError> {
    if values.len() > MAX_FILTER_VALUES {
        return validation(format!(
            "{field} exceeds the maximum size of {MAX_FILTER_VALUES} values"
        ));
    }
    values
        .into_iter()
        .map(|value| {
            Uuid::parse_str(value.trim()).map_err(|_| {
                ForumStorefrontSearchExecutionError::Validation(format!(
                    "{field} contains an invalid UUID"
                ))
            })
        })
        .collect()
}

fn parse_optional_uuid(
    field: &str,
    value: Option<&str>,
) -> Result<Option<Uuid>, ForumStorefrontSearchExecutionError> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            Uuid::parse_str(value).map_err(|_| {
                ForumStorefrontSearchExecutionError::Validation(format!(
                    "{field} contains an invalid UUID"
                ))
            })
        })
        .transpose()
}

fn normalize_preset_key(
    value: Option<String>,
) -> Result<Option<String>, ForumStorefrontSearchExecutionError> {
    let value = value
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    if let Some(value) = value.as_deref() {
        if value.len() > MAX_FILTER_VALUE_LEN
            || !value
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == ':')
        {
            return validation("Invalid preset key");
        }
    }
    Ok(value)
}

fn normalize_attribute_code(
    value: Option<String>,
) -> Result<Option<String>, ForumStorefrontSearchExecutionError> {
    let value = value
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    if let Some(value) = value.as_deref() {
        validate_attribute_code(value)?;
    }
    Ok(value)
}

fn normalize_attribute_filters(
    filters: Vec<ForumStorefrontSearchAttributeFilter>,
) -> Result<Vec<SearchAttributeFilter>, ForumStorefrontSearchExecutionError> {
    if filters.len() > MAX_ATTRIBUTE_FILTERS {
        return validation(format!(
            "attribute_filters exceeds the maximum size of {MAX_ATTRIBUTE_FILTERS} filters"
        ));
    }
    filters
        .into_iter()
        .map(|filter| {
            let attribute_code = filter.attribute_code.trim().to_ascii_lowercase();
            validate_attribute_code(&attribute_code)?;
            Ok(SearchAttributeFilter {
                attribute_code,
                values: normalize_filter_values("attribute_filter.values", filter.values)?,
                min: normalize_attribute_bound(filter.min)?,
                max: normalize_attribute_bound(filter.max)?,
            })
        })
        .collect()
}

fn validate_attribute_code(value: &str) -> Result<(), ForumStorefrontSearchExecutionError> {
    if value.is_empty()
        || value.len() > MAX_FILTER_VALUE_LEN
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return validation("attribute_code contains an invalid value");
    }
    Ok(())
}

fn normalize_attribute_bound(
    value: Option<String>,
) -> Result<Option<String>, ForumStorefrontSearchExecutionError> {
    let value = value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if let Some(value) = value.as_deref() {
        if value.len() > MAX_FILTER_VALUE_LEN || value.chars().any(char::is_control) {
            return validation("attribute filter bound contains an invalid value");
        }
    }
    Ok(value)
}

fn validation<T>(message: impl Into<String>) -> Result<T, ForumStorefrontSearchExecutionError> {
    Err(ForumStorefrontSearchExecutionError::Validation(
        message.into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::{forum_visible_status, normalize_optional_rfc3339, resolve_current_channel_filter};
    use crate::TrustedStorefrontChannel;

    #[test]
    fn visible_forum_statuses_match_owner_eligibility() {
        assert_eq!(forum_visible_status("forum_category"), Some("public"));
        assert_eq!(forum_visible_status("forum_topic"), Some("open"));
        assert_eq!(forum_visible_status("forum_reply"), Some("approved"));
        assert_eq!(forum_visible_status("product"), None);
    }

    #[test]
    fn date_bounds_require_rfc3339() {
        assert!(normalize_optional_rfc3339("published_from", Some("2026-07-31T00:00:00Z")).is_ok());
        assert!(normalize_optional_rfc3339("published_from", Some("2026-07-31")).is_err());
    }

    #[test]
    fn current_channel_only_resolves_exact_trusted_slug() {
        let filter = resolve_current_channel_filter(
            Some(true),
            &TrustedStorefrontChannel {
                channel_id: Some(uuid::Uuid::new_v4()),
                channel_slug: Some("web".to_string()),
            },
        )
        .expect("trusted current channel should resolve");
        assert_eq!(filter.channel_slug.as_deref(), Some("web"));
    }

    #[test]
    fn current_channel_only_rejects_unscoped_request() {
        let error = resolve_current_channel_filter(
            Some(true),
            &TrustedStorefrontChannel {
                channel_id: None,
                channel_slug: None,
            },
        )
        .expect_err("unscoped request must not select a channel");
        assert_eq!(
            error.to_string(),
            "current_channel_only requires a trusted storefront channel"
        );
    }

    #[test]
    fn false_current_channel_filter_preserves_existing_behavior() {
        let filter = resolve_current_channel_filter(
            Some(false),
            &TrustedStorefrontChannel {
                channel_id: None,
                channel_slug: None,
            },
        )
        .expect("false must preserve existing behavior");
        assert!(filter.is_empty());
    }
}
