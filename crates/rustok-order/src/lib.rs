/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod analytics;
pub mod checkout_order_recovery;
pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod ports;
pub mod services;

pub use analytics::{OrderStatsSnapshot, load_order_stats_snapshot};
pub use checkout_order_recovery::*;
pub use dto::*;
pub use entities::*;
pub use ports::*;

pub use error::{OrderError, OrderResult};
pub use services::{
    OrderCheckoutIdentityError, OrderCheckoutIdentityJournal, OrderCheckoutIdentityResult,
    OrderService, RecordOrderCheckoutIdentity,
};

pub struct OrderModule;

#[async_trait]
impl RusToKModule for OrderModule {
    fn slug(&self) -> &'static str {
        "order"
    }

    fn name(&self) -> &'static str {
        "Order"
    }

    fn description(&self) -> &'static str {
        "Default order submodule in the ecommerce family"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::ORDERS_CREATE,
            Permission::ORDERS_READ,
            Permission::ORDERS_UPDATE,
            Permission::ORDERS_DELETE,
            Permission::ORDERS_LIST,
            Permission::ORDERS_MANAGE,
        ]
    }
}

impl MigrationSource for OrderModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}
