use rustok_events::{
    CHECKOUT_OPERATION_ADMISSIONS, CHECKOUT_OPERATION_EVENT_SCHEMAS, CHECKOUT_OPERATION_OUTCOMES,
    CHECKOUT_OPERATION_PARK_REASONS, CheckoutOperationEvent, ContractEventEnvelope,
    ContractEventPayload, ValidateEvent, event_schema, event_schemas,
};
use uuid::Uuid;

fn parked() -> CheckoutOperationEvent {
    CheckoutOperationEvent::Parked {
        operation_id: Uuid::from_u128(1),
        cart_id: Uuid::from_u128(2),
        reason: "manual_reconciliation".to_string(),
    }
}

fn admission_changed() -> CheckoutOperationEvent {
    CheckoutOperationEvent::AdmissionChanged {
        operation_id: Uuid::from_u128(1),
        cart_id: Uuid::from_u128(2),
        previous_admission: Some(CHECKOUT_OPERATION_ADMISSIONS[0].to_string()),
        admission: CHECKOUT_OPERATION_ADMISSIONS[1].to_string(),
        admission_epoch: 2,
        status: "compensating".to_string(),
    }
}

fn reconciled() -> CheckoutOperationEvent {
    CheckoutOperationEvent::Reconciled {
        operation_id: Uuid::from_u128(1),
        cart_id: Uuid::from_u128(2),
        outcome: "compensated".to_string(),
        operator_id: Uuid::from_u128(3),
    }
}

#[test]
fn checkout_operation_family_registers_every_schema_v1_contract() {
    assert_eq!(CHECKOUT_OPERATION_EVENT_SCHEMAS.len(), 3);
    let registered = event_schemas()
        .filter(|schema| schema.event_type.starts_with("checkout.operation."))
        .count();
    assert_eq!(registered, 3);
    for schema in CHECKOUT_OPERATION_EVENT_SCHEMAS {
        assert_eq!(schema.version, 1);
        let found = event_schema(schema.event_type).expect("registered event schema");
        assert_eq!(found.event_type, schema.event_type);
        assert_eq!(found.version, schema.version);
    }
}

#[test]
fn parked_event_is_typed_validated_and_enveloped() {
    let event = parked();
    assert_eq!(event.event_type(), "checkout.operation.parked");
    assert_eq!(event.schema_version(), 1);
    event.validate().expect("valid parked event");

    let envelope = ContractEventEnvelope::new(Uuid::from_u128(10), None, event)
        .expect("valid contract envelope");
    assert_eq!(envelope.event_type(), "checkout.operation.parked");
    assert_eq!(envelope.schema_version(), 1);
    assert!(matches!(
        envelope.payload().expect("validated payload"),
        ContractEventPayload::CheckoutOperation(CheckoutOperationEvent::Parked { .. })
    ));
}

#[test]
fn reconciled_event_carries_the_operator_identity_into_the_payload() {
    let operator_id = Uuid::from_u128(3);
    let event = reconciled();
    event.validate().expect("valid reconciled event");

    let envelope = ContractEventEnvelope::new(Uuid::from_u128(10), Some(operator_id), event)
        .expect("valid contract envelope");
    assert_eq!(envelope.event_type(), "checkout.operation.reconciled");
    match envelope.into_payload().expect("validated payload") {
        ContractEventPayload::CheckoutOperation(CheckoutOperationEvent::Reconciled {
            operator_id: payload_operator,
            outcome,
            ..
        }) => {
            assert_eq!(payload_operator, operator_id);
            assert_eq!(outcome, "compensated");
        }
        other => panic!("unexpected contract payload: {other:?}"),
    }
}

