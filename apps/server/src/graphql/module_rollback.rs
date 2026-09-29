use async_graphql::{Context, Result};
use rustok_api::graphql::GraphQLError;
use rustok_modules::{ArtifactRollbackRequest, ModuleControlPlane};

use crate::graphql::artifact_lifecycle::map_artifact_installation_lifecycle_error;
use crate::graphql::mutations::{
    artifact_lifecycle_expected_revision, artifact_lifecycle_revision,
    ensure_modules_manage_permission, module_command_context, tenant_artifact_scope,
};
use crate::graphql::types::ArtifactRollback;
use uuid::Uuid;

fn validate_target_capability_grant_revision(value: i64) -> Result<u64> {
    if value <= 0 {
        return Err(<async_graphql::FieldError as GraphQLError>::bad_user_input(
            "Artifact rollback requires a positive target capability-grant revision",
        ));
    }

    u64::try_from(value).map_err(|_| {
        <async_graphql::FieldError as GraphQLError>::bad_user_input(
            "Artifact rollback capability-grant revision is outside the supported range",
        )
    })
}

/// Dedicated GraphQL mutation boundary for tenant artifact rollback.
///
/// The rollback target is owner-selected from the retained predecessor; callers
/// cannot supply an arbitrary installation ID. Authorization, tenant scope,
/// revision CAS, idempotency, and capability-grant revision checks remain
/// enforced before delegating to the module installation owner.
#[derive(Default)]
pub struct ModuleRollbackMutation;

#[async_graphql::Object]
impl ModuleRollbackMutation {
    async fn rollback_tenant_artifact(
        &self,
        ctx: &Context<'_>,
        installation_id: Uuid,
        expected_revision: i64,
        reason: String,
        idempotency_key: Uuid,
        target_capability_grant_revision: i64,
    ) -> Result<ArtifactRollback> {
        let (auth, tenant) = ensure_modules_manage_permission(ctx).await?;
        let expected_revision = artifact_lifecycle_expected_revision(
            installation_id,
            expected_revision,
            &reason,
            idempotency_key,
        )?;

        let target_capability_grant_revision =
            validate_target_capability_grant_revision(target_capability_grant_revision)?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let result = ModuleControlPlane::new(db.clone())
            .installation()
            .rollback_artifact(ArtifactRollbackRequest {
                installation_id,
                scope: tenant_artifact_scope(tenant.id),
                expected_revision,
                context: module_command_context(auth.user_id, Some(tenant.id), idempotency_key),
                reason,
                target_capability_grant_revision,
            })
            .await
            .map_err(map_artifact_installation_lifecycle_error)?;

        Ok(ArtifactRollback {
            operation_id: result.operation_id,
            source_installation_id: installation_id,
            target_installation_id: result.target_installation_id,
            source_revision: artifact_lifecycle_revision(result.source_revision)?,
            target_revision: artifact_lifecycle_revision(result.target_revision)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::validate_target_capability_grant_revision;

    #[test]
    fn rollback_target_revision_validation_rejects_non_positive_values() {
        assert!(validate_target_capability_grant_revision(0).is_err());
        assert!(validate_target_capability_grant_revision(-1).is_err());
    }

    #[test]
    fn rollback_target_revision_validation_accepts_positive_values() {
        assert_eq!(
            validate_target_capability_grant_revision(42).expect("positive revision"),
            42
        );
    }
}
