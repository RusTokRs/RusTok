#![deny(clippy::float_arithmetic)]

use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

mod balance;
mod balance_transfer;
mod receipts;
mod reversal;
#[cfg(test)]
mod tests;

pub mod accounting_bridge;
pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod ports;
pub mod service;

pub use accounting_bridge::*;
pub use dto::*;
pub use error::{MarketplaceLedgerError, MarketplaceLedgerResult};
pub use ports::*;
pub use service::MarketplaceLedgerService;

pub type MarketplaceBalanceError = MarketplaceLedgerError;
pub type MarketplaceBalanceResult<T> = MarketplaceLedgerResult<T>;
pub type MarketplaceBalanceService = MarketplaceLedgerService;

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
