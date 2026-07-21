mod artifact_permission_assignment;
mod artifact_permission_catalog;
pub mod bootstrap;
pub mod catalog;
pub mod dto;
pub mod entities;
pub mod error;
#[cfg(feature = "graphql")]
pub mod graphql;
pub mod integration;
mod invalidation_generation;
mod m20260714_900001_enforce_rbac_relation_tenant_integrity;
mod m20260714_900002_create_rbac_invalidation_state;
mod m20260716_000001_artifact_permission_catalog;
mod m20260717_000001_artifact_role_permissions;
pub mod ports;
mod repair;
pub mod services;

pub use artifact_permission_assignment::{
    ArtifactPermissionAssignmentError, ArtifactRolePermissionAssignmentCommand,
    ArtifactRolePermissionAssignmentResult, RbacArtifactPermissionAssignmentService,
    SeaOrmArtifactPermissionAuthorizer,
};
pub use artifact_permission_catalog::RbacArtifactPermissionCatalog;
pub use bootstrap::{RbacRoleAssignmentDbWriter, RbacRoleAssignmentError};
pub use catalog::BuiltinTenantRbacCatalog;
pub use consistency::{load_consistency_stats, RbacConsistencyStats};
pub use error::RbacError;
pub use integration::{
    RbacIntegrationEventKind, RbacRoleAssignmentEvent, RBAC_EVENT_ROLE_PERMISSIONS_ASSIGNED,
    RBAC_EVENT_TENANT_ROLE_ASSIGNMENTS_REMOVED, RBAC_EVENT_USER_ROLE_ASSIGNMENT_REMOVED,
    RBAC_EVENT_USER_ROLE_REPLACED,
};
pub use invalidation_generation::{
    read_permission_invalidation_generation, reserve_permission_invalidation_generation,
    RbacInvalidationGenerationError, RBAC_PERMISSION_INVALIDATION_SCOPE,
};
pub use ports::*;
pub use repair::{
    RbacAffectedUser, RbacSystemRoleRepairError, RbacSystemRoleRepairOptions,
    RbacSystemRoleRepairReport,
};
pub use services::authz_mode::AuthzEngine;
pub use services::permission_authorizer::{
    authorize_all_permissions, authorize_any_permission, authorize_permission,
    AuthorizationDecision,
};
pub use services::permission_evaluator::{
    evaluate_all_permissions, evaluate_any_permission, evaluate_single_permission,
    PermissionEvaluation,
};
pub use services::permission_policy::{
    check_all_permissions, check_any_permission, check_permission, denied_reason_for_denial,
    has_effective_permission_in_set, missing_permissions, DeniedReasonKind, PermissionCheckOutcome,
};

pub use services::permission_resolver::{PermissionResolution, PermissionResolver};
pub use services::policy_model::{
    build_tenant_policy_csv, build_tenant_policy_enforcer, default_tenant_policy_model,
    resolved_permissions_subject, TenantPolicyEnforcer,
};
pub use services::relation_permission_resolver::{
    invalidate_cached_permissions, resolve_permissions_from_relations,
    resolve_permissions_with_cache, PermissionCache, PermissionCacheLookup,
    RelationPermissionStore,
};
pub use services::runtime_permission_resolver::{RoleAssignmentStore, RuntimePermissionResolver};

/// Build a read-only canonical system-role repair plan.
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

/// Apply canonical system-role repair inside a caller-owned database
/// transaction. Restricting this public boundary to `DatabaseTransaction`
/// prevents external callers from mutating role definitions without an atomic
/// commit boundary for the accompanying invalidation generation.
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
