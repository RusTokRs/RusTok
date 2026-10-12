use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

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

    fn permissions(&self) -> Vec<Permission> {
        Vec::new()
    }
}

impl MigrationSource for AccountingModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        Vec::new()
    }
}
