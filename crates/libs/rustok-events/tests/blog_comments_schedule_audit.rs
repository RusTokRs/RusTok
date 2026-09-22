use rustok_events::{
    BLOG_COMMENTS_SCHEDULE_AUDIT_EVENT_TYPE, BLOG_COMMENTS_SCHEDULE_AUDIT_SCHEMA_VERSION,
    BLOG_COMMENTS_SCHEDULE_AUDIT_STATE_KEY, BlogCommentsDelegationScheduleAuditEvent,
    ContractEventEnvelope, ContractEventPayload, ValidateEvent, event_schema,
};
use uuid::Uuid;

fn event() -> BlogCommentsDelegationScheduleAuditEvent {
    BlogCommentsDelegationScheduleAuditEvent::ReplacementSucceeded {
        audit_schema_version: BLOG_COMMENTS_SCHEDULE_AUDIT_SCHEMA_VERSION,
        request_id: Uuid::new_v4(),
        state_key: BLOG_COMMENTS_SCHEDULE_AUDIT_STATE_KEY.to_string(),
        occurred_at_unix_ms: 1,
        principal_kind: "service".to_string(),
        operation: "replace_host_schedule".to_string(),
        source: "host_provided".to_string(),
        previous_generation: 1,
        candidate_generation: 2,
    }
}

#[test]
fn registry_exposes_the_blog_comments_schedule_audit_contract() {
    let schema = event_schema(BLOG_COMMENTS_SCHEDULE_AUDIT_EVENT_TYPE)
        .expect("Blog Comments schedule audit event must be registered");
    assert_eq!(schema.version, BLOG_COMMENTS_SCHEDULE_AUDIT_SCHEMA_VERSION);
}

#[test]
fn registered_contract_envelope_round_trips_without_payload_drift() {
    let event = event();
    event.validate().expect("bounded event should validate");
    let request_id = event.request_id();
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let envelope = ContractEventEnvelope::new(tenant_id, Some(actor_id), event)
        .expect("registered Blog audit envelope should validate");

    assert_eq!(
        envelope.event_type(),
        BLOG_COMMENTS_SCHEDULE_AUDIT_EVENT_TYPE
    );
    assert_eq!(
        envelope.schema_version(),
        BLOG_COMMENTS_SCHEDULE_AUDIT_SCHEMA_VERSION
    );
    assert_eq!(envelope.tenant_id(), tenant_id);

    let encoded = serde_json::to_vec(&envelope).expect("envelope should serialize");
    let decoded: ContractEventEnvelope =
        serde_json::from_slice(&encoded).expect("envelope should deserialize");
    decoded
        .validate_registered_schema()
        .expect("decoded envelope should remain registered");

    match decoded.payload().expect("decoded payload should validate") {
        ContractEventPayload::BlogCommentsDelegationScheduleAudit(event) => {
            assert_eq!(event.request_id(), request_id);
        }
        other => panic!("unexpected contract payload family: {other:?}"),
    }
}
