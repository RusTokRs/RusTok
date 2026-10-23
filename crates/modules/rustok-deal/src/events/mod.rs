/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Transactional Outbox domain events for deal.
//!
//! All state mutations emit typed events through `rustok-outbox` for
//! asynchronous distribution, cache invalidation, and CQRS projection updates.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Canonical event topic for all deal domain events.
pub const DEAL_EVENT_TOPIC: &str = "deal.events";

pub const DEAL_EVENT_CREATED: &str = "deal.created";
pub const DEAL_EVENT_UPDATED: &str = "deal.updated";
pub const DEAL_EVENT_DELETED: &str = "deal.deleted";

/// Published when a new deal record is created.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DealCreatedEvent {
    pub tenant_id: Uuid,
    pub entity_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

/// Published when an existing deal record is updated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DealUpdatedEvent {
    pub tenant_id: Uuid,
    pub entity_id: Uuid,
    pub name: Option<String>,
    pub updated_at: DateTime<Utc>,
}

/// Published when a deal record is deleted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DealDeletedEvent {
    pub tenant_id: Uuid,
    pub entity_id: Uuid,
    pub deleted_at: DateTime<Utc>,
}

/// Enumeration of all domain events emitted by deal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DealDomainEvent {
    Created(DealCreatedEvent),
    Updated(DealUpdatedEvent),
    Deleted(DealDeletedEvent),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_serde_roundtrip() {
        let now = Utc::now();
        let tenant_id = Uuid::new_v4();
        let entity_id = Uuid::new_v4();

        let event = DealCreatedEvent {
            tenant_id,
            entity_id,
            name: "Test entity".into(),
            created_at: now,
        };

        let json = serde_json::to_string(&event).expect("serialize event");
        let decoded: DealCreatedEvent = serde_json::from_str(&json).expect("deserialize event");
        assert_eq!(event, decoded);

        let domain_event = DealDomainEvent::Created(event);
        let domain_json = serde_json::to_string(&domain_event).expect("serialize domain event");
        let domain_decoded: DealDomainEvent =
            serde_json::from_str(&domain_json).expect("deserialize domain event");
        assert_eq!(domain_event, domain_decoded);
    }
}
