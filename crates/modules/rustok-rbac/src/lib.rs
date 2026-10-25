//! # RusToK RBAC Module
//!
//! Role-based access control (RBAC) helpers, permission evaluation, and catalog management.
//!
//! Provides permission resolution, policy enforcement, role mutation, system role repair,
//! localized permission presentations, and transport-neutral permission decision ports.

mod artifact_permission_assignment;
mod artifact_permission_catalog;
pub mod bootstrap;
pub mod catalog;
pub mod consistency;
mod control_plane;
pub mod dto;
pub mod error;
#[cfg(feature = "graphql")]
pub mod graphql;
mod invalidation_generation;
pub(crate) mod m20260714_900001_enforce_rbac_relation_tenant_integrity;
pub(crate) mod m20260714_900002_create_rbac_invalidation_state;
pub(crate) mod m20260716_000001_artifact_permission_catalog;
pub(crate) mod m20260717_000001_artifact_role_permissions;
pub(crate) mod m20260803_000001_canonicalize_artifact_permissions;
pub(crate) mod m20260914_000001_localized_presentations;
pub mod module;
pub mod ports;
pub mod presentation;
mod repair;
mod role_mutation;
pub mod services;

pub use module::RbacModule;

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
pub use ports::{
    RbacPermissionCheckMode, RbacPermissionCheckRequest, RbacPermissionCheckResponse,
    RbacPermissionDecisionPort, RbacPermissionDecisionProvider,
};
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
    DeniedReasonKind, PermissionCheckOutcome, check_all_permissions, check_any_permission,
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

#[cfg(test)]
mod contract_tests;

