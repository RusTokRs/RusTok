use async_graphql::{Context, Object, Result};
use rustok_telemetry::metrics;
use sea_orm::DatabaseConnection;
use std::time::Instant;
use uuid::Uuid;

use crate::context::TenantContext;
use rustok_blog::{BlogError, PostService};
use rustok_content::NodeService;
use rustok_outbox::TransactionalEventBus;

use crate::graphql::common::{require_module_enabled, resolve_graphql_locale};
use crate::graphql::schema::module_slug;

use super::types::*;

#[derive(Default)]
pub struct BlogQuery;

#[Object]
impl BlogQuery {
    async fn post(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
        locale: Option<String>,
    ) -> Result<Option<GqlPost>> {
        require_module_enabled(ctx, module_slug::BLOG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());

        let service = PostService::new(db.clone(), event_bus.clone());
        let post = match service
            .get_post_with_locale_fallback(
                tenant_id,
                id,
                &locale,
                Some(tenant.default_locale.as_str()),
            )
            .await
        {
            Ok(post) => post,
            Err(BlogError::PostNotFound(_))
            | Err(BlogError::Content(rustok_content::ContentError::NodeNotFound(_))) => {
                return Ok(None);
            }
            Err(err) => return Err(async_graphql::Error::new(err.to_string())),
        };

        Ok(Some(post.into()))
    }

    async fn posts(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        filter: Option<PostsFilter>,
    ) -> Result<GqlPostList> {
        require_module_enabled(ctx, module_slug::BLOG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;

        let service = NodeService::new(db.clone(), event_bus.clone());
        let tenant = ctx.data::<TenantContext>()?;
        let filter = filter.unwrap_or(PostsFilter {
            status: None,
            author_id: None,
            locale: None,
            page: Some(1),
            per_page: Some(20),
        });
        let requested_limit = filter.per_page.map(|value| value.max(0) as u64);
        let locale = resolve_graphql_locale(ctx, filter.locale.as_deref());
        let effective_limit = filter.per_page.unwrap_or(20).clamp(1, 100) as u64;

        let domain_filter = rustok_content::dto::ListNodesFilter {
            kind: Some("post".to_string()),
            status: filter.status.map(Into::into),
            parent_id: None,
            author_id: filter.author_id,
            locale: Some(locale),
            category_id: None,
            page: filter.page.unwrap_or(1),
            per_page: filter.per_page.unwrap_or(20),
            include_deleted: false,
        };

        let security = ctx
            .data::<crate::context::AuthContext>()
            .map(|a| a.security_context())
            .unwrap_or_else(|_| rustok_core::SecurityContext::system());

        let list_started_at = Instant::now();
        let (items, total): (Vec<rustok_content::dto::NodeListItem>, u64) = service
            .list_nodes_with_locale_fallback(
                tenant_id,
                security,
                domain_filter,
                Some(tenant.default_locale.as_str()),
            )
            .await?;
        metrics::record_read_path_query(
            "graphql",
            "blog.posts",
            "service_list",
            list_started_at.elapsed().as_secs_f64(),
            total,
        );

        let items = items.into_iter().map(Into::into).collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "blog.posts",
            requested_limit,
            effective_limit,
            items.len(),
        );

        Ok(GqlPostList { items, total })
    }
}
