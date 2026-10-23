use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

use crate::migrations;

/// Default payment submodule in the ecommerce family.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PaymentModule;

impl PaymentModule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RusToKModule for PaymentModule {
    fn slug(&self) -> &'static str {
        "payment"
    }

    fn name(&self) -> &'static str {
        "Payment"
    }

    fn description(&self) -> &'static str {
        "Default payment submodule in the ecommerce family"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Optional
    }

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::PAYMENTS_CREATE,
            Permission::PAYMENTS_READ,
            Permission::PAYMENTS_UPDATE,
            Permission::PAYMENTS_DELETE,
            Permission::PAYMENTS_LIST,
            Permission::PAYMENTS_MANAGE,
        ]
    }
}

impl MigrationSource for PaymentModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn payment_module_metadata_and_traits() {
        let module = PaymentModule::new();
        assert_eq!(module.slug(), "payment");
        assert_eq!(module.name(), "Payment");
        assert_eq!(
            module.description(),
            "Default payment submodule in the ecommerce family"
        );
        assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
        assert_eq!(module.kind(), ModuleKind::Optional);
        assert_eq!(module.health().await, HealthStatus::Healthy);
    }

    #[test]
    fn module_permissions_cover_payment_lifecycle() {
        let module = PaymentModule::new();
        let perms = module.permissions();
        assert!(perms.contains(&Permission::PAYMENTS_CREATE));
        assert!(perms.contains(&Permission::PAYMENTS_READ));
        assert!(perms.contains(&Permission::PAYMENTS_UPDATE));
        assert!(perms.contains(&Permission::PAYMENTS_DELETE));
        assert!(perms.contains(&Permission::PAYMENTS_LIST));
        assert!(perms.contains(&Permission::PAYMENTS_MANAGE));
    }
}
