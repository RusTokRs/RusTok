//! Shared host adapter for localized, authorized artifact UI projections.
//!
//! HTTP and GraphQL use this one adapter so neither transport can expose an
//! admitted contribution that the caller cannot render or whose exact locale
//! is unavailable. The artifact owner retains descriptor and admission state;
//! this adapter supplies only host RBAC enforcement and transport-safe errors.

use rustok_api::{ArtifactBindingExecutionAuditEntry, ArtifactUiContributionView};
use rustok_modules::{
    ArtifactUiProjectionError, InstalledModuleArtifact, ModuleControlPlane,
    find_artifact_ui_action_binding,
};
use rustok_rbac::SeaOrmArtifactPermissionAuthorizer;
use uuid::Uuid;

use crate::{
    error::{Error, Result, http_error},
    services::{
        artifact_binding::{ArtifactBindingOperation, dispatch_artifact_binding_operation},
        server_runtime_context::ServerRuntimeContext,
    },
};

/// Resolves the one active installation permitted to serve a tenant route.
pub(crate) async fn resolve_artifact_installation(
    ctx: &ServerRuntimeContext,
    installation_id: Uuid,
    tenant_id: Uuid,
) -> Result<InstalledModuleArtifact> {
    ModuleControlPlane::new(ctx.db_clone())
        .installation()
        .resolve_routed_installation(installation_id, tenant_id)
        .await
        .map_err(|_| Error::NotFound)
}

/// Checks the dynamic, admitted permission required for one artifact surface.
pub(crate) async fn is_artifact_permission_authorized(
    ctx: &ServerRuntimeContext,
    tenant_id: Uuid,
    actor_id: Uuid,
    installation: &InstalledModuleArtifact,
    permission: &str,
) -> Result<bool> {
    SeaOrmArtifactPermissionAuthorizer::new(ctx.db_clone())
        .is_authorized(
            tenant_id,
            actor_id,
            installation.installation_id,
            permission,
        )
        .await
        .map_err(|error| {
            tracing::error!(%error, "artifact UI RBAC authorization failed");
            Error::InternalServerError
        })
}

/// Enforces an admitted artifact permission with the same opaque forbidden
/// response for every server transport.
pub(crate) async fn require_artifact_permission(
    ctx: &ServerRuntimeContext,
    tenant_id: Uuid,
    actor_id: Uuid,
    installation: &InstalledModuleArtifact,
    permission: &str,
) -> Result<()> {
    if is_artifact_permission_authorized(ctx, tenant_id, actor_id, installation, permission).await?
    {
        Ok(())
    } else {
        Err(http_error(rustok_web::HttpError::forbidden(
            "forbidden",
            "Permission denied for artifact binding",
        )))
    }
}

/// Returns only localized contributions that the actor may render. There is
/// no locale fallback: a catalog gap omits that contribution fail-closed.
pub(crate) async fn list_authorized_artifact_ui_contributions(
    ctx: &ServerRuntimeContext,
    tenant_id: Uuid,
    actor_id: Uuid,
    installation_id: Uuid,
    effective_locale: &str,
) -> Result<Vec<ArtifactUiContributionView>> {
    let installation = resolve_artifact_installation(ctx, installation_id, tenant_id).await?;
    let mut contributions = Vec::with_capacity(installation.descriptor.ui_contributions.len());

    for contribution in &installation.descriptor.ui_contributions {
        if !is_artifact_permission_authorized(
            ctx,
            tenant_id,
            actor_id,
            &installation,
            &contribution.permission,
        )
        .await?
        {
            continue;
        }

        match installation
            .descriptor
            .project_ui_contribution(&contribution.id, effective_locale)
        {
            Ok(contribution) => contributions.push(contribution),
            Err(ArtifactUiProjectionError::LocaleUnavailable) => {}
            Err(error) => {
                tracing::error!(
                    %error,
                    installation_id = %installation.installation_id,
                    contribution_id = %contribution.id,
                    "admitted artifact UI projection failed"
                );
                return Err(Error::InternalServerError);
            }
        }
    }

    Ok(contributions)
}

/// Executes only the admitted Command binding selected by one Action or Form
/// contribution. The caller cannot name a raw binding; HTTP and GraphQL share
/// this path, including dynamic RBAC and durable binding idempotency.
pub(crate) async fn execute_artifact_ui_action(
    ctx: &ServerRuntimeContext,
    tenant_id: Uuid,
    actor_id: Uuid,
    installation_id: Uuid,
    contribution_id: &str,
    input: serde_json::Value,
    idempotency_key: Option<Uuid>,
) -> Result<serde_json::Value> {
    let installation = resolve_artifact_installation(ctx, installation_id, tenant_id).await?;
    let binding = find_artifact_ui_action_binding(
        &installation.descriptor.ui_contributions,
        &installation.descriptor.bindings,
        contribution_id,
    )
    .ok_or(Error::NotFound)?;
    dispatch_artifact_binding_operation(
        ctx,
        tenant_id,
        actor_id,
        &installation,
        binding,
        idempotency_key,
        ArtifactBindingOperation::Command {
            binding_id: binding.id.clone(),
            input,
        },
    )
    .await
}

/// Returns redacted evidence for only the admitted binding selected by one
/// Action or Form contribution. This shares the execution authorization path
/// and never exposes a caller-selectable binding identity.
pub(crate) async fn list_authorized_artifact_ui_action_audit(
    ctx: &ServerRuntimeContext,
    tenant_id: Uuid,
    actor_id: Uuid,
    installation_id: Uuid,
    contribution_id: &str,
) -> Result<Vec<ArtifactBindingExecutionAuditEntry>> {
    let installation = resolve_artifact_installation(ctx, installation_id, tenant_id).await?;
    let binding = find_artifact_ui_action_binding(
        &installation.descriptor.ui_contributions,
        &installation.descriptor.bindings,
        contribution_id,
    )
    .ok_or(Error::NotFound)?;
    require_artifact_permission(ctx, tenant_id, actor_id, &installation, &binding.permission)
        .await?;
    ModuleControlPlane::new(ctx.db_clone())
        .artifact_binding_execution_audit()
        .list(tenant_id, installation.installation_id, &binding.id, 50)
        .await
        .map_err(|error| {
            tracing::error!(%error, "artifact UI action audit lookup failed");
            Error::InternalServerError
        })
}
