use std::sync::Arc;

use async_graphql::{Context, FieldError, Object, Result};
use axum::http::HeaderMap;
use rustok_api::{
    AuthContext, RequestContext, TenantContext,
    graphql::{GraphQLError, require_module_enabled},
};
use rustok_core::ModuleRuntimeExtensions;
use rustok_telemetry::metrics;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    ForumStorefrontSearchAttributeFilter, ForumStorefrontSearchExecutionError,
    ForumStorefrontSearchRequest, SharedStorefrontSearchCategoryScopePort,
    SharedStorefrontSearchResultEligibilityPort, StorefrontSearchTransport,
    execute_forum_storefront_search, resolve_trusted_storefront_channel_input,
};

use super::{
    SearchGraphqlRateLimitError, SearchGraphqlRateLimiterHandle,
    types::{SearchPreviewInput, SearchPreviewPayload},
};

const FORUM_MODULE_SLUG: &str = "forum";
const FORUM_STOREFRONT_SEARCH_SURFACE: &str = "forum_storefront_search";
const FORUM_STOREFRONT_SEARCH_UNAVAILABLE: &str =
    "Forum storefront Search is temporarily unavailable";

#[derive(Default)]
pub struct ForumStorefrontSearchQuery;

#[Object]
impl ForumStorefrontSearchQuery {
    /// Executes published Search through the Forum-owned richer category scope,
    /// exact topic/reply result eligibility and optional exact author, tag,
    /// solved-state and inclusive published-date scope. The input must explicitly
    /// select only the `forum` source and at least one category root.
    async fn forum_storefront_search(
        &self,
        ctx: &Context<'_>,
        input: SearchPreviewInput,
        author_ids: Option<Vec<String>>,
        tags: Option<Vec<String>>,
        solved: Option<bool>,
        kinds: Option<Vec<String>>,
        published_from: Option<String>,
        published_to: Option<String>,
    ) -> Result<SearchPreviewPayload> {
        require_module_enabled(ctx, FORUM_MODULE_SLUG).await?;
        enforce_rate_limit(ctx).await?;

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        if let Some(value) = input
            .tenant_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let requested_tenant = Uuid::parse_str(value)
                .map_err(|_| FieldError::new("tenantId contains an invalid UUID"))?;
            if requested_tenant != tenant.id {
                return Err(FieldError::new(
                    "tenantId does not match the authenticated request tenant",
                ));
            }
        }
        let request_context = ctx.data::<RequestContext>()?.clone();
        let trusted_channel = resolve_trusted_storefront_channel_input(
            &request_context,
            tenant.id,
            input.channel_id.as_deref(),
        )
        .map_err(|error| FieldError::new(error.to_string()))?;
        let auth = ctx.data_opt::<AuthContext>().cloned();
        let extensions = ctx.data_opt::<Arc<ModuleRuntimeExtensions>>();
        let category_scope_port = extensions.and_then(|extensions| {
            extensions
                .get::<SharedStorefrontSearchCategoryScopePort>()
                .cloned()
        });
        let result_eligibility_port = extensions.and_then(|extensions| {
            extensions
                .get::<SharedStorefrontSearchResultEligibilityPort>()
                .cloned()
        });
        let request = ForumStorefrontSearchRequest {
            tenant_id: tenant.id,
            query: input.query,
            locale: input.locale,
            fallback_locale: tenant.default_locale.clone(),
            channel_id: trusted_channel.channel_id.map(|value| value.to_string()),
            limit: input.limit,
            offset: input.offset,
            ranking_profile: input.ranking_profile,
            preset_key: input.preset_key,
            entity_types: input.entity_types.unwrap_or_default(),
            source_modules: input.source_modules.unwrap_or_default(),
            statuses: input.statuses.unwrap_or_default(),
            category_ids: input.category_ids.unwrap_or_default(),
            kinds: kinds.unwrap_or_default(),
            author_ids: author_ids.unwrap_or_default(),
            tags: tags.unwrap_or_default(),
            solved,
            published_from,
            published_to,
            attribute_filters: input
                .attribute_filters
                .unwrap_or_default()
                .into_iter()
                .map(|filter| ForumStorefrontSearchAttributeFilter {
                    attribute_code: filter.attribute_code,
                    values: filter.values.unwrap_or_default(),
                    min: filter.min,
                    max: filter.max,
                })
                .collect(),
            sort_attribute_code: input.sort_attribute_code,
            sort_desc: input.sort_desc.unwrap_or(false),
            auth,
            request_context: Some(request_context),
            transport: StorefrontSearchTransport::Graphql,
        };

