use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AccountingModule;

#[async_trait]
impl RusToKModule for AccountingModule {
    fn slug(&self) -> &'static str {
        "accounting"
    }

    fn name(&self) -> &'static str {
        "Accounting"
    }

    fn description(&self) -> &'static str {
        "Universal double-entry General Ledger, Chart of Accounts, and Posting Rules"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn dependencies(&self) -> &[&'static str] {
        &["outbox"]
    }

    fn kind(&self) -> rustok_core::module::ModuleKind {
        rustok_core::module::ModuleKind::Optional
    }

    fn permissions(&self) -> Vec<Permission> {
        Vec::new()
    }

    async fn health(&self) -> rustok_core::module::HealthStatus {
        rustok_core::module::HealthStatus::Healthy
    }
}

impl MigrationSource for AccountingModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn test_accounting_module_metadata_and_traits() {
        let module = AccountingModule::default();
        assert_eq!(module.slug(), "accounting");
        assert_eq!(module.name(), "Accounting");
        assert_eq!(
            module.description(),
            "Universal double-entry General Ledger, Chart of Accounts, and Posting Rules"
        );
        assert!(!module.version().is_empty());
        assert_eq!(module.dependencies(), &["outbox"]);
        assert_eq!(module.kind(), rustok_core::module::ModuleKind::Optional);
        assert!(module.permissions().is_empty());
        assert!(module.migrations().is_empty());

        let cloned = module;
        assert_eq!(module, cloned);
    }
}
