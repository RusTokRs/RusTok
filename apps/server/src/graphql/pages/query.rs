use async_graphql::{Context, Object, Result};
use rustok_telemetry::metrics;
use sea_orm::DatabaseConnection;
use std::time::Instant;
use uuid::Uuid;

use rustok_outbox::TransactionalEventBus;
use rustok_pages::PageService;

use crate::context::AuthContext;
use crate::context::TenantContext;
use crate::graphql::common::require_module_enabled;
use crate::graphql::common::resolve_graphql_locale;
use crate::graphql::schema::module_slug;
use rustok_core::SecurityContext;

use super::types::*;

#[derive(Default)]
pub struct PagesQuery;

#[Object]
impl PagesQuery {
    async fn page(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        id: Uuid,
        locale: Option<String>,
    ) -> Result<Option<GqlPage>> {
        require_module_enabled(ctx, module_slug::PAGES).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let security = auth_context_to_security(ctx);
        let tenant = ctx.data::<TenantContext>()?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());

        let service = PageService::new(db.clone(), event_bus.clone());
        match service
            .get_with_locale_fallback(
                tenant_id,
                security,
                id,
                &locale,
                Some(tenant.default_locale.as_str()),
            )
            .await
        {
            Ok(page) => Ok(Some(page.into())),
            Err(rustok_pages::PagesError::PageNotFound(_)) => Ok(None),
            Err(err) => Err(async_graphql::Error::new(err.to_string())),
        }
    }

    async fn page_by_slug(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        locale: Option<String>,
        slug: String,
    ) -> Result<Option<GqlPage>> {
        require_module_enabled(ctx, module_slug::PAGES).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let security = auth_context_to_security(ctx);
        let tenant = ctx.data::<TenantContext>()?;
        let locale = resolve_graphql_locale(ctx, locale.as_deref());

        let service = PageService::new(db.clone(), event_bus.clone());
        let page = service
            .get_by_slug_with_locale_fallback(
                tenant_id,
                security,
                &locale,
                &slug,
                Some(tenant.default_locale.as_str()),
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;

        Ok(page.map(Into::into))
    }

    async fn pages(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        filter: Option<ListGqlPagesFilter>,
    ) -> Result<GqlPageList> {
        require_module_enabled(ctx, module_slug::PAGES).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let security = auth_context_to_security(ctx);

        let filter = filter.unwrap_or(ListGqlPagesFilter {
            locale: None,
            template: None,
            page: Some(1),
            per_page: Some(20),
        });
        let requested_limit = filter.per_page.map(|value| value.max(0) as u64);
        let locale = resolve_graphql_locale(ctx, filter.locale.as_deref());

        let service = PageService::new(db.clone(), event_bus.clone());
        let list_started_at = Instant::now();
        let (items, total) = service
            .list(
                tenant_id,
                security,
                rustok_pages::ListPagesFilter {
                    status: None,
                    template: filter.template,
                    locale: Some(locale),
                    page: filter.page.unwrap_or(1),
                    per_page: filter.per_page.unwrap_or(20),
                },
            )
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?;
        metrics::record_read_path_query(
            "graphql",
            "pages.pages",
            "service_list",
            list_started_at.elapsed().as_secs_f64(),
            total,
        );

        let items = items.into_iter().map(Into::into).collect::<Vec<_>>();

        metrics::record_read_path_budget(
            "graphql",
            "pages.pages",
            requested_limit,
            filter.per_page.unwrap_or(20).min(100) as u64,
            items.len(),
        );

        Ok(GqlPageList { items, total })
    }
}

fn auth_context_to_security(ctx: &Context<'_>) -> SecurityContext {
    ctx.data::<AuthContext>()
        .map(|a| a.security_context())
        .unwrap_or_else(|_| SecurityContext::system())
}
