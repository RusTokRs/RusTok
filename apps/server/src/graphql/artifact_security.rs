use async_graphql::{Context, ErrorExtensions, FieldError, Object, Result, SimpleObject};
use rustok_api::{AuthContext, AuthPrincipalContext, graphql::GraphQLError};
use rustok_modules::{ArtifactReleaseRef, ModuleArtifactSecurityError, ModuleControlPlane};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    context::TenantContext,
    services::{
        artifact_security::{
            ArtifactSecurityAuthorizationError, ArtifactSecurityReceiptView,
            ArtifactSecuritySnapshotView, ServerArtifactSecurityAuthorizer,
            authorize_artifact_security_operator, build_module_artifact_security_command,
        },
        server_runtime_context::ServerRuntimeContext,
    },
};

#[derive(Default)]
pub struct ArtifactSecurityQuery;

#[derive(Default)]
pub struct ArtifactSecurityMutation;

#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ArtifactSecuritySnapshot")]
pub struct ArtifactSecuritySnapshotGql {
    module_slug: String,
    version: String,
    payload_digest: String,
    revision: String,
    status: String,
    registry_status: String,
    policy_revision: Option<String>,
    reason_code: Option<String>,
    reason_detail: Option<String>,
}

#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ArtifactSecurityReceipt")]
pub struct ArtifactSecurityReceiptGql {
    snapshot: ArtifactSecuritySnapshotGql,
    created: bool,
}

#[derive(Clone, Copy)]
enum SecurityAction {
    Quarantine,
    ClearQuarantine,
    Revoke,
}

#[Object]
impl ArtifactSecurityQuery {
    async fn module_artifact_security(
        &self,
        ctx: &Context<'_>,
        module_slug: String,
        version: String,
        payload_digest: String,
    ) -> Result<ArtifactSecuritySnapshotGql> {
        let (auth, principal, tenant_id) = authorized_request(ctx)?;
        let db = ctx.data::<DatabaseConnection>()?;
        authorize_artifact_security_operator(db, &auth, principal, tenant_id)
            .await
            .map_err(map_authorization_error)?;

        let snapshot = ModuleControlPlane::new(db.clone())
            .artifact_security_resolver()
            .resolve(&ArtifactReleaseRef {
                slug: module_slug,
                version,
                digest: payload_digest,
            })
            .await
            .map_err(map_artifact_security_error)?;

        Ok(snapshot_gql(ArtifactSecuritySnapshotView::from(snapshot)))
    }
}

#[Object]
impl ArtifactSecurityMutation {
    async fn quarantine_module_artifact(
        &self,
        ctx: &Context<'_>,
        module_slug: String,
        version: String,
        payload_digest: String,
        expected_revision: String,
        policy_revision: String,
        reason_code: String,
        reason_detail: String,
        idempotency_key: Uuid,
    ) -> Result<ArtifactSecurityReceiptGql> {
        execute_security_command(
            ctx,
            module_slug,
            version,
            payload_digest,
            expected_revision,
            policy_revision,
            reason_code,
            reason_detail,
            idempotency_key,
            SecurityAction::Quarantine,
        )
        .await
    }

    async fn clear_module_artifact_quarantine(
        &self,
        ctx: &Context<'_>,
        module_slug: String,
        version: String,
        payload_digest: String,
        expected_revision: String,
        policy_revision: String,
        reason_code: String,
        reason_detail: String,
        idempotency_key: Uuid,
    ) -> Result<ArtifactSecurityReceiptGql> {
        execute_security_command(
            ctx,
            module_slug,
            version,
            payload_digest,
            expected_revision,
            policy_revision,
            reason_code,
            reason_detail,
            idempotency_key,
            SecurityAction::ClearQuarantine,
        )
        .await
    }

    async fn revoke_module_artifact(
        &self,
        ctx: &Context<'_>,
        module_slug: String,
        version: String,
        payload_digest: String,
        expected_revision: String,
        policy_revision: String,
        reason_code: String,
        reason_detail: String,
        idempotency_key: Uuid,
    ) -> Result<ArtifactSecurityReceiptGql> {
        execute_security_command(
            ctx,
            module_slug,
            version,
            payload_digest,
            expected_revision,
            policy_revision,
            reason_code,
            reason_detail,
            idempotency_key,
            SecurityAction::Revoke,
        )
        .await
    }
}

fn authorized_request(ctx: &Context<'_>) -> Result<(AuthContext, AuthPrincipalContext, Uuid)> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?
        .clone();
    let principal = *ctx
        .data::<AuthPrincipalContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    let tenant_id = ctx.data::<TenantContext>()?.id;
    Ok((auth, principal, tenant_id))
}

