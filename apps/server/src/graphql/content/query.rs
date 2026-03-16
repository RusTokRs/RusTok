use async_graphql::{Context, Object, Result};
use rustok_telemetry::metrics;
use sea_orm::DatabaseConnection;
use std::time::Instant;
use uuid::Uuid;

use crate::context::AuthContext;
use crate::context::TenantContext;
use crate::graphql::common::require_module_enabled;
use crate::graphql::common::resolve_graphql_locale;
use crate::graphql::schema::module_slug;
use rustok_content::NodeService;
use rustok_core::{SecurityContext, UserRole};
use rustok_outbox::TransactionalEventBus;

use super::types::*;

#[derive(Default)]
pub struct ContentQuery;

#[Object]
impl ContentQuery {
    async fn node(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
        locale: Option<String>,
    ) -> Result<Option<GqlNode>> {
        require_module_enabled(ctx, module_slug::CONTENT).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());

        let service = NodeService::new(db.clone(), event_bus.clone());
        match service.get_node(tenant_id, id).await {
            Ok(node) => Ok(Some(GqlNode::from_node_with_locale(
                node,
                Some(locale.as_str()),
                Some(tenant.default_locale.as_str()),
            ))),
            Err(rustok_content::ContentError::NodeNotFound(_)) => Ok(None),
            Err(err) => Err(async_graphql::Error::new(err.to_string())),
        }
    }

    async fn nodes(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        filter: Option<NodesFilter>,
    ) -> Result<GqlNodeList> {
        require_module_enabled(ctx, module_slug::CONTENT).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;

        let service = NodeService::new(db.clone(), event_bus.clone());
        let tenant = ctx.data::<TenantContext>()?;
        let filter = filter.unwrap_or(NodesFilter {
            kind: None,
            status: None,
            parent_id: None,
            author_id: None,
            locale: None,
            page: Some(1),
            per_page: Some(20),
        });
        let requested_limit = filter.per_page.map(|value| value.max(0) as u64);
        let locale = resolve_graphql_locale(ctx, filter.locale.as_deref());
        let effective_limit = filter.per_page.unwrap_or(20).clamp(1, 100) as u64;

        let domain_filter = rustok_content::dto::ListNodesFilter {
            kind: filter.kind,
            status: filter.status.map(Into::into),
            parent_id: filter.parent_id,
            author_id: filter.author_id,
            category_id: None,
            locale: Some(locale),
            page: filter.page.unwrap_or(1),
            per_page: filter.per_page.unwrap_or(20),
            include_deleted: false,
        };

        let security = ctx
            .data::<AuthContext>()
            .map(|auth| auth.security_context())
            .unwrap_or_else(|_| SecurityContext::new(UserRole::Customer, None));

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
            "content.nodes",
            "service_list",
            list_started_at.elapsed().as_secs_f64(),
            total,
        );

        let items = items.into_iter().map(Into::into).collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "content.nodes",
            requested_limit,
            effective_limit,
            items.len(),
        );

        Ok(GqlNodeList { items, total })
    }
}
