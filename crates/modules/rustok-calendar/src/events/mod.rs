/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Transactional Outbox domain events for calendar.
//!
//! All state mutations emit typed events through `rustok-outbox` for
//! asynchronous distribution, cache invalidation, and CQRS projection updates.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Canonical event topic for all calendar domain events.
pub const CALENDAR_EVENT_TOPIC: &str = "calendar.events";

pub const CALENDAR_EVENT_CREATED: &str = "calendar.created";
pub const CALENDAR_EVENT_UPDATED: &str = "calendar.updated";
pub const CALENDAR_EVENT_DELETED: &str = "calendar.deleted";

/// Published when a new calendar record is created.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarCreatedEvent {
    pub tenant_id: Uuid,
    pub entity_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

/// Published when an existing calendar record is updated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarUpdatedEvent {
    pub tenant_id: Uuid,
    pub entity_id: Uuid,
    pub name: Option<String>,
    pub updated_at: DateTime<Utc>,
}

/// Published when a calendar record is deleted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarDeletedEvent {
    pub tenant_id: Uuid,
    pub entity_id: Uuid,
    pub deleted_at: DateTime<Utc>,
}

/// Enumeration of all domain events emitted by calendar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CalendarDomainEvent {
    Created(CalendarCreatedEvent),
    Updated(CalendarUpdatedEvent),
    Deleted(CalendarDeletedEvent),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_serde_roundtrip() {
        let now = Utc::now();
        let tenant_id = Uuid::new_v4();
        let entity_id = Uuid::new_v4();

        let event = CalendarCreatedEvent {
            tenant_id,
            entity_id,
            name: "Test entity".into(),
            created_at: now,
        };

        let json = serde_json::to_string(&event).expect("serialize event");
        let decoded: CalendarCreatedEvent = serde_json::from_str(&json).expect("deserialize event");
        assert_eq!(event, decoded);

        let domain_event = CalendarDomainEvent::Created(event);
        let domain_json = serde_json::to_string(&domain_event).expect("serialize domain event");
        let domain_decoded: CalendarDomainEvent =
            serde_json::from_str(&domain_json).expect("deserialize domain event");
        assert_eq!(domain_event, domain_decoded);
    }
}
