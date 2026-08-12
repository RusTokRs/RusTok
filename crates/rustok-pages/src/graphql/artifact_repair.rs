use async_graphql::{
    Context, ErrorExtensions, FieldError, InputObject, Object, Result, SimpleObject,
};
use rustok_api::{
    Action, AuthContext, Permission, Resource, TenantContext,
    graphql::{GraphQLError, require_module_enabled},
    has_any_effective_permission,
};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    ActivateRebuiltPageArtifactTransportResult, PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT,
    PAGE_ARTIFACT_BINDING_REPLACEMENT_IDEMPOTENCY_CONFLICT,
    PAGE_ARTIFACT_BINDING_REPLACEMENT_OPERATION_INTEGRITY,
    PAGE_ARTIFACT_BINDING_REPLACEMENT_TARGET_INVALID, PAGE_ARTIFACT_REBUILD_IDEMPOTENCY_CONFLICT,
    PAGE_ARTIFACT_REBUILD_OPERATION_INTEGRITY, PAGE_ARTIFACT_REBUILD_SOURCE_INVALID, PageService,
    PagesError, RebuildPageArtifactInput, RebuildPageArtifactTransportResult,
    ReplacePageArtifactBindingInput, ReviewedPagePublishRuntimeInput,
};

use super::types::ReviewedGqlPagePublishRuntimeInput;

const MODULE_SLUG: &str = "pages";
const PAGES_PERMISSION_DENIED: &str = "PAGES_PERMISSION_DENIED";
const PAGE_ARTIFACT_REPAIR_INVALID_INPUT: &str = "PAGE_ARTIFACT_REPAIR_INVALID_INPUT";
const PAGE_ARTIFACT_REBUILD_RUNTIME_REVIEW_REJECTED: &str =
    "PAGE_ARTIFACT_REBUILD_RUNTIME_REVIEW_REJECTED";
const PAGE_ARTIFACT_REBUILD_REPRODUCTION_MISMATCH: &str =
    "PAGE_ARTIFACT_REBUILD_REPRODUCTION_MISMATCH";
const PAGE_ARTIFACT_REBUILD_FAILED: &str = "PAGE_ARTIFACT_REBUILD_FAILED";
const PAGE_ARTIFACT_BINDING_REPLACEMENT_VERSION_CONFLICT: &str =
    "PAGE_ARTIFACT_BINDING_REPLACEMENT_VERSION_CONFLICT";
const PAGE_ARTIFACT_BINDING_REPLACEMENT_FAILED: &str = "PAGE_ARTIFACT_BINDING_REPLACEMENT_FAILED";

#[derive(Default)]
pub(crate) struct PageArtifactRepairMutation;

#[Object]
impl PageArtifactRepairMutation {
    async fn rebuild_page_artifact(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: RebuildGqlPageArtifactInput,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlRebuildPageArtifactResult> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_pages_manage_permission(ctx)?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_current_tenant(tenant, &auth, tenant_id)?;

        let result = PageService::new(db.clone(), event_bus.clone())
            .rebuild_immutable_artifact(
                tenant_id,
                page_security(&auth),
                id,
                RebuildPageArtifactInput {
                    source_id: input.source_id,
                    expected_provenance_hash: input.expected_provenance_hash,
                    idempotency_key: input.idempotency_key,
                    runtime: reviewed_runtime_input(input.runtime),
                },
            )
            .await
            .map_err(map_rebuild_error)?;
        Ok(RebuildPageArtifactTransportResult::from(result).into())
    }

    async fn activate_rebuilt_page_artifact(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: ActivateGqlRebuiltPageArtifactInput,
        tenant_id: Option<Uuid>,
    ) -> Result<GqlActivateRebuiltPageArtifactResult> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_pages_manage_permission(ctx)?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_current_tenant(tenant, &auth, tenant_id)?;

        let result = PageService::new(db.clone(), event_bus.clone())
            .replace_rebuilt_artifact_binding(
                tenant_id,
                page_security(&auth),
                id,
                ReplacePageArtifactBindingInput {
                    rebuild_operation_id: input.rebuild_operation_id,
                    expected_version: input.expected_version,
                    expected_current_artifact_id: input.expected_current_artifact_id,
                    idempotency_key: input.idempotency_key,
                },
            )
            .await
            .map_err(map_activation_error)?;
        Ok(ActivateRebuiltPageArtifactTransportResult::from(result).into())
    }
}

