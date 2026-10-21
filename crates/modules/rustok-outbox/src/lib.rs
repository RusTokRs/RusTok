/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! # RusToK Outbox
//!
//! Transactional event persistence, reliable outbox relay, DLQ, and event retention
//! infrastructure for the RusToK platform.
//!
//! `rustok-outbox` guarantees atomic event persistence within database transactions
//! (`sys_events` table) and reliable asynchronous dispatch through background relay workers,
//! preventing dual-write inconsistencies and phantom events.
//!
//! ## Core Architecture & Modules
//!
//! - **Transactional Bus**: [`TransactionalEventBus`] provides transactional publish methods
//!   for root events and bounded typed contract events within caller transactions.
//! - **Relay Engine**: [`OutboxRelay`] polls, claims, batches, and dispatches pending events with
//!   lease-based concurrency controls and dead-letter queue (DLQ) transitions.
//! - **Idempotency & Write-Once**: [`ContractEventWriteOnceError`] and idempotency mechanics
//!   enforce exact-replay matching and detect conflict reuse.
//! - **Retention & Pruning**: [`OutboxRetention`] and [`OutboxRetentionConfig`] clean up
//!   dispatched events based on tenant or platform retention windows.
//! - **Module Entry Point**: [`OutboxModule`] exposes canonical module metadata and migrations.

pub mod entity;
pub mod idempotency;
pub mod migration;
pub mod module;
pub mod ports;
pub mod relay;
pub mod retention;
pub mod transactional;
pub mod transport;

pub use entity::{Entity as SysEvents, Model as SysEvent};
pub use migration::SysEventsMigration;
pub use module::OutboxModule;
pub use ports::{
    OutboxRelayPort, OutboxRelayRunOnceProjection, OutboxRelayRunOnceRequest,
    TransactionalEventWriter, require_outbox_relay_policy,
};
pub use relay::{OutboxRelay, RelayConfig, RelayMetricsSnapshot};
pub use retention::{
    DEFAULT_OUTBOX_RETENTION_BATCH_SIZE, OutboxPruneReport, OutboxRetention, OutboxRetentionConfig,
};
pub use transactional::TransactionalEventBus;
pub use transport::{ContractEventWriteOnceError, OutboxTransport};

#[cfg(test)]
mod contract_tests;
