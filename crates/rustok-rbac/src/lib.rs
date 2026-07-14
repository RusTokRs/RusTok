pub mod bootstrap;
pub mod dto;
pub mod entities;
pub mod error;
#[cfg(feature = "graphql")]
pub mod graphql;
pub mod integration;
pub mod ports;
pub mod repair;
pub mod services;

pub use bootstrap::{RbacRoleAssignmentDbWriter, RbacRoleAssignmentError};
pub use consistency::{load_consistency_stats, RbacConsistencyStats};
pub use error::RbacError;
pub use integration::{
    RbacIntegrationEventKind, RbacRoleAssignmentEvent, RBAC_EVENT_ROLE_PERMISSIONS_ASSIGNED,
    RBAC_EVENT_TENANT_ROLE_ASSIGNMENTS_REMOVED, RBAC_EVENT_USER_ROLE_ASSIGNMENT_REMOVED,
    RBAC_EVENT_USER_ROLE_REPLACED,
};
pub use ports::*;
pub use repair::{
    repair_system_roles, RbacAffectedUser, RbacSystemRoleRepairError,
    RbacSystemRoleRepairOptions, RbacSystemRoleRepairReport,
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

use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub struct RbacModule;

impl MigrationSource for RbacModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        Vec::new()
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

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
}

pub mod consistency;
#[cfg(test)]
mod contract_tests;
