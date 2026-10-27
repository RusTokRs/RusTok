/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Canonical platform owner of shared reaction catalog, actor state, aggregate, and subject orchestration in RusToK.
//!
//! # Architecture and Guidelines
//!
//! This crate follows Fluid Backend Architecture (FBA).
//! For design and implementation standards, see:
//! - [Backend Module Implementation Guide](https://github.com/RusTokRs/RusTok/blob/main/docs/backend/module-backend-implementation.md)
//! - [Module Platform Overview](https://github.com/RusTokRs/RusTok/blob/main/docs/modules/overview.md)

pub mod model;
pub mod provider;

pub use model::{
    ApplyReactionCommand, MAX_REACTION_CATALOG_SIZE, MAX_REACTION_KEY_BYTES,
    MAX_REACTION_SOURCE_SLUG_BYTES, MAX_REACTION_SUBJECT_KIND_BYTES, MAX_REACTIONS_PER_ACTOR,
    ReactionAction, ReactionActorState, ReactionAggregate, ReactionCatalog,
    ReactionCommandIdentity, ReactionContractError, ReactionKey, ReactionReadRequest,
    ReactionSelectionPolicy, ReactionSnapshot, ReactionSourceSlug, ReactionSubjectKind,
    ReactionSubjectRef, ReactionWriteReceipt,
};
pub use provider::{
    MAX_REACTION_PROVIDER_KINDS, ReactionProviderError, ReactionProviderResult, ReactionReadPort,
    ReactionSubjectAccess, ReactionSubjectAuthorization, ReactionSubjectDeletionBinding,
    ReactionSubjectFactoryRegistry, ReactionSubjectProvider, ReactionSubjectProviderFactory,
    ReactionSubjectRegistry, ReactionSubjectRegistryEntry, ReactionSubjectRegistryError,
    ReactionSubjectRequest, ReactionWritePort, ensure_reaction_subject_factory_registry,
    ensure_reaction_subject_registry, materialize_reaction_subject_registry,
    reaction_subject_factory_registry_from_extensions, reaction_subject_registry_from_extensions,
    register_reaction_subject_provider, register_reaction_subject_provider_factory,
};

pub mod api {
    pub use crate::model::{
        ApplyReactionCommand, MAX_REACTION_CATALOG_SIZE, MAX_REACTION_KEY_BYTES,
        MAX_REACTION_SOURCE_SLUG_BYTES, MAX_REACTION_SUBJECT_KIND_BYTES, MAX_REACTIONS_PER_ACTOR,
        ReactionAction, ReactionActorState, ReactionAggregate, ReactionCatalog,
        ReactionCommandIdentity, ReactionContractError, ReactionKey, ReactionReadRequest,
        ReactionSelectionPolicy, ReactionSnapshot, ReactionSourceSlug, ReactionSubjectKind,
        ReactionSubjectRef, ReactionWriteReceipt,
    };
    pub use crate::provider::{
        MAX_REACTION_PROVIDER_KINDS, ReactionProviderError, ReactionProviderResult,
        ReactionReadPort, ReactionSubjectAccess, ReactionSubjectAuthorization,
        ReactionSubjectDeletionBinding, ReactionSubjectFactoryRegistry, ReactionSubjectProvider,
        ReactionSubjectProviderFactory, ReactionSubjectRegistry, ReactionSubjectRegistryEntry,
        ReactionSubjectRegistryError, ReactionSubjectRequest, ReactionWritePort,
        ensure_reaction_subject_factory_registry, ensure_reaction_subject_registry,
        materialize_reaction_subject_registry, reaction_subject_factory_registry_from_extensions,
        reaction_subject_registry_from_extensions, register_reaction_subject_provider,
        register_reaction_subject_provider_factory,
    };
}

#[cfg(feature = "server")]
pub mod entities;
#[cfg(all(feature = "server", feature = "graphql"))]
pub mod graphql;
#[cfg(feature = "server")]
pub mod migrations;
#[cfg(feature = "server")]
mod reconciliation;
#[cfg(feature = "server")]
mod service;
#[cfg(feature = "server")]
mod target_lifecycle;

#[cfg(feature = "server")]
pub use reconciliation::{
    MAX_REACTION_RECONCILIATION_ACTOR_STATES, MAX_REACTION_RECONCILIATION_ISSUES,
    ReactionAggregateComparison, ReactionReconciliationIssue, ReactionReconciliationReceipt,
    ReactionReconciliationReport, ReactionReconciliationRequest, ReactionReconciliationStatus,
    RepairReactionSubjectCommand,
};
#[cfg(feature = "server")]
pub use service::ReactionsService;

#[cfg(feature = "server")]
pub mod module;
#[cfg(feature = "server")]
pub use module::ReactionsModule;

#[cfg(all(test, feature = "server"))]
mod contract_tests;