#[test]
fn admission_changed_event_registers_the_bounded_level_vocabulary() {
    assert_eq!(
        CHECKOUT_OPERATION_ADMISSIONS.to_vec(),
        vec!["open", "settling", "closed"]
    );

    let event = admission_changed();
    assert_eq!(event.event_type(), "checkout.operation.admission_changed");
    assert_eq!(event.schema_version(), 1);
    event.validate().expect("valid admission changed event");

    let envelope = ContractEventEnvelope::new(Uuid::from_u128(10), None, event)
        .expect("valid contract envelope");
    assert_eq!(
        envelope.event_type(),
        "checkout.operation.admission_changed"
    );
    match envelope.into_payload().expect("validated payload") {
        ContractEventPayload::CheckoutOperation(CheckoutOperationEvent::AdmissionChanged {
            previous_admission,
            admission,
            admission_epoch,
            status,
            ..
        }) => {
            assert_eq!(previous_admission.as_deref(), Some("open"));
            assert_eq!(admission, "settling");
            assert_eq!(admission_epoch, 2);
            assert_eq!(status, "compensating");
        }
        other => panic!("unexpected contract payload: {other:?}"),
    }

    for admission in CHECKOUT_OPERATION_ADMISSIONS {
        CheckoutOperationEvent::AdmissionChanged {
            operation_id: Uuid::from_u128(1),
            cart_id: Uuid::from_u128(2),
            previous_admission: None,
            admission: (*admission).to_string(),
            admission_epoch: 1,
            status: "pending".to_string(),
        }
        .validate()
        .expect("every documented admission level must validate");
    }
}

#[test]
fn park_reasons_and_outcomes_are_bounded_to_one_vocabulary() {
    assert_eq!(
        CHECKOUT_OPERATION_PARK_REASONS.to_vec(),
        vec!["manual_reconciliation", "attempts_exhausted"]
    );
    assert_eq!(
        CHECKOUT_OPERATION_OUTCOMES.to_vec(),
        vec!["compensation_required", "compensated", "failed"]
    );

    for reason in CHECKOUT_OPERATION_PARK_REASONS {
        CheckoutOperationEvent::Parked {
            operation_id: Uuid::from_u128(1),
            cart_id: Uuid::from_u128(2),
            reason: (*reason).to_string(),
        }
        .validate()
        .expect("every documented park reason must validate");
    }
    for outcome in CHECKOUT_OPERATION_OUTCOMES {
        CheckoutOperationEvent::Reconciled {
            operation_id: Uuid::from_u128(1),
            cart_id: Uuid::from_u128(2),
            outcome: (*outcome).to_string(),
            operator_id: Uuid::from_u128(3),
        }
        .validate()
        .expect("every documented outcome must validate");
    }
}

#[test]
fn unknown_labels_and_nil_identifiers_are_rejected() {
    for invalid in [
        CheckoutOperationEvent::Parked {
            operation_id: Uuid::from_u128(1),
            cart_id: Uuid::from_u128(2),
            reason: "provider said so".to_string(),
        },
        CheckoutOperationEvent::Parked {
            operation_id: Uuid::nil(),
            cart_id: Uuid::from_u128(2),
            reason: "attempts_exhausted".to_string(),
        },
        CheckoutOperationEvent::Reconciled {
            operation_id: Uuid::from_u128(1),
            cart_id: Uuid::from_u128(2),
            outcome: "refunded".to_string(),
            operator_id: Uuid::from_u128(3),
        },
        CheckoutOperationEvent::Reconciled {
            operation_id: Uuid::from_u128(1),
            cart_id: Uuid::from_u128(2),
            outcome: "failed".to_string(),
            operator_id: Uuid::nil(),
        },
        CheckoutOperationEvent::AdmissionChanged {
            operation_id: Uuid::from_u128(1),
            cart_id: Uuid::from_u128(2),
            previous_admission: Some("frozen".to_string()),
            admission: "settling".to_string(),
            admission_epoch: 2,
            status: "compensating".to_string(),
        },
        CheckoutOperationEvent::AdmissionChanged {
            operation_id: Uuid::from_u128(1),
            cart_id: Uuid::from_u128(2),
            previous_admission: None,
            admission: "settling".to_string(),
            admission_epoch: 0,
            status: "compensating".to_string(),
        },
    ] {
        assert!(
            invalid.validate().is_err(),
            "invalid checkout operation event must not validate: {invalid:?}"
        );
    }
}
