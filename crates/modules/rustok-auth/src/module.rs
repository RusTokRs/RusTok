use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::MigrationDependencyDescriptor;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

/// Core auth module — JWT lifecycle, credential hashing, token management.
///
/// Pure logic module with no framework dependencies. The server constructs
/// `AuthConfig` from its config source and passes it to the auth functions.
pub struct AuthModule;

impl MigrationSource for AuthModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        crate::migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<MigrationDependencyDescriptor> {
        crate::migrations::migration_dependencies()
    }
}

#[async_trait]
impl RusToKModule for AuthModule {
    fn slug(&self) -> &'static str {
        "auth"
    }

    fn name(&self) -> &'static str {
        "Auth"
    }

    fn description(&self) -> &'static str {
        "JWT lifecycle, credential hashing, token management."
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Core
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::USERS_CREATE,
            Permission::USERS_READ,
            Permission::USERS_UPDATE,
            Permission::USERS_DELETE,
            Permission::USERS_LIST,
            Permission::USERS_MANAGE,
        ]
    }

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
}
