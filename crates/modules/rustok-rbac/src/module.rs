use crate::catalog::BuiltinTenantRbacCatalog;
use async_trait::async_trait;
use rustok_api::{Permission, SharedTenantRbacCatalog};
use rustok_core::module::{
    HealthStatus, MigrationSource, ModuleKind, ModuleRuntimeExtensions, RusToKModule,
};
use sea_orm_migration::MigrationTrait;
use std::sync::Arc;

/// Role-based access control (RBAC) lifecycle and module definition.
pub struct RbacModule;

impl MigrationSource for RbacModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(crate::m20260714_900001_enforce_rbac_relation_tenant_integrity::Migration),
            Box::new(crate::m20260714_900002_create_rbac_invalidation_state::Migration),
            Box::new(crate::m20260716_000001_artifact_permission_catalog::Migration),
            Box::new(crate::m20260717_000001_artifact_role_permissions::Migration),
            Box::new(crate::m20260803_000001_canonicalize_artifact_permissions::Migration),
            Box::new(crate::m20260914_000001_localized_presentations::Migration),
            Box::new(crate::m20261127_000001_create_rbac_membership_revision::Migration),
            Box::new(crate::m20261127_000003_create_rbac_role_grant_revision::Migration),
            Box::new(crate::m20261127_000006_fence_rbac_relation_parents::Migration),
            Box::new(crate::m20261127_000012_fence_rbac_invalidation_generation::Migration),
            Box::new(crate::m20261127_000013_preserve_rbac_identity_witnesses::Migration),
            Box::new(crate::m20261127_000016_fence_artifact_catalog_parents::Migration),
            Box::new(crate::m20261127_000017_couple_artifact_grant_generation::Migration),
            Box::new(crate::m20261127_000206_preserve_artifact_operation_witnesses::Migration),
            Box::new(crate::m20261127_000207_couple_static_relation_generation::Migration),
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
