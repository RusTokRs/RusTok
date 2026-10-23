/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Transactional Outbox domain events for finance.
//!
//! All state mutations emit typed events through `rustok-outbox` for
//! asynchronous distribution, cache invalidation, and CQRS projection updates.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Canonical event topic for all finance domain events.
pub const FINANCE_EVENT_TOPIC: &str = "finance.events";

pub const FINANCE_EVENT_CREATED: &str = "finance.created";
pub const FINANCE_EVENT_UPDATED: &str = "finance.updated";
pub const FINANCE_EVENT_DELETED: &str = "finance.deleted";

/// Published when a new finance record is created.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinanceCreatedEvent {
    pub tenant_id: Uuid,
    pub entity_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

/// Published when an existing finance record is updated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinanceUpdatedEvent {
    pub tenant_id: Uuid,
    pub entity_id: Uuid,
    pub name: Option<String>,
    pub updated_at: DateTime<Utc>,
}

/// Published when a finance record is deleted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinanceDeletedEvent {
    pub tenant_id: Uuid,
    pub entity_id: Uuid,
    pub deleted_at: DateTime<Utc>,
}

/// Enumeration of all domain events emitted by finance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FinanceDomainEvent {
    Created(FinanceCreatedEvent),
    Updated(FinanceUpdatedEvent),
    Deleted(FinanceDeletedEvent),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_serde_roundtrip() {
        let now = Utc::now();
        let tenant_id = Uuid::new_v4();
        let entity_id = Uuid::new_v4();

        let event = FinanceCreatedEvent {
            tenant_id,
            entity_id,
            name: "Test entity".into(),
            created_at: now,
        };

        let json = serde_json::to_string(&event).expect("serialize event");
        let decoded: FinanceCreatedEvent = serde_json::from_str(&json).expect("deserialize event");
        assert_eq!(event, decoded);

        let domain_event = FinanceDomainEvent::Created(event);
        let domain_json = serde_json::to_string(&domain_event).expect("serialize domain event");
        let domain_decoded: FinanceDomainEvent =
            serde_json::from_str(&domain_json).expect("deserialize domain event");
        assert_eq!(domain_event, domain_decoded);
    }
}
