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
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

use crate::migration::SysEventsMigration;

/// Core outbox module — transactional event persistence and relay infrastructure.
pub struct OutboxModule;

impl MigrationSource for OutboxModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(SysEventsMigration)]
    }
}

#[async_trait]
impl RusToKModule for OutboxModule {
    fn slug(&self) -> &'static str {
        "outbox"
    }

    fn name(&self) -> &'static str {
        "Outbox"
    }

    fn description(&self) -> &'static str {
        "Transactional event persistence, relay, retry, DLQ, and delivered-event retention."
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Core
    }

    async fn health(&self) -> HealthStatus {
        // Module-level health has no host AppContext, so it cannot inspect
        // sys_events, relay worker state, backlog, lag or DLQ. The server
        // readiness layer owns those concrete checks.
        HealthStatus::Degraded
    }
}

#[cfg(test)]
mod health_tests {
    use super::OutboxModule;
    use rustok_core::module::{HealthStatus, RusToKModule};

    #[tokio::test]
    async fn outbox_module_health_defers_to_host_readiness() {
        assert_eq!(OutboxModule.health().await, HealthStatus::Degraded);
    }
}
