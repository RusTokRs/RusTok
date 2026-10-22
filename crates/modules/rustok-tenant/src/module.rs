use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::MigrationDependencyDescriptor;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

/// RusToK multi-tenancy lifecycle and tenant-scoped module-state module.
pub struct TenantModule;

impl MigrationSource for TenantModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        crate::migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<MigrationDependencyDescriptor> {
        vec![MigrationDependencyDescriptor::new(
            "m20260726_000001_enforce_tenant_locale_policy",
            vec!["m20260405_000001_expand_locale_storage_columns"],
        )]
    }
}

#[async_trait]
impl RusToKModule for TenantModule {
    fn slug(&self) -> &'static str {
        "tenant"
    }

    fn name(&self) -> &'static str {
        "Tenant"
    }

    fn description(&self) -> &'static str {
        "Multi-tenancy helpers and tenant metadata."
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Core
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::TENANTS_CREATE,
            Permission::TENANTS_READ,
            Permission::TENANTS_UPDATE,
            Permission::TENANTS_DELETE,
            Permission::TENANTS_LIST,
            Permission::TENANTS_MANAGE,
            Permission::MODULES_READ,
            Permission::MODULES_LIST,
            Permission::MODULES_MANAGE,
        ]
    }

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
}
