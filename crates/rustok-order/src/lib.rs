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

mod admin_command;
pub mod analytics;
mod checkout_compensation;
mod checkout_compensation_local_context;
pub mod checkout_order_recovery;
#[path = "checkout_owner_context.rs"]
mod checkout_owner_context_impl;
mod checkout_payment_settlement;
pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
mod order_read;
pub mod ports;
pub mod services;
pub mod status;

pub mod checkout_owner_context {
    pub use crate::checkout_compensation_local_context::{
        InProcessCheckoutOrderCompensationPort, in_process_checkout_order_compensation_port,
    };
    pub use crate::checkout_owner_context_impl::{
        InProcessCheckoutOrderPaymentSettlementPort,
        in_process_checkout_order_payment_settlement_port,
    };
}

pub use admin_command::{
    CancelOrderRequest, DeliverOrderRequest, InProcessOrderAdminCommandPort,
    MarkOrderPaidRequest, OrderAdminCommandPort, OrderAdminCommandRuntime, ShipOrderRequest,
    in_process_order_admin_command_port,
};
pub use analytics::{OrderStatsSnapshot, load_order_stats_snapshot};
pub use checkout_compensation::{
    CheckoutOrderCompensationPort, CheckoutOrderCompensationRequest,
    CheckoutOrderCompensationSnapshot,
};
pub use checkout_compensation_local_context::{
    InProcessCheckoutOrderCompensationPort, in_process_checkout_order_compensation_port,
};
pub use checkout_order_recovery::*;
pub use checkout_owner_context_impl::{
    InProcessCheckoutOrderPaymentSettlementPort,
    in_process_checkout_order_payment_settlement_port,
};
pub use checkout_payment_settlement::{
    CheckoutOrderPaymentSettlementPort, SettleCheckoutOrderPaymentRequest,
};
pub use dto::*;
pub use entities::*;
pub use order_read::{
    InProcessOrderReadPort, ListOrderChangeProjectionsRequest, ListOrderProjectionsRequest,
    ListOrderReturnProjectionsRequest, OrderChangeProjectionPage, OrderProjectionPage,
    OrderReadPort, OrderReturnProjectionPage, ReadOrderChangeProjectionRequest,
    ReadOrderProjectionRequest, ReadOrderReturnProjectionRequest, in_process_order_read_port,
};
pub use ports::*;
pub use status::*;

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
