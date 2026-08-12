use rustok_events::{
    ContractEventEnvelope, ContractEventPayload, ReactionsEvent, ValidateEvent, event_schema,
    event_schemas,
};
use uuid::Uuid;

#[test]
fn reactions_family_registers_both_schema_v1_contracts() {
    let schemas = event_schemas()
        .filter(|schema| schema.event_type.starts_with("reactions."))
        .collect::<Vec<_>>();
    assert_eq!(schemas.len(), 2);
    assert!(event_schema("reactions.actor_state.changed").is_some());
    assert!(event_schema("reactions.subject.reconciled").is_some());
}

#[test]
fn reactions_actor_state_change_is_typed_validated_and_enveloped() {
    let event = ReactionsEvent::ActorStateChanged {
        command_id: Uuid::new_v4(),
        source_slug: "forum".to_string(),
        subject_kind: "topic".to_string(),
        subject_id: Uuid::new_v4(),
        subject_revision: 4,
        actor_id: Uuid::new_v4(),
        requested_reaction: "love".to_string(),
        action: "add".to_string(),
        state_revision: 2,
        selected_keys: vec!["love".to_string()],
        added_keys: vec!["love".to_string()],
        removed_keys: vec!["like".to_string()],
    };
    event.validate().unwrap();

    let envelope_id = Uuid::new_v4();
    let envelope =
        ContractEventEnvelope::new_with_envelope_id(envelope_id, Uuid::new_v4(), None, event)
            .unwrap();
    assert_eq!(envelope.id(), envelope_id);
    assert_eq!(envelope.event_type(), "reactions.actor_state.changed");
    assert!(matches!(
        envelope.payload().unwrap(),
        ContractEventPayload::Reactions(ReactionsEvent::ActorStateChanged { .. })
    ));
}

#[test]
fn reactions_actor_state_change_rejects_noop_and_overlapping_deltas() {
    let noop = ReactionsEvent::ActorStateChanged {
        command_id: Uuid::new_v4(),
        source_slug: "forum".to_string(),
        subject_kind: "reply".to_string(),
        subject_id: Uuid::new_v4(),
        subject_revision: 1,
        actor_id: Uuid::new_v4(),
        requested_reaction: "like".to_string(),
        action: "add".to_string(),
        state_revision: 1,
        selected_keys: vec!["like".to_string()],
        added_keys: Vec::new(),
        removed_keys: Vec::new(),
    };
    assert!(noop.validate().is_err());

    let overlapping = ReactionsEvent::ActorStateChanged {
        command_id: Uuid::new_v4(),
        source_slug: "forum".to_string(),
        subject_kind: "reply".to_string(),
        subject_id: Uuid::new_v4(),
        subject_revision: 1,
        actor_id: Uuid::new_v4(),
        requested_reaction: "like".to_string(),
        action: "remove".to_string(),
        state_revision: 2,
        selected_keys: vec!["like".to_string()],
        added_keys: Vec::new(),
        removed_keys: vec!["like".to_string()],
    };
    assert!(overlapping.validate().is_err());
}

#[test]
fn reactions_reconciled_event_requires_truthful_bounded_sample() {
    let valid = ReactionsEvent::SubjectReconciled {
        repair_command_id: Uuid::new_v4(),
        source_slug: "forum".to_string(),
        subject_kind: "topic".to_string(),
        subject_id: Uuid::new_v4(),
        subject_revision: 9,
        catalog_revision: 9,
        actor_states_scanned: 12,
        aggregate_rows_before: 3,
        aggregate_rows_after: 2,
        changed_key_count: 2,
        changed_keys: vec!["like".to_string(), "love".to_string()],
        changed_keys_truncated: false,
    };
    valid.validate().unwrap();

    let invalid = ReactionsEvent::SubjectReconciled {
        repair_command_id: Uuid::new_v4(),
        source_slug: "forum".to_string(),
        subject_kind: "topic".to_string(),
        subject_id: Uuid::new_v4(),
        subject_revision: 9,
        catalog_revision: 9,
        actor_states_scanned: 12,
        aggregate_rows_before: 3,
        aggregate_rows_after: 2,
        changed_key_count: 2,
        changed_keys: vec!["like".to_string()],
        changed_keys_truncated: false,
    };
    assert!(invalid.validate().is_err());
}
