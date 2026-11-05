use rustok_events::{
    ContractEventEnvelope, ContractEventPayload, TranslationWorkflowEvent, ValidateEvent,
    event_schema, event_schemas,
};
use uuid::Uuid;

#[test]
fn workflow_family_registers_all_content_free_contracts() {
    let schemas = event_schemas()
        .filter(|schema| schema.event_type.starts_with("translation."))
        .collect::<Vec<_>>();
    assert_eq!(schemas.len(), 15);
    assert!(event_schema("translation.job.created").is_some());
    assert!(event_schema("translation.job.completed").is_some());
    assert!(event_schema("translation.item.retry_requested").is_some());
    assert!(event_schema("translation.apply.completed").is_some());
    assert!(event_schema("translation.apply.recovery_requested").is_some());
    assert!(event_schema("translation.note.created").is_some());
    assert!(event_schema("translation.note.resolved").is_some());

    for event_type in ["translation.note.created", "translation.note.resolved"] {
        let fields = event_schema(event_type).unwrap().fields;
        assert!(fields.iter().any(|field| field.name == "note_id"));
        assert!(!fields.iter().any(|field| field.name == "body"));
    }
}

#[test]
fn assignment_event_is_typed_validated_and_enveloped() {
    let event = TranslationWorkflowEvent::ItemAssigned {
        job_id: Uuid::new_v4(),
        item_id: Uuid::new_v4(),
        assignee_actor_kind: "user".to_string(),
        assignee_actor_id: Uuid::new_v4().to_string(),
        item_revision: 3,
    };
    event.validate().unwrap();
    assert_eq!(event.event_type(), "translation.item.assigned");

    let envelope = ContractEventEnvelope::new(Uuid::new_v4(), None, event).unwrap();
    assert_eq!(envelope.event_type(), "translation.item.assigned");
    assert!(matches!(
        envelope.payload().unwrap(),
        ContractEventPayload::TranslationWorkflow(TranslationWorkflowEvent::ItemAssigned { .. })
    ));
}

#[test]
fn workflow_contract_rejects_invalid_actor_and_attempt() {
    let invalid_actor = TranslationWorkflowEvent::ItemAssigned {
        job_id: Uuid::new_v4(),
        item_id: Uuid::new_v4(),
        assignee_actor_kind: "role".to_string(),
        assignee_actor_id: "manager".to_string(),
        item_revision: 1,
    };
    assert!(invalid_actor.validate().is_err());

    let invalid_attempt = TranslationWorkflowEvent::ApplyFailed {
        operation_id: Uuid::new_v4(),
        item_id: Uuid::new_v4(),
        proposal_id: Uuid::new_v4(),
        status: "pending".to_string(),
        error_code: "translation.owner_unavailable".to_string(),
        retryable: true,
        attempt_count: 0,
    };
    assert!(invalid_attempt.validate().is_err());

    let invalid_retry = TranslationWorkflowEvent::ItemRetryRequested {
        job_id: Uuid::new_v4(),
        item_id: Uuid::new_v4(),
        prior_status: "conflict".to_string(),
        item_revision: 2,
    };
    assert!(invalid_retry.validate().is_err());
}
