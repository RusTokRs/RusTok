//! # rustok-moderation
//!
//! Cross-domain moderation owner for reports, cases, decisions, and auditable enforcement.
//!
//! Provides the core moderation lifecycle, case investigation workflows, typed decision
//! effects, durable application leasing, and subject command ports.

pub mod model;
pub mod provider;

pub use model::{
    moderation_scope_claim, moderation_scope_from_claims, ApplyModerationDecisionCommand,
    ModerationCapabilityKey, ModerationCommandValidationError, ModerationDecisionApplication,
    ModerationDecisionEffect, ModerationDecisionEffectAction, ModerationDecisionKind,
    ModerationEffectValidationError, ModerationReasonCode, ModerationScopeClaimError,
    ModerationScopeKind, ModerationScopeRef, ModerationSubjectKind, ModerationSubjectRef,
    ModerationVisibilityState, MAX_MODERATION_CAPABILITY_KEY_BYTES,
    MAX_MODERATION_EFFECT_CAPABILITIES, MODERATION_DECISION_EFFECT_SCHEMA_V1,
    MODERATION_SCOPE_CLAIM_PREFIX,
};
pub use provider::{
    materialize_moderation_subject_adapter_registry,
    moderation_subject_adapter_registry_from_extensions, register_moderation_subject_adapter,
    register_moderation_subject_adapter_factory, ModerationSubjectAdapterBuildError,
    ModerationSubjectAdapterFactory, ModerationSubjectAdapterFactoryRegistry,
    ModerationSubjectAdapterKey, ModerationSubjectAdapterRegistry,
    ModerationSubjectAdapterRegistryError, ModerationSubjectCommandPort,
};

pub mod api {
    pub use crate::model::{
        moderation_scope_claim, moderation_scope_from_claims, ApplyModerationDecisionCommand,
        ModerationCapabilityKey, ModerationCommandValidationError, ModerationDecisionApplication,
        ModerationDecisionEffect, ModerationDecisionEffectAction, ModerationDecisionKind,
        ModerationEffectValidationError, ModerationReasonCode, ModerationScopeClaimError,
        ModerationScopeKind, ModerationScopeRef, ModerationSubjectKind, ModerationSubjectRef,
        ModerationVisibilityState, MAX_MODERATION_CAPABILITY_KEY_BYTES,
        MAX_MODERATION_EFFECT_CAPABILITIES, MODERATION_DECISION_EFFECT_SCHEMA_V1,
        MODERATION_SCOPE_CLAIM_PREFIX,
    };
    pub use crate::provider::{
        materialize_moderation_subject_adapter_registry,
        moderation_subject_adapter_registry_from_extensions, register_moderation_subject_adapter,
        register_moderation_subject_adapter_factory, ModerationSubjectAdapterBuildError,
        ModerationSubjectAdapterFactory, ModerationSubjectAdapterFactoryRegistry,
        ModerationSubjectAdapterKey, ModerationSubjectAdapterRegistry,
        ModerationSubjectAdapterRegistryError, ModerationSubjectCommandPort,
    };
}


#[cfg(feature = "server")]
pub mod application;
#[cfg(feature = "server")]
pub mod application_dispatch;
#[cfg(feature = "server")]
pub mod application_recovery;
#[cfg(feature = "server")]
mod application_scheduler;
#[cfg(feature = "server")]
mod commands;
#[cfg(feature = "server")]
pub mod domain;
#[cfg(feature = "server")]
pub mod entities;
#[cfg(feature = "server")]
pub mod error;
#[cfg(feature = "server")]
pub mod migrations;
#[cfg(feature = "server")]
pub mod module;
#[cfg(feature = "server")]
pub mod ports;
#[cfg(feature = "server")]
mod receipts;
#[cfg(feature = "server")]
pub mod service;

#[cfg(test)]
mod contract_tests;

#[cfg(feature = "server")]
pub use application::{
    DEFAULT_APPLICATION_LEASE_SECONDS, MAX_APPLICATION_LEASE_SECONDS,
    MAX_APPLICATION_RETRY_SECONDS, MAX_DUE_APPLICATION_OPERATIONS,
};
#[cfg(feature = "server")]
pub use application_dispatch::{
    APPLICATION_ADAPTER_DEADLINE_SECONDS, APPLICATION_RETRY_BASE_SECONDS,
    APPLICATION_RETRY_MAX_SECONDS, application_retry_delay_seconds,
};
#[cfg(feature = "server")]
pub use application_recovery::MAX_APPLICATION_RECOVERY_REASON_BYTES;
#[cfg(feature = "server")]
pub use domain::{
    AssignModerationCaseCommand, DecideModerationCaseCommand, ModerationApplicationOperationRecord,
    ModerationApplicationOperationStatus, ModerationApplicationRecoveryRecord,
    ModerationCasePriority, ModerationCaseRecord, ModerationCaseStatus, ModerationDecisionRecord,
    ModerationQueueFilter, ModerationReportRecord, ModerationReportStatus, ModerationReporterKind,
    OpenModerationCaseCommand, ReconcileLegacyModerationApplicationCommand,
    RequeueModerationApplicationCommand, SubmitModerationReportCommand,
};
#[cfg(feature = "server")]
pub use error::{ModerationError, ModerationResult};
#[cfg(feature = "server")]
pub use module::ModerationModule;
#[cfg(feature = "server")]
pub use ports::{ModerationCommandPort, ModerationReadPort, ModerationRecoveryCommandPort};
#[cfg(feature = "server")]
pub use service::ModerationService;