        let execution = execute_forum_storefront_search(
            db,
            category_scope_port,
            result_eligibility_port,
            request,
        )
        .await
        .map_err(map_execution_error)?;
        metrics::record_read_path_query(
            "graphql",
            FORUM_STOREFRONT_SEARCH_SURFACE,
            "forum_category_scope_document_filters_result_eligibility_then_fts",
            execution.elapsed_ms as f64 / 1000.0,
            execution.result.total,
        );
        let query_log_id = execution.query_log_id.map(|value| value.to_string());
        let preset_key = execution.preset_key;
        let elapsed_ms = execution.elapsed_ms;
        let mut payload: SearchPreviewPayload = execution.result.into();
        payload.query_log_id = query_log_id;
        payload.preset_key = preset_key;
        payload.took_ms = payload.took_ms.max(elapsed_ms);
        Ok(payload)
    }
}

async fn enforce_rate_limit(ctx: &Context<'_>) -> Result<()> {
    let Some(shared) = ctx.data_opt::<SearchGraphqlRateLimiterHandle>() else {
        return Ok(());
    };
    let tenant = ctx.data::<TenantContext>()?;
    let request = ctx.data::<RequestContext>()?;
    let auth = ctx.data_opt::<AuthContext>();
    let headers = ctx.data_opt::<HeaderMap>();
    let client = headers
        .and_then(extract_client_id)
        .or_else(|| auth.map(|auth| format!("user:{}", auth.user_id)))
        .or_else(|| request.user_id.map(|user_id| format!("user:{user_id}")))
        .unwrap_or_else(|| "anonymous".to_string());
    let key = format!(
        "tenant:{}:{}:{}",
        tenant.id, FORUM_STOREFRONT_SEARCH_SURFACE, client
    );

    match shared.0.check_rate_limit(&key).await {
        Ok(()) => Ok(()),
        Err(SearchGraphqlRateLimitError::Exceeded(exceeded)) => Err(FieldError::new(format!(
            "Search rate limit exceeded. Retry after {} seconds",
            exceeded.retry_after
        ))),
        Err(SearchGraphqlRateLimitError::BackendUnavailable(reason)) => {
            tracing::error!(
                tenant_id = %tenant.id,
                %reason,
                "Forum storefront Search rate limit backend unavailable"
            );
            Err(<FieldError as GraphQLError>::internal_error(
                "Search rate limit backend unavailable",
            ))
        }
    }
}

fn extract_client_id(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| value.parse::<std::net::IpAddr>().is_ok())
        .map(|value| format!("ip:{value}"))
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
                .filter(|value| value.parse::<std::net::IpAddr>().is_ok())
                .map(|value| format!("ip:{value}"))
        })
}

fn map_execution_error(error: ForumStorefrontSearchExecutionError) -> FieldError {
    match error {
        ForumStorefrontSearchExecutionError::Validation(message) => FieldError::new(message),
        ForumStorefrontSearchExecutionError::Scope(port_error) => match port_error.kind {
            rustok_api::PortErrorKind::Validation
            | rustok_api::PortErrorKind::NotFound
            | rustok_api::PortErrorKind::Forbidden => FieldError::new(port_error.message),
            _ => {
                tracing::error!(
                    error = ?port_error,
                    "Forum storefront Search owner scope failed"
                );
                <FieldError as GraphQLError>::internal_error(FORUM_STOREFRONT_SEARCH_UNAVAILABLE)
            }
        },
        ForumStorefrontSearchExecutionError::Search(
            rustok_core::Error::Validation(message)
            | rustok_core::Error::NotFound(message)
            | rustok_core::Error::InvalidIdFormat(message),
        ) => FieldError::new(message),
        ForumStorefrontSearchExecutionError::Search(error) => {
            tracing::error!(error = ?error, "Forum storefront Search execution failed");
            <FieldError as GraphQLError>::internal_error(FORUM_STOREFRONT_SEARCH_UNAVAILABLE)
        }
        ForumStorefrontSearchExecutionError::Database(error) => {
            tracing::error!(error = ?error, "Forum storefront Search database failure");
            <FieldError as GraphQLError>::internal_error(FORUM_STOREFRONT_SEARCH_UNAVAILABLE)
        }
        ForumStorefrontSearchExecutionError::Invariant(message) => {
            tracing::error!(message, "Forum storefront Search invariant failed");
            <FieldError as GraphQLError>::internal_error(FORUM_STOREFRONT_SEARCH_UNAVAILABLE)
        }
    }
}
