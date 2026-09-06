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
use rustok_core::{
    MigrationSource, ModuleEventListenerContext, ModuleEventListenerRegistry, RusToKModule,
};
use rustok_fulfillment::providers::FulfillmentProviderRegistry;
#[cfg(feature = "marketplace-financial")]
use rustok_outbox::TransactionalEventBus;
use sea_orm_migration::MigrationTrait;

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
mod state_machine_proptest;

pub use error::{CommerceError, CommerceResult};
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
    CheckoutPaymentStageExecutor, CheckoutPaymentStageResult, CheckoutPlanBuilder, CheckoutResult,
    CheckoutService, CheckoutStagePipeline, CheckoutStagePipelineError,
    CheckoutStagePipelineResult, CompleteReturnClaimInput, CompleteReturnExchangeInput,
    CompleteReturnRefundInput, CompleteReturnResolutionInput, CreateReturnDecisionInput,
    DEFAULT_CHECKOUT_LEASE_SECONDS, DEFAULT_RETURN_COMPLETION_LEASE_SECONDS,
    ExchangeDifferenceRefundInput, FulfillmentCreateLabelRecoveryService,
    FulfillmentReconciliationService, JournaledCheckoutError, JournaledCheckoutResult,
    JournaledCheckoutService, MAX_CHECKOUT_LEASE_SECONDS, MAX_RETURN_COMPLETION_LEASE_SECONDS,
    OrderChangeOrchestrationError, OrderChangeOrchestrationResult, OrderChangeOrchestrationService,
    PaidOrderCreateLabelSweepReport, PaidOrderCreateLabelSweepService, PaymentOrchestrationError,
    PaymentOrchestrationResult, PaymentOrchestrationService, PlanCheckoutInventoryReservation,
    PostOrderOrchestrationError, PostOrderOrchestrationService, RecoveringStagedCheckoutError,
    RecoveringStagedCheckoutResult, RecoveringStagedCheckoutService, RefundReconciliationService,
    ReturnClaimDecisionInput, ReturnCompletionOperationCheckpoint, ReturnCompletionOperationError,
    ReturnCompletionOperationJournal, ReturnCompletionOperationResult,
    ReturnCompletionOperationStage, ReturnCompletionOperationStatus,
    ReturnCompletionOrchestrationService, ReturnDecisionInput,
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
pub(crate) use storefront_checkout_pricing::StorefrontCheckoutPricingResolver;

pub struct CommerceModule;

#[async_trait]
impl RusToKModule for CommerceModule {
    fn slug(&self) -> &'static str {
        "commerce"
    }

    fn name(&self) -> &'static str {
        "Ecommerce"
    }

    fn description(&self) -> &'static str {
        "Ecommerce umbrella/root module for the commerce family and orchestration surface"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn dependencies(&self) -> &[&'static str] {
        &[
            "tenant",
            "cart",
            "customer",
            "product",
            "region",
            "pricing",
            "inventory",
            "order",
            "payment",
            "fulfillment",
        ]
    }

    fn register_event_listeners(
        &self,
        registry: &mut ModuleEventListenerRegistry,
        ctx: &ModuleEventListenerContext<'_>,
    ) {
        let fulfillment_registry = ctx
            .extensions
            .get::<FulfillmentProviderRegistry>()
            .cloned()
            .expect(
                "commerce module requires FulfillmentProviderRegistry in ModuleRuntimeExtensions",
            );
        registry.register(services::PaidOrderCreateLabelHandler::new(
            ctx.db.clone(),
            fulfillment_registry,
        ));

        #[cfg(feature = "marketplace-financial")]
        {
            let financial_runtime = ctx
                .extensions
                .get::<MarketplaceFinancialRuntime>()
                .cloned()
                .expect(
                    "commerce marketplace-financial capability requires MarketplaceFinancialRuntime in ModuleRuntimeExtensions",
                );
            let event_bus = ctx
                .extensions
                .get::<TransactionalEventBus>()
                .cloned()
                .expect(
                    "commerce marketplace-financial capability requires TransactionalEventBus in ModuleRuntimeExtensions",
                );
            registry.register(services::MarketplacePaidOrderFinancialHandler::new(
                ctx.db.clone(),
                event_bus,
                financial_runtime.ledger_port(),
            ));
        }
    }
}

impl MigrationSource for CommerceModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<rustok_core::MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}
