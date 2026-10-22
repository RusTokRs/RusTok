/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */
//! # RusToK Commerce
//!
//! Umbrella and orchestration module for the RusToK commerce domain.
//!
//! Coordinates cart, checkout, orders, inventory reservations, payments,
//! fulfillments, and marketplace financial transactions across domain services.

#![allow(clippy::collapsible_if)]

pub mod module;
mod collection_translation_changes;
mod collection_translation_progress;
mod collection_translation_progress_target;
mod collection_translation_target;
pub mod controllers;
pub mod dto;
pub mod entities;
pub mod error;
pub mod fba;
pub mod graphql;
pub mod graphql_runtime;
pub mod migrations;
pub mod openapi;
mod search;
pub mod services;
pub mod state_machine;
mod storefront_channel;
mod storefront_checkout_pricing;
#[path = "storefront_checkout_runtime_mounted.rs"]
pub mod storefront_checkout_runtime;
mod storefront_shipping;

#[cfg(test)]
extern crate self as rustok_commerce;

#[cfg(test)]
mod contract_tests;
#[cfg(test)]
mod state_machine_proptest;

pub use collection_translation_changes::{
    CollectionTranslationChangeLifecycle, CollectionTranslationChangeRecord,
    MAX_COLLECTION_TRANSLATION_CHANGE_PAGE,
};
pub use collection_translation_progress_target::CommerceCollectionTranslationTargetProvider;
pub use error::{CommerceError, CommerceResult};
pub use services::checkout_execution_admission_port;
pub use services::{
    ApplyOrderChangeResult, BeginCheckoutOperation, BeginReturnCompletionOperation,
    CheckoutCompensationError, CheckoutCompensationResult, CheckoutCompensationService,
    CheckoutCompensationSweepFailure, CheckoutCompensationSweepReport,
    CheckoutCompensationSweepService, CheckoutError, CheckoutFulfillmentCreatedState,
    CheckoutFulfillmentPlan, CheckoutFulfillmentPlanItem, CheckoutFulfillmentStageError,
    CheckoutFulfillmentStageExecutor, CheckoutFulfillmentStageResult,
    CheckoutInventoryExecutionError, CheckoutInventoryExecutionResult,
    CheckoutInventoryOrderAdoption, CheckoutInventoryOrderAdoptionError,
    CheckoutInventoryOrderAdoptionResult, CheckoutInventoryOrderAdoptionService,
    CheckoutInventoryReservationError, CheckoutInventoryReservationExecutor,
    CheckoutInventoryReservationJournal, CheckoutInventoryReservationResult,
    CheckoutInventoryReservationStatus, CheckoutOperationCheckpoint, CheckoutOperationError,
    CheckoutOperationJournal, CheckoutOperationResult, CheckoutOperationStage,
    CheckoutOperationStatus, CheckoutOrderConfirmationError, CheckoutOrderConfirmationExecutor,
    CheckoutOrderConfirmationResult, CheckoutOrderCreationError, CheckoutOrderCreationExecutor,
    CheckoutOrderCreationResult, CheckoutOrderPlanError, CheckoutOrderPlanJournal,
    CheckoutOrderPlanPayload, CheckoutOrderPlanRecord, CheckoutOrderPlanResult,
    CheckoutOrderStageError, CheckoutOrderStageExecutor, CheckoutOrderStageResult,
    CheckoutPaymentCapturedState, CheckoutPaymentReadyState, CheckoutPaymentStageError,
    CheckoutPaymentStageExecutor, CheckoutPaymentStageResult, CheckoutPlanBuilder,
    CheckoutReconciliationAction, CheckoutReconciliationActionRequest,
    CheckoutReconciliationDecision, CheckoutReconciliationError, CheckoutReconciliationOutcome,
    CheckoutReconciliationResult, CheckoutReconciliationService, CheckoutResult, CheckoutService,
    CheckoutStagePipeline, CheckoutStagePipelineError, CheckoutStagePipelineResult,
    CompleteReturnClaimInput, CompleteReturnExchangeInput, CompleteReturnRefundInput,
    CompleteReturnResolutionInput, CreateReturnDecisionInput, DEFAULT_CHECKOUT_LEASE_SECONDS,
    DEFAULT_RETURN_COMPLETION_LEASE_SECONDS, ExchangeDifferenceRefundInput,
    FulfillmentCreateLabelRecoveryService, FulfillmentReconciliationService,
    JournaledCheckoutError, JournaledCheckoutResult, JournaledCheckoutService,
    MAX_CHECKOUT_LEASE_SECONDS, MAX_RETURN_COMPLETION_LEASE_SECONDS, OrderChangeOrchestrationError,
    OrderChangeOrchestrationResult, OrderChangeOrchestrationService,
    PaidOrderCreateLabelSweepReport, PaidOrderCreateLabelSweepService, PaymentOrchestrationError,
    PaymentOrchestrationResult, PaymentOrchestrationService, PlanCheckoutInventoryReservation,
    PostOrderOrchestrationError, PostOrderOrchestrationService, ReconciliationResolution,
    RecoveringStagedCheckoutError, RecoveringStagedCheckoutResult, RecoveringStagedCheckoutService,
    RefundReconciliationService, ReturnClaimDecisionInput, ReturnCompletionOperationCheckpoint,
    ReturnCompletionOperationError, ReturnCompletionOperationJournal,
    ReturnCompletionOperationResult, ReturnCompletionOperationStage,
    ReturnCompletionOperationStatus, ReturnCompletionOrchestrationService, ReturnDecisionInput,
    ReturnDecisionOwnerOrchestrationError, ReturnDecisionOwnerOrchestrationResult,
    ReturnDecisionOwnerOrchestrationService, ReturnDecisionResponse, ReturnExchangeDecisionInput,
    ReturnRefundDecisionInput, ShippingProfileService, StagedCheckoutError, StagedCheckoutResult,
    StagedCheckoutService, StoreContextError, StoreContextResult, StoreContextService,
};
#[cfg(feature = "marketplace-financial")]
pub use services::{
    CheckoutMarketplaceFinancialError, CheckoutMarketplaceFinancialResult,
    CheckoutMarketplaceFinancialStage, IngestMarketplacePaidEvent, IngestMarketplaceReversalEvent,
    MarketplaceFinancialOperationError, MarketplaceFinancialOperationJournal,
    MarketplaceFinancialOperationOperatorView, MarketplaceFinancialOperationResult,
    MarketplaceFinancialOperationStatus, MarketplaceFinancialOperatorError,
    MarketplaceFinancialOperatorResult, MarketplaceFinancialOperatorService,
    MarketplaceFinancialRuntime, MarketplacePaidEventInboxError, MarketplacePaidEventInboxJournal,
    MarketplacePaidEventInboxResult, MarketplacePaidEventInboxService,
    MarketplacePaidEventOperatorView, MarketplacePaidEventStatus, MarketplacePaidEventSweepFailure,
    MarketplacePaidEventSweepReport, MarketplaceProviderPaidEventAdapter,
    MarketplaceProviderPaidEventAdapterError, MarketplaceProviderPaidEventAdapterResult,
    MarketplaceProviderReversalAdaptFailure, MarketplaceProviderReversalAdaptReport,
    MarketplaceProviderReversalBackfillError, MarketplaceProviderReversalBackfillResult,
    MarketplaceProviderReversalBackfillService, MarketplaceProviderReversalEventAdapter,
    MarketplaceProviderReversalEventAdapterError, MarketplaceProviderReversalEventAdapterResult,
    MarketplaceReversalAdaptationFailureError, MarketplaceReversalAdaptationFailureJournal,
    MarketplaceReversalAdaptationFailureResult, MarketplaceReversalAdaptationFailureStatus,
    MarketplaceReversalEventInboxError, MarketplaceReversalEventInboxJournal,
    MarketplaceReversalEventInboxResult, MarketplaceReversalEventInboxService,
    MarketplaceReversalEventOperatorView, MarketplaceReversalEventStatus,
    MarketplaceReversalEventSweepFailure, MarketplaceReversalEventSweepReport,
    MarketplaceReversalOperatorError, MarketplaceReversalOperatorResult,
    MarketplaceReversalOperatorService,
};
pub use services::{
    FulfillmentOrchestrationError, FulfillmentOrchestrationService,
    JournaledFulfillmentOrchestrationService,
};
pub use services::{
    InventoryShipmentStockPort, fulfillment_admin_command_runtime, fulfillment_service,
    inventory_shipment_stock_port,
};
pub use services::{
    ManualShipmentCommand, OrchestratedOrderAdminCommandPort, OrderCancellationOrchestrator,
    OrderShipmentProjector, ProjectingFulfillmentAdminCommandPort, ShipmentTruth,
    orchestrated_order_admin_command_runtime, projecting_fulfillment_admin_command_runtime,
    shipment_truth,
};
pub(crate) use storefront_checkout_pricing::StorefrontCheckoutPricingResolver;

pub use module::CommerceModule;
