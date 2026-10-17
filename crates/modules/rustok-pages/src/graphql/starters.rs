//! GraphQL query and mutation surface for starter presets.
use async_graphql::{Context, InputObject, Object, Result, SimpleObject};
use rustok_api::{
    AuthContext, Permission, TenantContext, graphql::require_module_enabled,
    has_any_effective_permission,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{PageStarterRecord, PageStarterService, SavePageStarterInput};

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlPageStarter {
    pub id: Uuid,
    pub key: String,
    pub category: String,
    pub name: String,
    pub description: Option<String>,
    pub thumbnail_url: Option<String>,
    pub default_template_key: String,
    pub content_json: String,
}

impl From<PageStarterRecord> for GqlPageStarter {
    fn from(value: PageStarterRecord) -> Self {
        Self {
            id: value.id,
            key: value.key,
            category: value.category,
            name: value.name,
            description: value.description,
            thumbnail_url: value.thumbnail_url,
            default_template_key: value.default_template_key,
            content_json: serde_json::to_string(&value.content).unwrap_or_default(),
        }
    }
}

#[derive(Clone, Debug, InputObject)]
pub struct SaveGqlPageStarterInput {
    pub key: String,
    pub category: String,
    pub name: String,
    pub description: Option<String>,
    pub thumbnail_url: Option<String>,
    pub default_template_key: Option<String>,
    pub content_json: String,
}

fn scope(
    ctx: &Context<'_>,
    requested: Option<Uuid>,
    permission: Permission,
) -> Result<(Uuid, rustok_core::SecurityContext)> {
    let auth = ctx.data::<AuthContext>().map_err(|_| {
        async_graphql::Error::new("Pages starter authoring requires authentication")
    })?;
    let tenant = ctx.data::<TenantContext>()?;
    if auth.tenant_id != tenant.id || requested.is_some_and(|id| id != tenant.id) {
        return Err(async_graphql::Error::new(
            "Pages starter authoring must use the current tenant",
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
pub struct PageStarterQuery;

#[Object]
impl PageStarterQuery {
    async fn page_starters(
        &self,
        ctx: &Context<'_>,
        category: Option<String>,
        tenant_id: Option<Uuid>,
    ) -> Result<Vec<GqlPageStarter>> {
        require_module_enabled(ctx, "pages").await?;
        let (tenant_id, security) = scope(ctx, tenant_id, Permission::PAGES_READ)?;
        PageStarterService::new(ctx.data::<DatabaseConnection>()?.clone())
            .list(tenant_id, &security, category.as_deref())
            .await
            .map(|rows| rows.into_iter().map(Into::into).collect())
            .map_err(|err| async_graphql::Error::new(err.to_string()))
    }

    async fn page_starter(
        &self,
        ctx: &Context<'_>,
        key: String,
        tenant_id: Option<Uuid>,
    ) -> Result<Option<GqlPageStarter>> {
        require_module_enabled(ctx, "pages").await?;
        let (tenant_id, security) = scope(ctx, tenant_id, Permission::PAGES_READ)?;
        PageStarterService::new(ctx.data::<DatabaseConnection>()?.clone())
            .get(tenant_id, &security, &key)
            .await
            .map(|opt| opt.map(Into::into))
            .map_err(|err| async_graphql::Error::new(err.to_string()))
    }
}

#[derive(Default)]
pub struct PageStarterMutation;

#[Object]
impl PageStarterMutation {
    async fn save_page_starter(
        &self,
        ctx: &Context<'_>,
        input: SaveGqlPageStarterInput,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlPageStarter> {
        require_module_enabled(ctx, "pages").await?;
        let (tenant_id, security) = scope(ctx, tenant_id, Permission::PAGES_MANAGE)?;
        let content: serde_json::Value = serde_json::from_str(&input.content_json)
            .map_err(|err| async_graphql::Error::new(format!("Invalid starter JSON: {err}")))?;

        PageStarterService::new(ctx.data::<DatabaseConnection>()?.clone())
            .save(
                tenant_id,
                &security,
                SavePageStarterInput {
                    key: input.key,
                    category: input.category,
                    name: input.name,
                    description: input.description,
                    thumbnail_url: input.thumbnail_url,
                    default_template_key: input.default_template_key,
                    content,
                },
            )
            .await
            .map(Into::into)
            .map_err(|err| async_graphql::Error::new(err.to_string()))
    }

    /// Clones and re-maps all identifiers of a starter into a fresh page document ready for `createPage`.
    async fn instantiate_page_starter(
        &self,
        ctx: &Context<'_>,
        key: String,
        tenant_id: Option<Uuid>,
    ) -> Result<String> {
        require_module_enabled(ctx, "pages").await?;
        let (tenant_id, security) = scope(ctx, tenant_id, Permission::PAGES_READ)?;
        let starter = PageStarterService::new(ctx.data::<DatabaseConnection>()?.clone())
            .get(tenant_id, &security, &key)
            .await
            .map_err(|err| async_graphql::Error::new(err.to_string()))?
            .ok_or_else(|| async_graphql::Error::new("Starter preset not found"))?;

        PageStarterService::instantiate_starter(&starter)
            .map_err(|err| async_graphql::Error::new(err.to_string()))
    }
}
