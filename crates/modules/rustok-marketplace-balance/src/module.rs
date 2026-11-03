//! # RusToK Marketplace Balance Module Implementation
//!
//! Provides `MarketplaceBalanceModule` implementing `RusToKModule` and `MigrationSource`.

use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

use crate::migrations;

/// Owns seller balances, escrow holds, and financial bucket transitions.
#[derive(Debug, Default, Clone, Copy)]
pub struct MarketplaceBalanceModule;

impl MarketplaceBalanceModule {
    pub fn new() -> Self {
        Self
    }
}

#[allow(non_camel_case_types)]
pub type Marketplace_balanceModule = MarketplaceBalanceModule;

#[async_trait]
impl RusToKModule for MarketplaceBalanceModule {
    fn slug(&self) -> &'static str {
        "marketplace_balance"
    }

    fn name(&self) -> &'static str {
        "Marketplace Balance"
    }

    fn description(&self) -> &'static str {
        "Marketplace Family seller balance, escrow hold, and financial tracking owner"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn dependencies(&self) -> &[&'static str] {
        &["marketplace_commission", "accounting"]
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::ORDERS_READ,
            Permission::ORDERS_LIST,
            Permission::ORDERS_MANAGE,
            Permission::MARKETPLACE_SELLERS_READ,
        ]
    }
}

impl MigrationSource for MarketplaceBalanceModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}