#[derive(InputObject)]
pub struct RebuildGqlPageArtifactInput {
    pub source_id: Uuid,
    pub expected_provenance_hash: String,
    pub idempotency_key: String,
    pub runtime: ReviewedGqlPagePublishRuntimeInput,
}

#[derive(InputObject)]
pub struct ActivateGqlRebuiltPageArtifactInput {
    pub rebuild_operation_id: Uuid,
    pub expected_version: i32,
    pub expected_current_artifact_id: Uuid,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlRebuildPageArtifactResult {
    pub operation_id: Uuid,
    pub page_id: Uuid,
    pub locale: String,
    pub source_artifact_id: Uuid,
    pub rebuilt_artifact_id: Uuid,
    pub artifact_hash: String,
    pub materialization_hash: String,
    pub replayed: bool,
    pub rebuilt_at: String,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlActivateRebuiltPageArtifactResult {
    pub operation_id: Uuid,
    pub page_id: Uuid,
    pub version: i32,
    pub locale: String,
    pub rebuild_operation_id: Uuid,
    pub previous_artifact_id: Uuid,
    pub replacement_artifact_id: Uuid,
    pub replacement_artifact_hash: String,
    pub replacement_materialization_hash: String,
    pub replayed: bool,
    pub replaced_at: String,
}

impl From<RebuildPageArtifactTransportResult> for GqlRebuildPageArtifactResult {
    fn from(result: RebuildPageArtifactTransportResult) -> Self {
        Self {
            operation_id: result.operation_id,
            page_id: result.page_id,
            locale: result.locale,
            source_artifact_id: result.source_artifact_id,
            rebuilt_artifact_id: result.rebuilt_artifact_id,
            artifact_hash: result.artifact_hash,
            materialization_hash: result.materialization_hash,
            replayed: result.replayed,
            rebuilt_at: result.rebuilt_at,
        }
    }
}

impl From<ActivateRebuiltPageArtifactTransportResult> for GqlActivateRebuiltPageArtifactResult {
    fn from(result: ActivateRebuiltPageArtifactTransportResult) -> Self {
        Self {
            operation_id: result.operation_id,
            page_id: result.page_id,
            version: result.version,
            locale: result.locale,
            rebuild_operation_id: result.rebuild_operation_id,
            previous_artifact_id: result.previous_artifact_id,
            replacement_artifact_id: result.replacement_artifact_id,
            replacement_artifact_hash: result.replacement_artifact_hash,
            replacement_materialization_hash: result.replacement_materialization_hash,
            replayed: result.replayed,
            replaced_at: result.replaced_at,
        }
    }
}

fn reviewed_runtime_input(
    input: ReviewedGqlPagePublishRuntimeInput,
) -> ReviewedPagePublishRuntimeInput {
    ReviewedPagePublishRuntimeInput {
        format: input.format,
        scenario_id: input.scenario_id,
        context: input.context,
        review_hash: input.review_hash,
    }
}

fn require_pages_manage_permission(ctx: &Context<'_>) -> Result<AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?
        .clone();
    let permission = Permission::new(Resource::Pages, Action::Manage);
    if !has_any_effective_permission(&auth.permissions, &[permission]) {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Permission denied: pages:manage required",
        ));
    }
    Ok(auth)
}

fn resolve_current_tenant(
    tenant: &TenantContext,
    auth: &AuthContext,
    requested: Option<Uuid>,
) -> Result<Uuid> {
    if auth.tenant_id != tenant.id || requested.is_some_and(|id| id != tenant.id) {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Pages artifact repair mutations must use the current tenant",
        ));
    }
    Ok(tenant.id)
}

fn page_security(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    )
}

