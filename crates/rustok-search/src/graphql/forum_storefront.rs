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

use crate::{
    ForumStorefrontSearchAttributeFilter, ForumStorefrontSearchExecutionError,
    ForumStorefrontSearchRequest, SharedStorefrontSearchCategoryScopePort,
    StorefrontSearchTransport, execute_forum_storefront_search,
};

use super::{
    SearchGraphqlRateLimitError, SearchGraphqlRateLimiterHandle,
    types::{SearchPreviewInput, SearchPreviewPayload},
};

const FORUM_MODULE_SLUG: &str = "forum";
const FORUM_STOREFRONT_SEARCH_SURFACE: &str = "forum_storefront_search";

#[derive(Default)]
pub struct ForumStorefrontSearchQuery;

#[Object]
impl ForumStorefrontSearchQuery {
    /// Executes published Search through the Forum-owned richer category scope.
    /// The input must explicitly select only the `forum` source and at least one
    /// category root; mixed Search remains on the ordinary storefront field.
    async fn forum_storefront_search(
        &self,
        ctx: &Context<'_>,
        input: SearchPreviewInput,
    ) -> Result<SearchPreviewPayload> {
        require_module_enabled(ctx, FORUM_MODULE_SLUG).await?;
        enforce_rate_limit(ctx).await?;

        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let request_context = ctx.data::<RequestContext>()?.clone();
        let auth = ctx.data_opt::<AuthContext>().cloned();
        let category_scope_port = ctx
            .data_opt::<Arc<ModuleRuntimeExtensions>>()
            .and_then(|extensions| {
                extensions
                    .get::<SharedStorefrontSearchCategoryScopePort>()
                    .cloned()
            });
        let request = ForumStorefrontSearchRequest {
            tenant_id: tenant.id,
            query: input.query,
            locale: input.locale,
            fallback_locale: tenant.default_locale.clone(),
            channel_id: input.channel_id,
            limit: input.limit,
            offset: input.offset,
            ranking_profile: input.ranking_profile,
            preset_key: input.preset_key,
            entity_types: input.entity_types.unwrap_or_default(),
            source_modules: input.source_modules.unwrap_or_default(),
            statuses: input.statuses.unwrap_or_default(),
            category_ids: input.category_ids.unwrap_or_default(),
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

        let execution = execute_forum_storefront_search(db, category_scope_port, request)
            .await
            .map_err(map_execution_error)?;
        metrics::record_read_path_query(
            "graphql",
            FORUM_STOREFRONT_SEARCH_SURFACE,
            "forum_category_scope_then_fts",
            execution.elapsed_ms as f64 / 1000.0,
            execution.result.total,
        );
        let query_log_id = execution.query_log_id.map(|value| value.to_string());
        let preset_key = execution.preset_key;
        let mut payload: SearchPreviewPayload = execution.result.into();
        payload.query_log_id = query_log_id;
        payload.preset_key = preset_key;
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
            _ => <FieldError as GraphQLError>::internal_error(&port_error.message),
        },
        ForumStorefrontSearchExecutionError::Search(
            rustok_core::Error::Validation(message)
            | rustok_core::Error::NotFound(message)
            | rustok_core::Error::InvalidIdFormat(message),
        ) => FieldError::new(message),
        ForumStorefrontSearchExecutionError::Search(error) => {
            <FieldError as GraphQLError>::internal_error(&error.to_string())
        }
        ForumStorefrontSearchExecutionError::Database(error) => {
            tracing::error!(error = ?error, "Forum storefront Search database failure");
            <FieldError as GraphQLError>::internal_error(
                "Forum storefront Search is temporarily unavailable",
            )
        }
    }
}