#[allow(clippy::too_many_arguments)]
async fn execute_security_command(
    ctx: &Context<'_>,
    module_slug: String,
    version: String,
    payload_digest: String,
    expected_revision: String,
    policy_revision: String,
    reason_code: String,
    reason_detail: String,
    idempotency_key: Uuid,
    action: SecurityAction,
) -> Result<ArtifactSecurityReceiptGql> {
    let expected_revision = expected_revision.parse::<u64>().map_err(|_| {
        <FieldError as GraphQLError>::bad_user_input(
            "expectedRevision must be an unsigned 64-bit decimal string",
        )
    })?;
    let (auth, principal, tenant_id) = authorized_request(ctx)?;
    let runtime_ctx = ctx.data::<ServerRuntimeContext>()?.clone();
    let db = ctx.data::<DatabaseConnection>()?.clone();
    let authorizer = ServerArtifactSecurityAuthorizer::new(db.clone(), &auth, principal, tenant_id)
        .map_err(map_authorization_error)?;
    let command = build_module_artifact_security_command(
        ArtifactReleaseRef {
            slug: module_slug,
            version,
            digest: payload_digest,
        },
        expected_revision,
        policy_revision,
        reason_code,
        reason_detail,
        auth.user_id,
        idempotency_key,
    );
    let service = ModuleControlPlane::new(db).artifact_security(authorizer);
    let receipt = match action {
        SecurityAction::Quarantine => service.quarantine(command).await,
        SecurityAction::ClearQuarantine => service.clear_quarantine(command).await,
        SecurityAction::Revoke => service.revoke(command).await,
    }
    .map_err(map_artifact_security_error)?;

    runtime_ctx.effective_policy_cache().clear();
    Ok(receipt_gql(ArtifactSecurityReceiptView::from(receipt)))
}

fn snapshot_gql(snapshot: ArtifactSecuritySnapshotView) -> ArtifactSecuritySnapshotGql {
    ArtifactSecuritySnapshotGql {
        module_slug: snapshot.module_slug,
        version: snapshot.version,
        payload_digest: snapshot.payload_digest,
        revision: snapshot.revision,
        status: snapshot.status,
        registry_status: snapshot.registry_status,
        policy_revision: snapshot.policy_revision,
        reason_code: snapshot.reason_code,
        reason_detail: snapshot.reason_detail,
    }
}

fn receipt_gql(receipt: ArtifactSecurityReceiptView) -> ArtifactSecurityReceiptGql {
    ArtifactSecurityReceiptGql {
        snapshot: snapshot_gql(receipt.snapshot),
        created: receipt.created,
    }
}

fn map_authorization_error(error: ArtifactSecurityAuthorizationError) -> FieldError {
    match error {
        ArtifactSecurityAuthorizationError::DirectSessionRequired => {
            <FieldError as GraphQLError>::permission_denied(
                "Direct authenticated user session required",
            )
        }
        ArtifactSecurityAuthorizationError::ModulesManageRequired => {
            <FieldError as GraphQLError>::permission_denied(
                "Permission denied: modules:manage required",
            )
        }
        ArtifactSecurityAuthorizationError::PermissionCheckFailed => {
            tracing::error!(
                error_code = "module_artifact_security_authorization_lookup_failed",
                "Artifact security authorization lookup failed"
            );
            <FieldError as GraphQLError>::internal_error(
                "Artifact security authorization lookup failed",
            )
        }
    }
}

fn conflict_error(code: &'static str, message: &'static str) -> FieldError {
    FieldError::new(message).extend_with(|_, extensions| {
        extensions.set("code", code);
    })
}

fn map_artifact_security_error(error: ModuleArtifactSecurityError) -> FieldError {
    match error {
        ModuleArtifactSecurityError::InvalidCommand => {
            <FieldError as GraphQLError>::bad_user_input("Invalid artifact security command")
        }
        ModuleArtifactSecurityError::RevisionConflict { .. } => conflict_error(
            "ARTIFACT_SECURITY_REVISION_CONFLICT",
            "Artifact security state changed; reload the snapshot before retrying",
        ),
        ModuleArtifactSecurityError::AlreadyQuarantined => conflict_error(
            "ARTIFACT_ALREADY_QUARANTINED",
            "Artifact release is already quarantined",
        ),
        ModuleArtifactSecurityError::NotQuarantined => conflict_error(
            "ARTIFACT_NOT_QUARANTINED",
            "Artifact release is not quarantined",
        ),
        ModuleArtifactSecurityError::Revoked => conflict_error(
            "ARTIFACT_RELEASE_REVOKED",
            "Artifact release is permanently revoked",
        ),
        ModuleArtifactSecurityError::IdempotencyConflict => conflict_error(
            "ARTIFACT_SECURITY_IDEMPOTENCY_CONFLICT",
            "Idempotency key conflicts with another artifact security command",
        ),
        ModuleArtifactSecurityError::OperationInProgress => conflict_error(
            "ARTIFACT_SECURITY_OPERATION_IN_PROGRESS",
            "Artifact security operation is still in progress",
        ),
        ModuleArtifactSecurityError::RevisionOverflow => {
            <FieldError as GraphQLError>::internal_error("Artifact security revision overflow")
        }
        ModuleArtifactSecurityError::AuthorizationDenied(_) => {
            <FieldError as GraphQLError>::permission_denied(
                "Artifact security authorization denied",
            )
        }
        ModuleArtifactSecurityError::Store(_) => {
            tracing::error!(
                error_code = "module_artifact_security_store_failed",
                "Artifact security owner operation failed"
            );
            <FieldError as GraphQLError>::internal_error("Artifact security operation failed")
        }
    }
}
