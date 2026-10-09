//! Authenticated authoring surface for Pages-owned layout definitions.
//! These definitions are not yet composed into public artifacts.
use async_graphql::{Context, Object, Result, SimpleObject};
use rustok_api::{AuthContext, Permission, TenantContext, graphql::require_module_enabled, has_any_effective_permission};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{PageTemplateRecord, PageTemplateService};

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlPageTemplate {
    pub key: String,
    pub locale: String,
    pub header_symbol_ids: Vec<String>,
    pub footer_symbol_ids: Vec<String>,
    pub revision: i64,
}

impl From<PageTemplateRecord> for GqlPageTemplate {
    fn from(value: PageTemplateRecord) -> Self {
        Self { key: value.key, locale: value.locale, header_symbol_ids: value.header_symbol_ids, footer_symbol_ids: value.footer_symbol_ids, revision: value.revision }
    }
}

fn scope(ctx: &Context<'_>, requested: Option<Uuid>, permission: Permission) -> Result<(Uuid, rustok_core::SecurityContext)> {
    let auth = ctx.data::<AuthContext>().map_err(|_| async_graphql::Error::new("Pages template authoring requires authentication"))?;
    let tenant = ctx.data::<TenantContext>()?;
    if auth.tenant_id != tenant.id || requested.is_some_and(|id| id != tenant.id) {
        return Err(async_graphql::Error::new("Pages template authoring must use the current tenant"));
    }
    if !has_any_effective_permission(&auth.permissions, &[permission]) {
        return Err(async_graphql::Error::new("Permission denied"));
    }
    let security = rustok_core::security_context_from_access_token(auth.user_id, &auth.grant_type, &auth.permissions);
    Ok((tenant.id, security))
}

#[derive(Default)]
pub struct PageTemplateQuery;

#[Object]
impl PageTemplateQuery {
    async fn page_templates(&self, ctx: &Context<'_>, locale: String, tenant_id: Option<Uuid>) -> Result<Vec<GqlPageTemplate>> {
        require_module_enabled(ctx, "pages").await?;
        let (tenant_id, security) = scope(ctx, tenant_id, Permission::PAGES_READ)?;
        PageTemplateService::new(ctx.data::<DatabaseConnection>()?.clone())
            .list(tenant_id, &security, &locale).await
            .map(|rows| rows.into_iter().map(Into::into).collect())
            .map_err(|err| async_graphql::Error::new(err.to_string()))
    }
}

#[derive(Default)]
pub struct PageTemplateMutation;

#[Object]
impl PageTemplateMutation {
    async fn save_page_template(&self, ctx: &Context<'_>, key: String, locale: String, header_symbol_ids: Vec<String>, footer_symbol_ids: Vec<String>, expected_revision: Option<i64>, tenant_id: Option<Uuid>) -> Result<GqlPageTemplate> {
        require_module_enabled(ctx, "pages").await?;
        let (tenant_id, security) = scope(ctx, tenant_id, Permission::PAGES_MANAGE)?;
        PageTemplateService::new(ctx.data::<DatabaseConnection>()?.clone())
            .save(tenant_id, &security, &key, &locale, header_symbol_ids, footer_symbol_ids, expected_revision).await
            .map(Into::into).map_err(|err| async_graphql::Error::new(err.to_string()))
    }

    async fn delete_page_template(&self, ctx: &Context<'_>, key: String, locale: String, expected_revision: i64, tenant_id: Option<Uuid>) -> Result<bool> {
        require_module_enabled(ctx, "pages").await?;
        let (tenant_id, security) = scope(ctx, tenant_id, Permission::PAGES_MANAGE)?;
        PageTemplateService::new(ctx.data::<DatabaseConnection>()?.clone())
            .delete(tenant_id, &security, &key, &locale, expected_revision).await
            .map(|()| true).map_err(|err| async_graphql::Error::new(err.to_string()))
    }
}
