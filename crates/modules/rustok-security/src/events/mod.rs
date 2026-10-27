/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Transactional Outbox domain events for security.
//!
//! All state mutations emit typed events through `rustok-outbox` for
//! asynchronous distribution, cache invalidation, and CQRS projection updates.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Canonical event topic for all security domain events.
pub const SECURITY_EVENT_TOPIC: &str = "security.events";

pub const SECURITY_EVENT_CREATED: &str = "security.created";
pub const SECURITY_EVENT_UPDATED: &str = "security.updated";
pub const SECURITY_EVENT_DELETED: &str = "security.deleted";

/// Published when a new security record is created.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityCreatedEvent {
    pub tenant_id: Uuid,
    pub entity_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

/// Published when an existing security record is updated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityUpdatedEvent {
    pub tenant_id: Uuid,
    pub entity_id: Uuid,
    pub name: Option<String>,
    pub updated_at: DateTime<Utc>,
}

/// Published when a security record is deleted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityDeletedEvent {
    pub tenant_id: Uuid,
    pub entity_id: Uuid,
    pub deleted_at: DateTime<Utc>,
}

/// Enumeration of all domain events emitted by security.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SecurityDomainEvent {
    Created(SecurityCreatedEvent),
    Updated(SecurityUpdatedEvent),
    Deleted(SecurityDeletedEvent),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_serde_roundtrip() {
        let now = Utc::now();
        let tenant_id = Uuid::new_v4();
        let entity_id = Uuid::new_v4();

        let event = SecurityCreatedEvent {
            tenant_id,
            entity_id,
            name: "Test entity".into(),
            created_at: now,
        };

        let json = serde_json::to_string(&event).expect("serialize event");
        let decoded: SecurityCreatedEvent = serde_json::from_str(&json).expect("deserialize event");
        assert_eq!(event, decoded);

        let domain_event = SecurityDomainEvent::Created(event);
        let domain_json = serde_json::to_string(&domain_event).expect("serialize domain event");
        let domain_decoded: SecurityDomainEvent =
            serde_json::from_str(&domain_json).expect("deserialize domain event");
        assert_eq!(domain_event, domain_decoded);
    }
}
