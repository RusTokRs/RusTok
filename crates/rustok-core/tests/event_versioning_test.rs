use rustok_core::events::{DomainEvent, EventEnvelope};
use uuid::Uuid;

#[test]
fn test_event_envelope_has_version_fields() {
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();

    let event = DomainEvent::NodeCreated {
        node_id,
        kind: "post".to_string(),
        author_id: Some(user_id),
    };

    let envelope = EventEnvelope::new(tenant_id, Some(user_id), event);

    // Should have event_type field
    assert_eq!(envelope.event_type, "node.created");

    // Should have schema_version field
    assert_eq!(envelope.schema_version, 1);

    // Should match the event's schema version
    assert_eq!(envelope.schema_version, envelope.event.schema_version());
}

#[test]
fn test_all_events_have_schema_version() {
    let id = Uuid::new_v4();

    let events = vec![
        // Content events
        DomainEvent::NodeCreated {
            node_id: id,
            kind: "post".to_string(),
            author_id: None,
        },
        DomainEvent::NodeUpdated {
            node_id: id,
            kind: "post".to_string(),
        },
        DomainEvent::NodePublished {
            node_id: id,
            kind: "post".to_string(),
        },
        // Category events
        DomainEvent::CategoryCreated { category_id: id },
        DomainEvent::CategoryUpdated { category_id: id },
        // Tag events
        DomainEvent::TagCreated { tag_id: id },
        // User events
        DomainEvent::UserAccountRegistered { user_id: id },
        DomainEvent::UserLoggedIn { user_id: id },
        // Commerce events
        DomainEvent::ProductCreated { product_id: id },
        DomainEvent::OrderPlaced {
            order_id: id,
            customer_id: Some(id),
            total: 1000,
            currency: "USD".to_string(),
        },
        // Index events
        DomainEvent::IndexUpdated {
            index_name: "nodes".to_string(),
            target_id: id,
        },
        // Tenant events
        DomainEvent::TenantCreated { tenant_id: id },
    ];

    for event in events {
        let version = event.schema_version();
        assert!(
            version >= 1,
            "Event {:?} should have version >= 1, got {}",
            event.event_type(),
            version
        );
    }
}

#[test]
fn test_schema_version_consistency() {
    // All current events should be at version 1
    let id = Uuid::new_v4();

    let event = DomainEvent::NodeCreated {
        node_id: id,
        kind: "post".to_string(),
        author_id: None,
    };

    assert_eq!(event.schema_version(), 1);

    // When we update an event structure, we should increment the version
    // This test will fail if someone changes the version without updating the test
    let order_event = DomainEvent::OrderPlaced {
        order_id: id,
        customer_id: Some(id),
        total: 1000,
        currency: "USD".to_string(),
    };

    assert_eq!(order_event.schema_version(), 1);
}

#[test]
fn test_event_type_matches_schema_version() {
    let id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();

    let event = DomainEvent::NodeCreated {
        node_id: id,
        kind: "post".to_string(),
        author_id: None,
    };

    let event_type = event.event_type();
    let schema_version = event.schema_version();

    let envelope = EventEnvelope::new(tenant_id, None, event);

    // Envelope should store both fields correctly
    assert_eq!(envelope.event_type, event_type);
    assert_eq!(envelope.schema_version, schema_version);
}

#[test]
fn test_envelope_serialization_includes_version() {
    let tenant_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();

    let event = DomainEvent::NodeCreated {
        node_id,
        kind: "post".to_string(),
        author_id: None,
    };

    let envelope = EventEnvelope::new(tenant_id, None, event);

    // Serialize to JSON
    let json = serde_json::to_string(&envelope).expect("Should serialize");

    // Should contain event_type and schema_version fields
    assert!(json.contains("event_type"));
    assert!(json.contains("schema_version"));
    assert!(json.contains("node.created"));
}
