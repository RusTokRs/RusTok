//! Transport-agnostic helpers for building Flex standalone events.

use rustok_events::{DomainEvent, EventEnvelope};
use uuid::Uuid;

/// Build `flex.schema.created` envelope.
pub fn flex_schema_created_event(
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    schema_id: Uuid,
    slug: impl Into<String>,
) -> EventEnvelope {
    EventEnvelope::new(
        tenant_id,
        actor_id,
        DomainEvent::FlexSchemaCreated {
            tenant_id,
            schema_id,
            slug: slug.into(),
        },
    )
}

/// Build `flex.schema.updated` envelope.
pub fn flex_schema_updated_event(
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    schema_id: Uuid,
    slug: impl Into<String>,
) -> EventEnvelope {
    EventEnvelope::new(
        tenant_id,
        actor_id,
        DomainEvent::FlexSchemaUpdated {
            tenant_id,
            schema_id,
            slug: slug.into(),
        },
    )
}

/// Build `flex.schema.deleted` envelope.
pub fn flex_schema_deleted_event(
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    schema_id: Uuid,
) -> EventEnvelope {
    EventEnvelope::new(
        tenant_id,
        actor_id,
        DomainEvent::FlexSchemaDeleted {
            tenant_id,
            schema_id,
        },
    )
}

/// Build `flex.entry.created` envelope.
pub fn flex_entry_created_event(
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    schema_id: Uuid,
    entry_id: Uuid,
    entity_type: Option<String>,
    entity_id: Option<Uuid>,
) -> EventEnvelope {
    EventEnvelope::new(
        tenant_id,
        actor_id,
        DomainEvent::FlexEntryCreated {
            tenant_id,
            schema_id,
            entry_id,
            entity_type,
            entity_id,
        },
    )
}

/// Build `flex.entry.updated` envelope.
pub fn flex_entry_updated_event(
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    schema_id: Uuid,
    entry_id: Uuid,
) -> EventEnvelope {
    EventEnvelope::new(
        tenant_id,
        actor_id,
        DomainEvent::FlexEntryUpdated {
            tenant_id,
            schema_id,
            entry_id,
        },
    )
}

/// Build `flex.entry.deleted` envelope.
pub fn flex_entry_deleted_event(
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    schema_id: Uuid,
    entry_id: Uuid,
) -> EventEnvelope {
    EventEnvelope::new(
        tenant_id,
        actor_id,
        DomainEvent::FlexEntryDeleted {
            tenant_id,
            schema_id,
            entry_id,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_schema_event_envelope() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Some(Uuid::new_v4());
        let schema_id = Uuid::new_v4();

        let envelope = flex_schema_created_event(tenant_id, actor_id, schema_id, "landing_page");

        assert_eq!(envelope.event_type, "flex.schema.created");
        assert_eq!(envelope.tenant_id, tenant_id);
        assert_eq!(envelope.actor_id, actor_id);

        match envelope.event {
            DomainEvent::FlexSchemaCreated {
                tenant_id: ev_tenant,
                schema_id: ev_schema,
                slug,
            } => {
                assert_eq!(ev_tenant, tenant_id);
                assert_eq!(ev_schema, schema_id);
                assert_eq!(slug, "landing_page");
            }
            _ => panic!("unexpected event variant"),
        }
    }

    #[test]
    fn creates_entry_event_envelope() {
        let tenant_id = Uuid::new_v4();
        let schema_id = Uuid::new_v4();
        let entry_id = Uuid::new_v4();
        let entity_id = Uuid::new_v4();

        let envelope = flex_entry_created_event(
            tenant_id,
            None,
            schema_id,
            entry_id,
            Some("product".to_string()),
            Some(entity_id),
        );

        assert_eq!(envelope.event_type, "flex.entry.created");

        match envelope.event {
            DomainEvent::FlexEntryCreated {
                tenant_id: ev_tenant,
                schema_id: ev_schema,
                entry_id: ev_entry,
                entity_type,
                entity_id: ev_entity_id,
            } => {
                assert_eq!(ev_tenant, tenant_id);
                assert_eq!(ev_schema, schema_id);
                assert_eq!(ev_entry, entry_id);
                assert_eq!(entity_type.as_deref(), Some("product"));
                assert_eq!(ev_entity_id, Some(entity_id));
            }
            _ => panic!("unexpected event variant"),
        }
    }
}
