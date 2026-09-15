mod artifact_permission_assignment;
mod artifact_permission_catalog;
pub mod bootstrap;
pub mod catalog;
mod control_plane;
pub mod dto;
pub mod error;
#[cfg(feature = "graphql")]
pub mod graphql;
mod invalidation_generation;
mod m20260714_900001_enforce_rbac_relation_tenant_integrity;
mod m20260714_900002_create_rbac_invalidation_state;
mod m20260716_000001_artifact_permission_catalog;
mod m20260717_000001_artifact_role_permissions;
mod m20260803_000001_canonicalize_artifact_permissions;
mod m20260914_000001_localized_presentations;
pub mod ports;
pub mod presentation;
mod presentation_seed;
mod repair;
mod role_mutation;
pub mod services;

pub use artifact_permission_assignment::{
    ArtifactPermissionAssignmentError, ArtifactPermissionAssignmentScope,
    ArtifactPermissionEventPublisher, ArtifactRolePermissionAssignmentCommand,
    ArtifactRolePermissionAssignmentResult, RbacArtifactPermissionAssignmentService,
    SeaOrmArtifactPermissionAuthorizer,
};
pub use artifact_permission_catalog::RbacArtifactPermissionCatalog;
pub use bootstrap::{RbacRoleAssignmentDbWriter, RbacRoleAssignmentError};
pub use catalog::BuiltinTenantRbacCatalog;
pub use consistency::{RbacConsistencyStats, load_consistency_stats};
pub use control_plane::{
    RbacControlPlaneAdmissionError, RbacControlPlanePrincipal, require_direct_control_plane_user,
};
pub use error::RbacError;
pub use invalidation_generation::{
    RBAC_PERMISSION_INVALIDATION_SCOPE, RbacInvalidationGenerationError,
    read_permission_invalidation_generation, reserve_permission_invalidation_generation,
};
pub use ports::*;
pub use presentation::{
    RbacLocalizedPresentation, RbacPresentationResourceKind, RbacPresentationStore,
    RbacPresentationStoreError, SeaOrmRbacPresentationStore,
};
pub use repair::{
    RbacAffectedUser, RbacSystemRoleRepairError, RbacSystemRoleRepairOptions,
    RbacSystemRoleRepairReport,
};
pub use role_mutation::{
    RbacRoleMutationChange, RbacRoleMutationFacts, RbacRoleMutationOutcome, RbacRoleMutationPlan,
    RbacRoleMutationPolicyError, RbacRolePersistenceError, RbacUserAuthorityChange,
    RbacUserRoleMutationRequest, count_remaining_active_super_admins_on,
    ensure_user_authority_continuity_on, has_exact_tenant_role_assignment_on,
    plan_persisted_user_role_mutation_on, plan_user_role_mutation, replace_persisted_user_role_on,
    require_request_role_grant, require_role_assignment, require_user_management,
};
pub use services::authz_mode::AuthzEngine;
pub use services::permission_authorizer::{
    AuthorizationDecision, authorize_all_permissions, authorize_any_permission,
    authorize_permission,
};
pub use services::permission_evaluator::{
    PermissionEvaluation, evaluate_all_permissions, evaluate_any_permission,
    evaluate_single_permission,
};
pub use services::permission_policy::{
    DeniedReasonKind, PermissionCheckOutcome, check_all_permissions, check_any_permissions,
    check_permission, denied_reason_for_denial, has_effective_permission_in_set,
    missing_permissions,
};

pub use services::permission_resolver::{PermissionResolution, PermissionResolver};
pub use services::policy_model::{
    TenantPolicyEnforcer, build_tenant_policy_csv, build_tenant_policy_enforcer,
    default_tenant_policy_model, resolved_permissions_subject,
};
pub use services::relation_permission_resolver::{
    PermissionCache, PermissionCacheLookup, RelationPermissionStore, SeaOrmRelationPermissionStore,
    authorize_current_permission, invalidate_cached_permissions, load_role_user_ids_on,
    resolve_permissions_from_relations, resolve_permissions_with_cache,
    resolve_persisted_permissions_on,
};
pub use services::runtime_permission_resolver::RuntimePermissionResolver;

pub async fn plan_system_role_repair(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Option<uuid::Uuid>,
) -> Result<RbacSystemRoleRepairReport, RbacSystemRoleRepairError> {
    repair::repair_system_roles(
        db,
        RbacSystemRoleRepairOptions {
            tenant_id,
            apply: false,
        },
    )
    .await
}

pub async fn apply_system_role_repair_in_transaction(
    db: &sea_orm::DatabaseTransaction,
    tenant_id: Option<uuid::Uuid>,
) -> Result<RbacSystemRoleRepairReport, RbacSystemRoleRepairError> {
    repair::repair_system_roles_in_transaction(
        db,
        RbacSystemRoleRepairOptions {
            tenant_id,
            apply: true,
        },
    )
    .await
}

use async_trait::async_trait;
use rustok_api::{Permission, SharedTenantRbacCatalog};
use rustok_core::module::{
    HealthStatus, MigrationSource, ModuleKind, ModuleRuntimeExtensions, RusToKModule,
};
use sea_orm_migration::MigrationTrait;
use std::sync::Arc;

pub struct RbacModule;

impl MigrationSource for RbacModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260714_900001_enforce_rbac_relation_tenant_integrity::Migration),
            Box::new(m20260714_900002_create_rbac_invalidation_state::Migration),
            Box::new(m20260716_000001_artifact_permission_catalog::Migration),
            Box::new(m20260717_000001_artifact_role_permissions::Migration),
            Box::new(m20260803_000001_canonicalize_artifact_permissions::Migration),
            Box::new(m20260914_000001_localized_presentations::Migration),
        ]
    }
}

#[async_trait]
impl RusToKModule for RbacModule {
    fn slug(&self) -> &'static str {
        "rbac"
    }

    fn name(&self) -> &'static str {
        "RBAC"
    }

    fn description(&self) -> &'static str {
        "Role-based access control helpers."
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn dependencies(&self) -> &[&'static str] {
        &["outbox"]
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Core
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::SETTINGS_READ,
            Permission::SETTINGS_UPDATE,
            Permission::SETTINGS_MANAGE,
            Permission::LOGS_READ,
            Permission::LOGS_LIST,
        ]
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        extensions.insert(SharedTenantRbacCatalog(Arc::new(BuiltinTenantRbacCatalog)));
        Ok(())
    }

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
}

pub mod consistency;
#[cfg(test)]
mod contract_tests;
