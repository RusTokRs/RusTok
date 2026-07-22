use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

#[cfg(feature = "server")]
pub mod controllers;
pub mod dto;
pub mod entities;
pub mod error;
#[cfg(feature = "server")]
pub mod http;
pub mod migrations;
#[cfg(feature = "server")]
pub mod openapi;
pub mod ports;
#[cfg(feature = "server")]
pub mod provider_event_recovery_controller;
pub mod providers;
pub mod services;
#[cfg(feature = "stripe")]
pub mod stripe_provider;

pub use dto::*;
pub use entities::*;
pub use ports::*;
pub use providers::*;
#[cfg(feature = "stripe")]
pub use stripe_provider::*;

pub use error::{PaymentError, PaymentResult};
pub use services::{
    BeginProviderOperation, ChargebackLifecycleEventApplier, CheckpointProviderEvent,
    CompleteProviderEvent, FailProviderEvent, PaymentDomainEventApplier,
    PaymentLifecycleEventApplier, PaymentObservedDomainEventApplier,
    PaymentProviderEventApplier, PaymentProviderEventApplyError, PaymentProviderEventContext,
    PaymentProviderEventExecution, PaymentProviderEventIngressError,
    PaymentProviderEventIngressResult, PaymentProviderEventIngressService,
    PaymentProviderEventJournal, PaymentProviderEventObservers,
    PaymentProviderEventRecoveryFailure, PaymentProviderEventRecoveryOutcome,
    PaymentProviderEventRecoveryReport, PaymentProviderEventRecoveryService,
    PaymentProviderOperationJournal, PaymentProviderProcessedEventObserver,
    PaymentRefundCreationService, PaymentService, ReceiveProviderEvent,
    RefundLifecycleEventApplier, VerifiedProviderEvent, PROVIDER_EVENT_DEAD_LETTER,
    PROVIDER_EVENT_FAILED, PROVIDER_EVENT_PROCESSED, PROVIDER_EVENT_PROCESSING,
    PROVIDER_EVENT_RECEIVED, PROVIDER_OPERATION_COMMITTED, PROVIDER_OPERATION_ERROR,
    PROVIDER_OPERATION_EXECUTING, PROVIDER_OPERATION_PENDING,
    PROVIDER_OPERATION_RECONCILIATION_REQUIRED, PROVIDER_OPERATION_SUCCEEDED,
};

pub struct PaymentModule;

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
