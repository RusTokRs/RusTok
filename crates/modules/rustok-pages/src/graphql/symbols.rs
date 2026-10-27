//! GraphQL query and mutation surface for site symbols.
use async_graphql::{Context, InputObject, Object, Result, SimpleObject};
use rustok_api::{
    AuthContext, Permission, TenantContext, graphql::require_module_enabled,
    has_any_effective_permission,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{SaveSiteSymbolInput, SiteSymbolRecord, SiteSymbolService};

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlSiteSymbol {
    pub symbol_id: String,
    pub locale: String,
    pub name: Option<String>,
    pub content_json: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<SiteSymbolRecord> for GqlSiteSymbol {
    fn from(value: SiteSymbolRecord) -> Self {
        Self {
            symbol_id: value.symbol_id,
            locale: value.locale,
            name: value.name,
            content_json: serde_json::to_string(&value.content).unwrap_or_default(),
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Clone, Debug, InputObject)]
pub struct SaveGqlSiteSymbolInput {
    pub symbol_id: String,
    pub locale: String,
    pub name: Option<String>,
    pub content_json: String,
}

fn scope(
    ctx: &Context<'_>,
    requested: Option<Uuid>,
    permission: Permission,
) -> Result<(Uuid, rustok_core::SecurityContext)> {
    let auth = ctx.data::<AuthContext>().map_err(|_| {
        async_graphql::Error::new("Pages symbol authoring requires authentication")
    })?;
    let tenant = ctx.data::<TenantContext>()?;
    if auth.tenant_id != tenant.id || requested.is_some_and(|id| id != tenant.id) {
        return Err(async_graphql::Error::new(
            "Pages symbol authoring must use the current tenant",
        ));
    }
    if !has_any_effective_permission(&auth.permissions, &[permission]) {
        return Err(async_graphql::Error::new("Permission denied"));
    }
    let security = rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    );
    Ok((tenant.id, security))
}

#[derive(Default)]
pub struct SiteSymbolQuery;

#[Object]
impl SiteSymbolQuery {
    async fn site_symbols(
        &self,
        ctx: &Context<'_>,
        locale: String,
        tenant_id: Option<Uuid>,
    ) -> Result<Vec<GqlSiteSymbol>> {
        require_module_enabled(ctx, "pages").await?;
        let (tenant_id, security) = scope(ctx, tenant_id, Permission::PAGES_MANAGE)?;
        SiteSymbolService::new(ctx.data::<DatabaseConnection>()?.clone())
            .list(tenant_id, &security, &locale)
            .await
            .map(|rows| rows.into_iter().map(Into::into).collect())
            .map_err(|err| async_graphql::Error::new(err.to_string()))
    }

    async fn site_symbol(
        &self,
        ctx: &Context<'_>,
        symbol_id: String,
        locale: String,
        tenant_id: Option<Uuid>,
    ) -> Result<Option<GqlSiteSymbol>> {
        require_module_enabled(ctx, "pages").await?;
        let (tenant_id, security) = scope(ctx, tenant_id, Permission::PAGES_MANAGE)?;
        SiteSymbolService::new(ctx.data::<DatabaseConnection>()?.clone())
            .get(tenant_id, &security, &symbol_id, &locale)
            .await
            .map(|opt| opt.map(Into::into))
            .map_err(|err| async_graphql::Error::new(err.to_string()))
    }
}

#[derive(Default)]
pub struct SiteSymbolMutation;

#[Object]
impl SiteSymbolMutation {
    async fn save_site_symbol(
        &self,
        ctx: &Context<'_>,
        input: SaveGqlSiteSymbolInput,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlSiteSymbol> {
        require_module_enabled(ctx, "pages").await?;
        let (tenant_id, security) = scope(ctx, tenant_id, Permission::PAGES_MANAGE)?;
        let content = serde_json::from_str(&input.content_json).map_err(|err| {
            async_graphql::Error::new(format!("Invalid site symbol content JSON: {err}"))
        })?;
        let service_input = SaveSiteSymbolInput {
            symbol_id: input.symbol_id,
            locale: input.locale,
            name: input.name,
            content,
        };
        SiteSymbolService::new(ctx.data::<DatabaseConnection>()?.clone())
            .save(tenant_id, &security, service_input)
            .await
            .map(Into::into)
            .map_err(|err| async_graphql::Error::new(err.to_string()))
    }

    async fn delete_site_symbol(
        &self,
        ctx: &Context<'_>,
        symbol_id: String,
        locale: String,
        tenant_id: Option<Uuid>,
    ) -> Result<bool> {
        require_module_enabled(ctx, "pages").await?;
        let (tenant_id, security) = scope(ctx, tenant_id, Permission::PAGES_MANAGE)?;
        SiteSymbolService::new(ctx.data::<DatabaseConnection>()?.clone())
            .delete(tenant_id, &security, &symbol_id, &locale)
            .await
            .map(|()| true)
            .map_err(|err| async_graphql::Error::new(err.to_string()))
    }
}
