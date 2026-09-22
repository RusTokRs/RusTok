use rustok_events::{
    ContractEventEnvelope, ContractEventPayload, SOCIAL_GRAPH_RELATION_EVENT_SCHEMAS,
    SocialGraphRelationEvent, ValidateEvent, event_schema, event_schemas,
};
use uuid::Uuid;

fn event() -> SocialGraphRelationEvent {
    SocialGraphRelationEvent::RelationStateChanged {
        relation_id: Uuid::from_u128(1),
        source_user_id: Uuid::from_u128(2),
        target_user_id: Uuid::from_u128(3),
        relation_kind: "follow".to_string(),
        active: true,
        revision: 1,
    }
}

#[test]
fn social_graph_family_has_one_registered_versioned_contract() {
    assert_eq!(SOCIAL_GRAPH_RELATION_EVENT_SCHEMAS.len(), 1);
    let registered = event_schemas()
        .filter(|schema| schema.event_type.starts_with("social_graph.relation."))
        .count();
    assert_eq!(registered, 1);
    let schema = event_schema("social_graph.relation.state_changed")
        .expect("registered social graph event schema");
    assert_eq!(schema.version, 1);
}

#[test]
fn social_graph_contract_is_typed_validated_and_enveloped() {
    let event = event();
    assert_eq!(event.event_type(), "social_graph.relation.state_changed");
    assert_eq!(event.schema_version(), 1);
    event.validate().expect("valid social graph event");

    let envelope =
        ContractEventEnvelope::new(Uuid::from_u128(10), Some(Uuid::from_u128(11)), event)
            .expect("valid contract envelope");
    assert_eq!(envelope.event_type(), "social_graph.relation.state_changed");
    assert_eq!(envelope.schema_version(), 1);
    assert!(matches!(
        envelope.payload().expect("validated payload"),
        ContractEventPayload::SocialGraphRelation(
            SocialGraphRelationEvent::RelationStateChanged { .. }
        )
    ));
}

#[test]
fn social_graph_contract_rejects_invalid_identity_kind_and_revision() {
    for invalid in [
        SocialGraphRelationEvent::RelationStateChanged {
            relation_id: Uuid::nil(),
            source_user_id: Uuid::from_u128(2),
            target_user_id: Uuid::from_u128(3),
            relation_kind: "follow".to_string(),
            active: true,
            revision: 1,
        },
        SocialGraphRelationEvent::RelationStateChanged {
            relation_id: Uuid::from_u128(1),
            source_user_id: Uuid::from_u128(2),
            target_user_id: Uuid::from_u128(2),
            relation_kind: "follow".to_string(),
            active: true,
            revision: 1,
        },
        SocialGraphRelationEvent::RelationStateChanged {
            relation_id: Uuid::from_u128(1),
            source_user_id: Uuid::from_u128(2),
            target_user_id: Uuid::from_u128(3),
            relation_kind: "friend".to_string(),
            active: true,
            revision: 1,
        },
        SocialGraphRelationEvent::RelationStateChanged {
            relation_id: Uuid::from_u128(1),
            source_user_id: Uuid::from_u128(2),
            target_user_id: Uuid::from_u128(3),
            relation_kind: "follow".to_string(),
            active: true,
            revision: 0,
        },
    ] {
        assert!(invalid.validate().is_err());
    }
}

#[test]
fn social_graph_external_payload_excludes_command_and_request_metadata() {
    let encoded = serde_json::to_string(&event()).expect("serialize social graph event");
    for forbidden in [
        "idempotency_key",
        "expected_revision",
        "correlation_id",
        "causation_id",
        "traceparent",
        "claims",
        "roles",
        "locale",
        "channel",
    ] {
        assert!(!encoded.contains(forbidden), "payload leaked {forbidden}");
    }
}
