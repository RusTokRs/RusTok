/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! # RusToK Order
//!
//! Order domain module extracted from commerce.
//! Provides order lifecycle state machines, staged checkout compensation,
//! checkout recovery adapters, post-order command execution, and read projections.

#![deny(clippy::float_arithmetic)]

mod admin_command;
pub mod analytics;
mod checkout_compensation;
mod checkout_compensation_local_context;
pub mod checkout_order_recovery;
#[path = "checkout_owner_context.rs"]
mod checkout_owner_context_impl;
mod checkout_payment_settlement;
mod command_receipts;
pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod module;
mod order_read;
pub mod ports;
mod post_order_command;
pub mod services;
pub mod status;

#[cfg(test)]
mod contract_tests;

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
    CancelOrderRequest, DeliverOrderRequest, InProcessOrderAdminCommandPort, MarkOrderPaidRequest,
    OrderAdminCommandPort, OrderAdminCommandRuntime, ShipOrderRequest,
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
pub use checkout_order_recovery::{
    CheckoutOrderRecoveryAdapter, ReadCheckoutOrderProjectionRequest,
    RecoverExistingCheckoutOrderRequest, in_process_checkout_order_recovery_adapter,
};
pub use checkout_owner_context_impl::{
    InProcessCheckoutOrderPaymentSettlementPort, in_process_checkout_order_payment_settlement_port,
};
pub use checkout_payment_settlement::{
    CheckoutOrderPaymentSettlementPort, SettleCheckoutOrderPaymentRequest,
};
pub use dto::{
    ApplyOrderChangeInput, CancelOrderChangeInput, CancelOrderInput, CancelOrderReturnInput,
    CompleteOrderReturnInput, CreateOrderAdjustmentInput, CreateOrderChangeInput, CreateOrderInput,
    CreateOrderLineItemInput, CreateOrderReturnInput, CreateOrderReturnItemInput,
    CreateOrderTaxLineInput, DeliverOrderInput, ListOrderChangesInput, ListOrderReturnsInput,
    ListOrdersInput, MarkPaidOrderInput, OrderAdjustmentResponse, OrderChangeResponse,
    OrderLineFulfillmentRequirement, OrderLineItemResponse, OrderResponse, OrderReturnItemResponse,
    OrderReturnResponse, OrderTaxLineResponse, ShipOrderInput,
};
pub use entities::order as order_entity;
pub use error::{OrderError, OrderResult};
pub use module::OrderModule;
pub use order_read::{
    InProcessOrderReadPort, ListOrderChangeProjectionsRequest, ListOrderProjectionsRequest,
    ListOrderReturnProjectionsRequest, OrderChangeProjectionPage, OrderProjectionPage,
    OrderReadPort, OrderReturnProjectionPage, ReadOrderChangeProjectionRequest,
    ReadOrderProjectionRequest, ReadOrderReturnProjectionRequest, in_process_order_read_port,
};
pub use ports::{
    BindCheckoutOrderIdentityRequest, CheckoutCompletionPort, CheckoutCompletionSnapshot,
    CheckoutOrderIdentityPort, CheckoutOrderIdentitySnapshot, CheckoutResultByOperationRequest,
    CheckoutResultRequest, CompleteCheckoutPortRequest, InProcessCheckoutCompletionPort,
    InProcessCheckoutOrderIdentityPort, OrderStatusRequest, OrderStatusSnapshot,
    ReadCheckoutOrderIdentityByCartRequest, ReadCheckoutOrderIdentityByOperationRequest,
    in_process_checkout_completion_port, in_process_checkout_order_identity_port,
};
pub use post_order_command::{
    ApplyOrderChangeRequest, CancelOrderChangeRequest, CancelOrderReturnRequest,
    CompleteOrderReturnRequest, CreateOrderChangeRequest, CreateOrderReturnRequest,
    InProcessOrderPostOrderCommandPort, OrderPostOrderCommandPort, OrderPostOrderCommandRuntime,
    in_process_order_post_order_command_port,
};
pub use services::{
    CreateCheckoutOrderIdentityInput, OrderCheckoutIdentityError, OrderCheckoutIdentityJournal,
    OrderCheckoutIdentityResult, OrderService, RecordOrderCheckoutIdentity,
};
pub use status::{OrderChangeStatusKind, OrderReturnStatusKind, OrderStatusKind};
