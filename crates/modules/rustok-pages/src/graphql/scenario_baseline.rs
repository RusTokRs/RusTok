use async_graphql::{Context, FieldError, InputObject, Object, Result, SimpleObject};
use rustok_api::{
    AuthContext, Permission, TenantContext,
    graphql::{GraphQLError, require_module_enabled},
    has_any_effective_permission,
};
use rustok_page_builder::runtime_scenario_release::{
    RuntimeScenarioReleaseBaseline, RuntimeScenarioReleaseEvaluation, RuntimeScenarioReleaseStatus,
    RuntimeScenarioRenderChangeImpact,
};
use sea_orm::DatabaseConnection;
use serde_json::Value;
use uuid::Uuid;

use crate::{PageBuilderScenarioBaselineService, SaveIfCurrentScenarioBaselineRequest};

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

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlPageBuilderScenarioReleaseStatus {
    pub page_id: Uuid,
    pub baseline_present: bool,
    pub allowed: bool,
    pub status: String,
    pub baseline_id: Option<String>,
    pub baseline_hash: Option<String>,
    pub visual_changes: i32,
    pub breaking_changes: i32,
    pub diagnostics: Value,
}

impl GqlPageBuilderScenarioReleaseStatus {
    fn not_configured(page_id: Uuid) -> Self {
        Self {
            page_id,
            baseline_present: false,
            allowed: true,
            status: "not_configured".to_string(),
            baseline_id: None,
            baseline_hash: None,
            visual_changes: 0,
            breaking_changes: 0,
            diagnostics: Value::Array(Vec::new()),
        }
    }

    fn from_evaluation(
        page_id: Uuid,
        evaluation: RuntimeScenarioReleaseEvaluation,
    ) -> Result<Self> {
        let visual_changes = evaluation
            .diff
            .as_ref()
            .map(|diff| {
                diff.changes
                    .iter()
                    .filter(|change| change.impact() == RuntimeScenarioRenderChangeImpact::Visual)
                    .count()
            })
            .unwrap_or_default();
        let breaking_changes = evaluation
            .diff
            .as_ref()
            .map(|diff| {
                diff.changes
                    .iter()
                    .filter(|change| change.impact() == RuntimeScenarioRenderChangeImpact::Breaking)
                    .count()
            })
            .unwrap_or_default();
        let diagnostics = serde_json::to_value(&evaluation.diagnostics)
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        Ok(Self {
            page_id,
            baseline_present: evaluation.baseline_id.is_some(),
            allowed: evaluation.allowed,
            status: release_status_label(evaluation.status).to_string(),
            baseline_id: evaluation.baseline_id,
            baseline_hash: evaluation.baseline_hash,
            visual_changes: i32::try_from(visual_changes).unwrap_or(i32::MAX),
            breaking_changes: i32::try_from(breaking_changes).unwrap_or(i32::MAX),
            diagnostics,
        })
    }
}

#[derive(InputObject)]
pub struct SaveGqlPageBuilderScenarioBaselineInput {
    pub baseline: Value,
    pub expected_baseline_hash: Option<String>,
    pub promotion_note: Option<String>,
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

    async fn page_builder_scenario_release_status(
        &self,
        ctx: &Context<'_>,
        page_id: Uuid,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlPageBuilderScenarioReleaseStatus> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_pages_permission(ctx, Permission::PAGES_READ)?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = current_tenant_id(tenant, &auth, tenant_id)?;
        let service = PageBuilderScenarioBaselineService::new(db.clone());
        let baseline = service
            .get(tenant_id, page_security(&auth), page_id)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        if baseline.is_none() {
            return Ok(GqlPageBuilderScenarioReleaseStatus::not_configured(page_id));
        }
        let evaluation = service
            .evaluate_publish(tenant_id, page_id)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?
            .ok_or_else(|| async_graphql::Error::new("Scenario baseline disappeared"))?;
        GqlPageBuilderScenarioReleaseStatus::from_evaluation(page_id, evaluation)
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
            .save_if_current(SaveIfCurrentScenarioBaselineRequest {
                tenant_id,
                security: page_security(&auth),
                page_id,
                baseline,
                expected_baseline_hash: input.expected_baseline_hash,
                promoted_by: auth.user_id,
                promotion_note: input.promotion_note,
            })
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        GqlPageBuilderScenarioBaseline::from_baseline(page_id, baseline)
    }

    async fn delete_page_builder_scenario_baseline(
        &self,
        ctx: &Context<'_>,
        page_id: Uuid,
        expected_baseline_hash: Option<String>,
        tenant_id: Option<Uuid>,
    ) -> Result<bool> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_pages_permission(ctx, Permission::PAGES_UPDATE)?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = current_tenant_id(tenant, &auth, tenant_id)?;
        PageBuilderScenarioBaselineService::new(db.clone())
            .delete_if_current(
                tenant_id,
                page_security(&auth),
                page_id,
                expected_baseline_hash.as_deref(),
            )
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))
    }
}

fn release_status_label(status: RuntimeScenarioReleaseStatus) -> &'static str {
    match status {
        RuntimeScenarioReleaseStatus::Disabled => "disabled",
        RuntimeScenarioReleaseStatus::BaselineMissing => "baseline_missing",
        RuntimeScenarioReleaseStatus::BaselineInvalid => "baseline_invalid",
        RuntimeScenarioReleaseStatus::Stable => "stable",
        RuntimeScenarioReleaseStatus::RequiresReview => "requires_review",
        RuntimeScenarioReleaseStatus::Broken => "broken",
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
