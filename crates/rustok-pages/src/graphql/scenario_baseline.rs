use async_graphql::{Context, FieldError, InputObject, Object, Result, SimpleObject};
use rustok_api::{
    graphql::{require_module_enabled, GraphQLError}, has_any_effective_permission, AuthContext,
    Permission, TenantContext,
};
use rustok_page_builder::runtime_scenario_release::RuntimeScenarioReleaseBaseline;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use uuid::Uuid;

use crate::PageBuilderScenarioBaselineService;

const MODULE_SLUG: &str = "pages";

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlPageBuilderScenarioBaseline {
    pub page_id: Uuid,
    pub baseline_id: String,
    pub baseline_hash: String,
    pub source_project_hash: String,
    pub baseline: Value,
}

impl GqlPageBuilderScenarioBaseline {
    fn from_baseline(page_id: Uuid, baseline: RuntimeScenarioReleaseBaseline) -> Result<Self> {
        let baseline_json = serde_json::to_value(&baseline)
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        Ok(Self {
            page_id,
            baseline_id: baseline.baseline_id,
            baseline_hash: baseline.baseline_hash,
            source_project_hash: baseline.source_project_hash,
            baseline: baseline_json,
        })
    }
}

#[derive(InputObject)]
pub struct SaveGqlPageBuilderScenarioBaselineInput {
    pub baseline: Value,
}

#[derive(Default)]
pub struct PageBuilderScenarioBaselineQuery;

#[Object]
impl PageBuilderScenarioBaselineQuery {
    async fn page_builder_scenario_baseline(
        &self,
        ctx: &Context<'_>,
        page_id: Uuid,
        tenant_id: Option<Uuid>,
    ) -> Result<Option<GqlPageBuilderScenarioBaseline>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_pages_permission(ctx, Permission::PAGES_READ)?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = current_tenant_id(tenant, &auth, tenant_id)?;
        let service = PageBuilderScenarioBaselineService::new(db.clone());
        service
            .get(tenant_id, page_security(&auth), page_id)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?
            .map(|baseline| GqlPageBuilderScenarioBaseline::from_baseline(page_id, baseline))
            .transpose()
    }
}

#[derive(Default)]
pub struct PageBuilderScenarioBaselineMutation;

#[Object]
impl PageBuilderScenarioBaselineMutation {
    async fn save_page_builder_scenario_baseline(
        &self,
        ctx: &Context<'_>,
        page_id: Uuid,
        input: SaveGqlPageBuilderScenarioBaselineInput,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlPageBuilderScenarioBaseline> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_pages_permission(ctx, Permission::PAGES_UPDATE)?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = current_tenant_id(tenant, &auth, tenant_id)?;
        let baseline = serde_json::from_value::<RuntimeScenarioReleaseBaseline>(input.baseline)
            .map_err(|error| {
                async_graphql::Error::new(format!(
                    "Invalid Page Builder scenario release baseline: {error}"
                ))
            })?;
        let service = PageBuilderScenarioBaselineService::new(db.clone());
        let baseline = service
            .save(tenant_id, page_security(&auth), page_id, baseline)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        GqlPageBuilderScenarioBaseline::from_baseline(page_id, baseline)
    }

    async fn delete_page_builder_scenario_baseline(
        &self,
        ctx: &Context<'_>,
        page_id: Uuid,
        tenant_id: Option<Uuid>,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_pages_permission(ctx, Permission::PAGES_UPDATE)?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = current_tenant_id(tenant, &auth, tenant_id)?;
        PageBuilderScenarioBaselineService::new(db.clone())
            .delete(tenant_id, page_security(&auth), page_id)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))
    }
}

fn page_security(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    )
}

fn current_tenant_id(
    tenant: &TenantContext,
    auth: &AuthContext,
    requested: Option<Uuid>,
) -> Result<Uuid> {
    if auth.tenant_id != tenant.id {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Authenticated actor is not bound to the current tenant",
        ));
    }
    if requested.is_some_and(|requested| requested != tenant.id) {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Page Builder scenario baseline operations must use the current tenant",
        ));
    }
    Ok(tenant.id)
}

fn require_pages_permission(ctx: &Context<'_>, permission: Permission) -> Result<AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?
        .clone();
    if !has_any_effective_permission(&auth.permissions, &[permission]) {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Permission denied: pages read/update authority required",
        ));
    }
    Ok(auth)
}