fn map_rebuild_error(error: PagesError) -> async_graphql::Error {
    match error {
        PagesError::PageNotFound(_) => public_graphql_error("PAGE_NOT_FOUND", "Page not found"),
        PagesError::Forbidden(_) => {
            public_graphql_error(PAGES_PERMISSION_DENIED, "Permission denied")
        }
        PagesError::Validation(_) => public_graphql_error(
            PAGE_ARTIFACT_REPAIR_INVALID_INPUT,
            "Invalid immutable artifact rebuild input",
        ),
        PagesError::PublishRuntimeReviewInvalid(_) => public_graphql_error(
            PAGE_ARTIFACT_REBUILD_RUNTIME_REVIEW_REJECTED,
            "Reviewed runtime does not match retained artifact provenance",
        ),
        PagesError::PublishIdempotencyConflict(_) => public_graphql_error(
            PAGE_ARTIFACT_REBUILD_IDEMPOTENCY_CONFLICT,
            "Artifact rebuild idempotency conflict",
        ),
        PagesError::PublishOperationIntegrity(detail)
            if detail.starts_with(PAGE_ARTIFACT_REBUILD_SOURCE_INVALID) =>
        {
            public_graphql_error(
                PAGE_ARTIFACT_REBUILD_SOURCE_INVALID,
                "Immutable artifact rebuild source is unavailable or invalid",
            )
        }
        PagesError::PublishOperationIntegrity(detail)
            if detail.starts_with(PAGE_ARTIFACT_REBUILD_OPERATION_INTEGRITY) =>
        {
            public_graphql_error(
                PAGE_ARTIFACT_REBUILD_OPERATION_INTEGRITY,
                "Stored artifact rebuild receipt failed integrity validation",
            )
        }
        PagesError::ArtifactIntegrity(_) => public_graphql_error(
            PAGE_ARTIFACT_REBUILD_REPRODUCTION_MISMATCH,
            "Immutable artifact could not be reproduced exactly",
        ),
        _ => public_graphql_error(
            PAGE_ARTIFACT_REBUILD_FAILED,
            "Immutable artifact rebuild failed",
        ),
    }
}

fn map_activation_error(error: PagesError) -> async_graphql::Error {
    match error {
        PagesError::PageNotFound(_) => public_graphql_error("PAGE_NOT_FOUND", "Page not found"),
        PagesError::Forbidden(_) => {
            public_graphql_error(PAGES_PERMISSION_DENIED, "Permission denied")
        }
        PagesError::Validation(_) => public_graphql_error(
            PAGE_ARTIFACT_REPAIR_INVALID_INPUT,
            "Invalid rebuilt artifact activation input",
        ),
        PagesError::VersionConflict { .. } => public_graphql_error(
            PAGE_ARTIFACT_BINDING_REPLACEMENT_VERSION_CONFLICT,
            "Page changed before rebuilt artifact activation",
        ),
        PagesError::RollbackIdempotencyConflict(_) => public_graphql_error(
            PAGE_ARTIFACT_BINDING_REPLACEMENT_IDEMPOTENCY_CONFLICT,
            "Artifact activation idempotency conflict",
        ),
        PagesError::RollbackTargetUnavailable(detail)
            if detail.starts_with(PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT) =>
        {
            public_graphql_error(
                PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT,
                "Current artifact binding no longer matches the activation request",
            )
        }
        PagesError::RollbackTargetUnavailable(detail)
            if detail.starts_with(PAGE_ARTIFACT_BINDING_REPLACEMENT_TARGET_INVALID) =>
        {
            public_graphql_error(
                PAGE_ARTIFACT_BINDING_REPLACEMENT_TARGET_INVALID,
                "Rebuilt artifact activation target is unavailable or invalid",
            )
        }
        PagesError::RollbackOperationIntegrity(detail)
            if detail.starts_with(PAGE_ARTIFACT_BINDING_REPLACEMENT_OPERATION_INTEGRITY) =>
        {
            public_graphql_error(
                PAGE_ARTIFACT_BINDING_REPLACEMENT_OPERATION_INTEGRITY,
                "Stored artifact activation receipt failed integrity validation",
            )
        }
        PagesError::ArtifactIntegrity(_) => public_graphql_error(
            PAGE_ARTIFACT_BINDING_REPLACEMENT_TARGET_INVALID,
            "Rebuilt artifact activation target is unavailable or invalid",
        ),
        _ => public_graphql_error(
            PAGE_ARTIFACT_BINDING_REPLACEMENT_FAILED,
            "Rebuilt artifact activation failed",
        ),
    }
}

fn public_graphql_error(code: &'static str, message: &'static str) -> async_graphql::Error {
    async_graphql::Error::new(message).extend_with(|_, extensions| {
        extensions.set("code", code);
    })
}
