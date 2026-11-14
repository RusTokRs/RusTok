//! GraphQL query and mutation surface for starter presets.
use async_graphql::{Context, ErrorExtensions, InputObject, Object, Result, SimpleObject};
use rustok_api::{
    AuthContext, Permission, TenantContext, graphql::require_module_enabled,
    has_any_effective_permission,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{PageStarterRecord, PageStarterService, PagesError, SavePageStarterInput};
use crate::services::page_starter::{SavePageStarterFromPageInput, UpdatePageStarterMetadataInput};

fn starter_error(error: PagesError) -> async_graphql::Error {
    let code = match &error {
        PagesError::StarterRevisionConflict => "PAGE_STARTER_REVISION_CONFLICT",
        PagesError::Forbidden(_) => "PAGES_PERMISSION_DENIED",
        PagesError::Validation(_) => "PAGE_STARTER_INVALID",
        _ => "PAGES_OPERATION_FAILED",
    };
    async_graphql::Error::new(error.to_string()).extend_with(|_, ext| ext.set("code", code))
}

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
    /// Null for built-ins; tenant records require this token for update or delete.
    pub revision: Option<Uuid>,
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
            revision: value.revision,
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
    pub expected_revision: Option<Uuid>,
}

#[derive(Clone, Debug, InputObject)]
pub struct UpdateGqlPageStarterMetadataInput {
    pub key: String,
    pub category: String,
    pub name: String,
    pub description: Option<String>,
    pub default_template_key: String,
    pub expected_revision: Uuid,
}

#[derive(Clone, Debug, InputObject)]
pub struct SavePageStarterFromPageGqlInput {
    pub page_id: Uuid,
    pub locale: String,
    pub key: String,
    pub category: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlSavedPageStarterSnapshot {
    pub starter: GqlPageStarter,
    pub source_locale: String,
    pub source_state: String,
    pub source_revision: String,
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
            .map_err(starter_error)
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
            .map_err(starter_error)
    }
}

#[derive(Default)]
pub struct PageStarterMutation;

#[Object]
impl PageStarterMutation {
    /// Captures the saved exact-locale working copy, not unsaved editor state.
    async fn save_page_starter_from_page(
        &self,
        ctx: &Context<'_>,
        input: SavePageStarterFromPageGqlInput,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlSavedPageStarterSnapshot> {
        require_module_enabled(ctx, "pages").await?;
        let (tenant_id, security) = scope(ctx, tenant_id, Permission::PAGES_MANAGE)?;
        let snapshot = PageStarterService::new(ctx.data::<DatabaseConnection>()?.clone())
            .save_from_page(tenant_id, &security, SavePageStarterFromPageInput {
                page_id: input.page_id, locale: input.locale, key: input.key,
                category: input.category, name: input.name, description: input.description,
            }).await.map_err(starter_error)?;
        Ok(GqlSavedPageStarterSnapshot {
            starter: snapshot.starter.into(), source_locale: snapshot.source_locale,
            source_state: snapshot.source_state, source_revision: snapshot.source_revision,
        })
    }

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
                    expected_revision: input.expected_revision,
                },
            )
            .await
            .map(Into::into)
            .map_err(starter_error)
    }

    /// Updates metadata only, at an observed tenant revision, without rewriting the document.
    async fn update_page_starter_metadata(
        &self,
        ctx: &Context<'_>,
        input: UpdateGqlPageStarterMetadataInput,
        tenant_id: Option<Uuid>,
    ) -> Result<Uuid> {
        require_module_enabled(ctx, "pages").await?;
        let (tenant_id, security) = scope(ctx, tenant_id, Permission::PAGES_MANAGE)?;
        PageStarterService::new(ctx.data::<DatabaseConnection>()?.clone())
            .update_metadata(tenant_id, &security, UpdatePageStarterMetadataInput {
                key: input.key, category: input.category, name: input.name,
                description: input.description, default_template_key: input.default_template_key,
                expected_revision: input.expected_revision,
            }).await.map_err(starter_error)
    }

    /// Deletes a tenant override only if its observed revision still matches.
    async fn delete_page_starter(
        &self,
        ctx: &Context<'_>,
        key: String,
        expected_revision: Uuid,
        tenant_id: Option<Uuid>,
    ) -> Result<bool> {
        require_module_enabled(ctx, "pages").await?;
        let (tenant_id, security) = scope(ctx, tenant_id, Permission::PAGES_MANAGE)?;
        PageStarterService::new(ctx.data::<DatabaseConnection>()?.clone())
            .delete(tenant_id, &security, &key, expected_revision)
            .await
            .map(|()| true)
            .map_err(starter_error)
    }

    /// Clone the selected built-in (null revision) or exact tenant revision for the page locale.
    /// A changed or replaced selection fails with `PAGE_STARTER_REVISION_CONFLICT`.
    /// Linked site symbols require pages:manage and must resolve in the exact locale.
    async fn instantiate_page_starter(
        &self,
        ctx: &Context<'_>,
        key: String,
        expected_revision: Option<Uuid>,
        locale: String,
        tenant_id: Option<Uuid>,
    ) -> Result<String> {
        require_module_enabled(ctx, "pages").await?;
        let (tenant_id, security) = scope(ctx, tenant_id, Permission::PAGES_READ)?;
        PageStarterService::new(ctx.data::<DatabaseConnection>()?.clone())
            .instantiate_selected(tenant_id, &security, &key, expected_revision, &locale)
            .await
            .map_err(starter_error)
    }
}
