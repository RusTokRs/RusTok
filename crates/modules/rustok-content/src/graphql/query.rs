use std::time::Instant;

use async_graphql::{Context, FieldError, Object, Result, SimpleObject};
use rustok_api::{TenantContext, graphql::GraphQLError};
use rustok_telemetry::metrics;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{CanonicalUrlService, ContentError, ResolvedContentRoute};

#[derive(Default)]
pub struct ContentQuery;

#[Object]
impl ContentQuery {
    async fn resolve_canonical_route(
        &self,
        ctx: &Context<'_>,
        route: String,
        locale: String,
    ) -> Result<Option<ResolvedCanonicalRoute>> {
        let db = ctx.data::<DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let lookup_started_at = Instant::now();
        let resolved = CanonicalUrlService::new(db.clone())
            .resolve_route(tenant.id, locale.as_str(), route.as_str())
            .await
            .map_err(map_content_error)?;

        metrics::record_read_path_query(
            "graphql",
            "content.resolve_canonical_route",
            "canonical_lookup",
            lookup_started_at.elapsed().as_secs_f64(),
            resolved.is_some() as u64,
        );

        Ok(resolved.map(ResolvedCanonicalRoute::from))
    }
}

#[derive(SimpleObject, Debug, Clone)]
pub struct ResolvedCanonicalRoute {
    pub target_kind: String,
    pub target_id: Uuid,
    pub locale: String,
    pub matched_url: String,
    pub canonical_url: String,
    pub redirect_required: bool,
}

impl From<ResolvedContentRoute> for ResolvedCanonicalRoute {
    fn from(value: ResolvedContentRoute) -> Self {
        Self {
            target_kind: value.target_kind,
            target_id: value.target_id,
            locale: value.locale,
            matched_url: value.matched_url,
            canonical_url: value.canonical_url,
            redirect_required: value.redirect_required,
        }
    }
}

fn map_content_error(err: ContentError) -> FieldError {
    match err {
        ContentError::Validation(message) | ContentError::Forbidden(message) => {
            FieldError::new(message)
        }
        ContentError::NodeNotFound(_)
        | ContentError::CategoryNotFound(_)
        | ContentError::TranslationNotFound { .. }
        | ContentError::DuplicateSlug { .. }
        | ContentError::ConcurrentModification { .. } => FieldError::new(err.to_string()),
        ContentError::Database(inner) => {
            <FieldError as GraphQLError>::internal_error(&inner.to_string())
        }
        ContentError::Core(inner) => {
            <FieldError as GraphQLError>::internal_error(&inner.to_string())
        }
        ContentError::Rich(inner) => {
            <FieldError as GraphQLError>::internal_error(&inner.to_string())
        }
    }
}
