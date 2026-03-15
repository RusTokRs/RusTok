pub mod dto;
pub mod entities;
pub mod error;
pub mod integration;
pub mod services;

pub use error::RbacError;
pub use integration::{
    RbacIntegrationEventKind, RbacRoleAssignmentEvent, RBAC_EVENT_ROLE_PERMISSIONS_ASSIGNED,
    RBAC_EVENT_TENANT_ROLE_ASSIGNMENTS_REMOVED, RBAC_EVENT_USER_ROLE_ASSIGNMENT_REMOVED,
    RBAC_EVENT_USER_ROLE_REPLACED,
};
pub use services::authz_mode::{AuthzEngine, RbacAuthzMode};
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

pub use services::casbin_model::default_casbin_model;
pub use services::casbin_shadow_evaluator::{
    compare_casbin_shadow_decision, evaluate_casbin_shadow, evaluate_casbin_shadow_comparison,
    evaluate_casbin_shadow_for_mode, evaluate_casbin_shadow_result, permissions_for_shadow_check,
    CasbinShadowComparison, CasbinShadowDecision, CasbinShadowEvaluation,
    CasbinShadowMismatchRecord, CasbinShadowSkipReason, CasbinShadowTelemetry,
};
pub use services::legacy_role_resolver::{
    invalidate_cached_legacy_role, resolve_legacy_role_with_cache, LegacyRoleCache,
    LegacyRoleResolution, LegacyRoleStore,
};
pub use services::permission_resolver::{PermissionResolution, PermissionResolver};
pub use services::relation_permission_resolver::{
    invalidate_cached_permissions, resolve_permissions_from_relations,
    resolve_permissions_with_cache, PermissionCache, RelationPermissionStore,
};
pub use services::runtime_permission_resolver::{RoleAssignmentStore, RuntimePermissionResolver};
pub use services::shadow_decision::{
    compare_all_permissions, compare_any_permissions, compare_shadow_decision,
    compare_single_permission, ShadowCheck, ShadowDecision,
};
pub use services::shadow_dual_read::{
    evaluate_dual_read, evaluate_dual_read_for_mode, evaluate_dual_read_result, DualReadComparison,
    DualReadEvaluation, DualReadMismatchRecord, DualReadOutcome, DualReadSkipReason,
    DualReadTelemetry,
};
pub use services::shadow_runtime::{
    evaluate_shadow_runtime_for_mode, observe_shadow_runtime, shadow_runtime_needs_legacy_role,
    shadow_runtime_runs_casbin, ShadowRuntimeContext, ShadowRuntimeEvaluation, ShadowRuntimeInput,
    ShadowRuntimeObserver, ShadowRuntimeTelemetry,
};

use async_trait::async_trait;
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

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
}

#[cfg(test)]
mod contract_tests;
