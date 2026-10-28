//! # RusToK Payment
//!
//! Payment domain module extracted from commerce.
//! Provides payment collection schemas, provider SPI abstraction, refund lifecycle,
//! checkout execution ports, and transactional event handling.

#![deny(clippy::float_arithmetic)]

mod admin_collection_command;
mod admin_read;
mod admin_refund_command;
mod cart_read;
#[path = "checkout_compensation_api.rs"]
pub mod checkout_compensation;
mod checkout_compensation_context;
#[path = "checkout_compensation.rs"]
mod checkout_compensation_persistent;
pub mod checkout_execution;
mod collection_runtime;
#[cfg(feature = "server")]
pub mod controllers;
pub mod dto;
pub mod entities;
pub mod error;
#[cfg(feature = "server")]
pub mod http;
pub mod migrations;
pub mod module;
#[cfg(feature = "server")]
pub mod openapi;
mod order_read;
pub mod ports;
#[cfg(feature = "server")]
pub mod provider_event_recovery_controller;
pub mod providers;
pub mod services;
#[cfg(feature = "stripe")]
pub mod stripe_provider;

#[cfg(test)]
mod contract_tests;

pub use admin_collection_command::{
    AuthorizeAdminPaymentCollectionRequest, CancelAdminPaymentCollectionRequest,
    CaptureAdminPaymentCollectionRequest, InProcessPaymentAdminCollectionCommandPort,
    PaymentAdminCollectionCommandPort, PaymentAdminCollectionCommandRuntime,
    in_process_payment_admin_collection_command_port,
};
pub use admin_read::{
    InProcessPaymentAdminReadPort, ListPaymentCollectionProjectionsRequest,
    ListRefundProjectionsRequest, PaymentAdminReadPort, PaymentAdminReadRuntime,
    PaymentCollectionProjectionPage, ReadPaymentCollectionProjectionRequest,
    ReadRefundProjectionRequest, RefundProjectionPage, in_process_payment_admin_read_port,
};
pub use admin_refund_command::{
    CancelAdminRefundRequest, CompleteAdminRefundRequest, CreateAdminRefundRequest,
    InProcessPaymentAdminRefundCommandPort, PaymentAdminRefundCommandPort,
    PaymentAdminRefundCommandRuntime, in_process_payment_admin_refund_command_port,
};
pub use cart_read::{
    InProcessPaymentCartReadPort, PaymentCartReadPort, PaymentCartReadRuntime,
    ReusablePaymentCollectionByCartRequest, in_process_payment_cart_read_port,
};
pub use checkout_compensation::{
    CheckoutPaymentCompensationPort, CheckoutPaymentCompensationRequest,
    InProcessCheckoutPaymentCompensationPort, in_process_checkout_payment_compensation_port,
};
pub use checkout_execution::{
    AuthorizeCheckoutPaymentCollectionRequest, CaptureCheckoutPaymentCollectionRequest,
    CheckoutPaymentExecutionPort, CheckoutPaymentIdentity, InProcessCheckoutPaymentExecutionPort,
    PrepareCheckoutPaymentCollectionRequest, ReadCheckoutPaymentCollectionRequest,
    in_process_checkout_payment_execution_port,
};
pub use collection_runtime::PaymentCollectionRuntime;
pub use dto::{
    AuthorizePaymentInput, CancelPaymentInput, CancelRefundInput, CapturePaymentInput,
    CompleteRefundInput, CreatePaymentCollectionInput, CreateRefundInput,
    ListPaymentCollectionsInput, ListRefundsInput, PaymentCollectionResponse,
    PaymentCollectionStatusKind, PaymentResponse, PaymentStatusKind, RefundResponse,
    RefundStatusKind,
};
pub use entities::payment as payment_entity;
pub use entities::payment_collection as payment_collection_entity;
pub use error::{PaymentError, PaymentResult};
pub use module::PaymentModule;
pub use order_read::{
    InProcessPaymentOrderReadPort, LatestPaymentCollectionByOrderRequest,
    ListRefundsByOrderRequest, PaymentOrderReadPort, PaymentOrderReadRuntime,
    PaymentOrderRefundPage, in_process_payment_order_read_port,
};
pub use ports::{
    PaymentCollectionCreateOrReuseRequest, PaymentCollectionPort, PaymentCollectionStatusRequest,
    PaymentCollectionStatusSnapshot,
};
pub use providers::{
    ExternalPaymentProviderRegistration, MANUAL_PAYMENT_PROVIDER_ID, ManualPaymentProvider,
    PaymentProvider, PaymentProviderCapabilities, PaymentProviderDegradedMode,
    PaymentProviderDescriptor, PaymentProviderHealth, PaymentProviderOperationRequest,
    PaymentProviderOperationResult, PaymentProviderRegistry, PaymentProviderRuntimeMode,
    PaymentProviderWebhookRequest, PaymentProviderWebhookResult,
};
pub use services::{
    BeginProviderOperation, ChargebackLifecycleEventApplier, CheckoutAdmissionDecision,
    CheckoutAdmissionLinkState, CheckoutAdmissionRefusal, CheckoutExecutionAdmissionPort,
    CheckoutExecutionAdmissionRecord, CheckpointProviderEvent, CompleteProviderEvent,
    FailProviderEvent, PROVIDER_EVENT_DEAD_LETTER, PROVIDER_EVENT_FAILED, PROVIDER_EVENT_PROCESSED,
    PROVIDER_EVENT_PROCESSING, PROVIDER_EVENT_RECEIVED, PROVIDER_OPERATION_COMMITTED,
    PROVIDER_OPERATION_ERROR, PROVIDER_OPERATION_EXECUTING, PROVIDER_OPERATION_PENDING,
    PROVIDER_OPERATION_RECONCILIATION_REQUIRED, PROVIDER_OPERATION_SUCCEEDED,
    PaymentDomainEventApplier, PaymentLifecycleEventApplier, PaymentObservedDomainEventApplier,
    PaymentProviderEventApplier, PaymentProviderEventApplyError, PaymentProviderEventContext,
    PaymentProviderEventExecution, PaymentProviderEventIngressError,
    PaymentProviderEventIngressResult, PaymentProviderEventIngressService,
    PaymentProviderEventJournal, PaymentProviderEventObservers,
    PaymentProviderEventRecoveryFailure, PaymentProviderEventRecoveryOutcome,
    PaymentProviderEventRecoveryReport, PaymentProviderEventRecoveryService,
    PaymentProviderOperationJournal, PaymentProviderProcessedEventObserver,
    PaymentRefundCreationService, PaymentService, ProviderExecutionAdmission,
    ProviderExecutionEffect, ReceiveProviderEvent, RefundLifecycleEventApplier,
    VerifiedProviderEvent, decide_checkout_admission_claim, execution_admission_refusal_error,
    refusal_metric_operation_label,
};
#[cfg(feature = "stripe")]
pub use stripe_provider::{
    STRIPE_PAYMENT_PROVIDER_ID, StaticStripeCredentialProvider, StripeCredentialProvider,
    StripeCredentials, StripePaymentProvider, StripePaymentProviderConfig,
};
