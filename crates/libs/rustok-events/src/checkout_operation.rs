use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::contract::{ContractEventPayload, EventContract, sealed};
use crate::validation::{EventValidationError, ValidateEvent, validators};
use crate::{EventSchema, FieldSchema};

/// Park reason for an operation whose compensation cannot be rolled back
/// automatically (captured money, or an owner that refuses the compensation).
///
/// The label is part of the published contract and is also the bounded label of
/// `rustok_checkout_reconciliation_parked_total`, so there is exactly one
/// vocabulary for "why did this checkout park".
pub const CHECKOUT_OPERATION_PARK_REASON_MANUAL_RECONCILIATION: &str = "manual_reconciliation";

/// Park reason for an operation that reached the compensation attempt cap; the
/// sweep parks it instead of retrying in the dark forever.
pub const CHECKOUT_OPERATION_PARK_REASON_ATTEMPTS_EXHAUSTED: &str = "attempts_exhausted";

/// The operator returned the parked operation to the compensation queue.
pub const CHECKOUT_OPERATION_OUTCOME_COMPENSATION_REQUIRED: &str = "compensation_required";
/// The operation closed with the funds returned.
pub const CHECKOUT_OPERATION_OUTCOME_COMPENSATED: &str = "compensated";
/// The operation closed without returning funds (write-off, voided
/// authorization, or an explicitly attested failure).
pub const CHECKOUT_OPERATION_OUTCOME_FAILED: &str = "failed";

/// Bounded park reasons, in contract order.
pub const CHECKOUT_OPERATION_PARK_REASONS: &[&str] = &[
    CHECKOUT_OPERATION_PARK_REASON_MANUAL_RECONCILIATION,
    CHECKOUT_OPERATION_PARK_REASON_ATTEMPTS_EXHAUSTED,
];

/// Bounded outcomes of leaving the parked state, in contract order.
pub const CHECKOUT_OPERATION_OUTCOMES: &[&str] = &[
    CHECKOUT_OPERATION_OUTCOME_COMPENSATION_REQUIRED,
    CHECKOUT_OPERATION_OUTCOME_COMPENSATED,
    CHECKOUT_OPERATION_OUTCOME_FAILED,
];

/// The checkout operation admits extending provider execution
/// (`authorize`, `capture`, new charges).
pub const CHECKOUT_OPERATION_ADMISSION_OPEN: &str = "open";

/// The checkout operation entered compensation: extending provider execution is
/// refused, while unwinding effects (`cancel`, `refund`) stay admitted because
/// the compensation itself needs the provider.
pub const CHECKOUT_OPERATION_ADMISSION_SETTLING: &str = "settling";

/// The checkout operation reached a terminal status: no provider execution may
/// extend the charge. Unwinding effects stay admitted for post-checkout refunds.
pub const CHECKOUT_OPERATION_ADMISSION_CLOSED: &str = "closed";

/// Bounded execution admission levels, in contract order.
///
/// The vocabulary is shared by the checkout journal (single writer of the
/// level), the published `checkout.operation.admission_changed` event, and the
/// `rustok-payment` projection that fences provider execution.
pub const CHECKOUT_OPERATION_ADMISSIONS: &[&str] = &[
    CHECKOUT_OPERATION_ADMISSION_OPEN,
    CHECKOUT_OPERATION_ADMISSION_SETTLING,
    CHECKOUT_OPERATION_ADMISSION_CLOSED,
];

pub const CHECKOUT_OPERATION_PARKED_EVENT_TYPE: &str = "checkout.operation.parked";
pub const CHECKOUT_OPERATION_RECONCILED_EVENT_TYPE: &str = "checkout.operation.reconciled";
pub const CHECKOUT_OPERATION_ADMISSION_CHANGED_EVENT_TYPE: &str =
    "checkout.operation.admission_changed";
pub const CHECKOUT_OPERATION_EVENT_SCHEMA_VERSION: u16 = 1;

/// Money-path events of the checkout operation journal.
///
/// The journal owns the checkout state machine, so it owns these events: there
/// is no third writer that could park, close or re-admit an operation behind its
/// back. Every event is published inside the transaction that changes
/// `checkout_operations`, so a consumer can never observe a state that has no
/// event and a consumer can never be told about a state that was rolled back.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(tag = "type", content = "data")]
pub enum CheckoutOperationEvent {
    /// The operation entered `reconciliation_required` and the cart stays
    /// blocked until an operator acts.
    Parked {
        operation_id: Uuid,
        cart_id: Uuid,
        /// One of [`CHECKOUT_OPERATION_PARK_REASONS`].
        reason: String,
    },
    /// An operator moved the operation out of `reconciliation_required`.
    Reconciled {
        operation_id: Uuid,
        cart_id: Uuid,
        /// One of [`CHECKOUT_OPERATION_OUTCOMES`].
        outcome: String,
        operator_id: Uuid,
    },
    /// The operation's execution admission level changed.
    ///
    /// The level is the checkout-owned half of the provider execution
    /// admission: `rustok-payment` projects it and refuses to start (or resume)
    /// an extending provider operation whose generation is not the admitted one.
    /// A fresh operation for the same cart publishes `previous_admission: None`
    /// together with the next [`CHECKOUT_OPERATION_ADMISSIONS`] generation, so a
    /// projection can rebuild any level from the stream alone.
    AdmissionChanged {
        operation_id: Uuid,
        cart_id: Uuid,
        /// Previous level, `None` when the operation's admission record was
        /// created by `begin`.
        previous_admission: Option<String>,
        /// One of [`CHECKOUT_OPERATION_ADMISSIONS`].
        admission: String,
        /// Monotonically increasing generation of the admission level for the
        /// cart: every level change and every fresh operation increments it.
        admission_epoch: i64,
        /// Journal status that produced the level.
        status: String,
    },
}

impl CheckoutOperationEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::Parked { .. } => CHECKOUT_OPERATION_PARKED_EVENT_TYPE,
            Self::Reconciled { .. } => CHECKOUT_OPERATION_RECONCILED_EVENT_TYPE,
            Self::AdmissionChanged { .. } => CHECKOUT_OPERATION_ADMISSION_CHANGED_EVENT_TYPE,
        }
    }

    pub const fn schema_version(&self) -> u16 {
        CHECKOUT_OPERATION_EVENT_SCHEMA_VERSION
    }
}

const CHECKOUT_OPERATION_PARKED_FIELDS: &[FieldSchema] = &[
    FieldSchema {
        name: "operation_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "cart_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "reason",
        data_type: "string",
        optional: false,
    },
];

const CHECKOUT_OPERATION_RECONCILED_FIELDS: &[FieldSchema] = &[
    FieldSchema {
        name: "operation_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "cart_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "outcome",
        data_type: "string",
        optional: false,
    },
    FieldSchema {
        name: "operator_id",
        data_type: "uuid",
        optional: false,
    },
];

const CHECKOUT_OPERATION_ADMISSION_CHANGED_FIELDS: &[FieldSchema] = &[
    FieldSchema {
        name: "operation_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "cart_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "previous_admission",
        data_type: "string",
        optional: true,
    },
    FieldSchema {
        name: "admission",
        data_type: "string",
        optional: false,
    },
    FieldSchema {
        name: "admission_epoch",
        data_type: "int64",
        optional: false,
    },
    FieldSchema {
        name: "status",
        data_type: "string",
        optional: false,
    },
];

pub const CHECKOUT_OPERATION_EVENT_SCHEMAS: &[EventSchema] = &[
    EventSchema {
        event_type: CHECKOUT_OPERATION_PARKED_EVENT_TYPE,
        version: CHECKOUT_OPERATION_EVENT_SCHEMA_VERSION,
        description: "A checkout operation was parked in reconciliation_required.",
        fields: CHECKOUT_OPERATION_PARKED_FIELDS,
    },
    EventSchema {
        event_type: CHECKOUT_OPERATION_RECONCILED_EVENT_TYPE,
        version: CHECKOUT_OPERATION_EVENT_SCHEMA_VERSION,
        description: "An operator moved a parked checkout operation out of the parked state.",
        fields: CHECKOUT_OPERATION_RECONCILED_FIELDS,
    },
    EventSchema {
        event_type: CHECKOUT_OPERATION_ADMISSION_CHANGED_EVENT_TYPE,
        version: CHECKOUT_OPERATION_EVENT_SCHEMA_VERSION,
        description: "A checkout operation changed its provider execution admission level.",
        fields: CHECKOUT_OPERATION_ADMISSION_CHANGED_FIELDS,
    },
];

impl sealed::Sealed for CheckoutOperationEvent {}

impl EventContract for CheckoutOperationEvent {
    fn event_type(&self) -> &'static str {
        CheckoutOperationEvent::event_type(self)
    }

    fn schema_version(&self) -> u16 {
        CheckoutOperationEvent::schema_version(self)
    }

    fn into_contract_payload(self) -> ContractEventPayload {
        ContractEventPayload::CheckoutOperation(self)
    }
}

impl ValidateEvent for CheckoutOperationEvent {
    fn validate(&self) -> Result<(), EventValidationError> {
        match self {
            Self::Parked {
                operation_id,
                cart_id,
                reason,
            } => {
                validators::validate_not_nil_uuid("operation_id", operation_id)?;
                validators::validate_not_nil_uuid("cart_id", cart_id)?;
                validate_bounded_label("reason", reason, CHECKOUT_OPERATION_PARK_REASONS)
            }
            Self::Reconciled {
                operation_id,
                cart_id,
                outcome,
                operator_id,
            } => {
                validators::validate_not_nil_uuid("operation_id", operation_id)?;
                validators::validate_not_nil_uuid("cart_id", cart_id)?;
                validators::validate_not_nil_uuid("operator_id", operator_id)?;
                validate_bounded_label("outcome", outcome, CHECKOUT_OPERATION_OUTCOMES)
            }
            Self::AdmissionChanged {
                operation_id,
                cart_id,
                previous_admission,
                admission,
                admission_epoch,
                status,
            } => {
                validators::validate_not_nil_uuid("operation_id", operation_id)?;
                validators::validate_not_nil_uuid("cart_id", cart_id)?;
                if let Some(previous_admission) = previous_admission {
                    validate_bounded_label(
                        "previous_admission",
                        previous_admission,
                        CHECKOUT_OPERATION_ADMISSIONS,
                    )?;
                }
                validate_bounded_label("admission", admission, CHECKOUT_OPERATION_ADMISSIONS)?;
                validators::validate_range("admission_epoch", *admission_epoch, 1, i64::MAX)?;
                validators::validate_not_empty("status", status)?;
                validators::validate_max_length("status", status, 32)?;
                validators::validate_alphanumeric_with_dash("status", status)
            }
        }
    }
}

fn validate_bounded_label(
    field_name: &'static str,
    value: &str,
    allowed: &[&str],
) -> Result<(), EventValidationError> {
    if allowed.contains(&value) {
        return Ok(());
    }
    Err(EventValidationError::InvalidValue(
        field_name,
        format!("must be one of: {}", allowed.join(", ")),
    ))
}

pub fn checkout_operation_event_schema(event_type: &str) -> Option<&'static EventSchema> {
    CHECKOUT_OPERATION_EVENT_SCHEMAS
        .iter()
        .find(|schema| schema.event_type == event_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parked_event_accepts_only_the_bounded_park_reasons() {
        for reason in CHECKOUT_OPERATION_PARK_REASONS {
            assert!(
                CheckoutOperationEvent::Parked {
                    operation_id: Uuid::new_v4(),
                    cart_id: Uuid::new_v4(),
                    reason: (*reason).to_string(),
                }
                .validate()
                .is_ok()
            );
        }

        assert!(
            CheckoutOperationEvent::Parked {
                operation_id: Uuid::new_v4(),
                cart_id: Uuid::new_v4(),
                reason: "provider said so".to_string(),
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn reconciled_event_requires_operator_identity_and_bounded_outcome() {
        let cart_id = Uuid::new_v4();
        let operation_id = Uuid::new_v4();
        let operator_id = Uuid::new_v4();

        for outcome in CHECKOUT_OPERATION_OUTCOMES {
            assert!(
                CheckoutOperationEvent::Reconciled {
                    operation_id,
                    cart_id,
                    outcome: (*outcome).to_string(),
                    operator_id,
                }
                .validate()
                .is_ok()
            );
        }

        for invalid in [
            CheckoutOperationEvent::Reconciled {
                operation_id,
                cart_id,
                outcome: CHECKOUT_OPERATION_OUTCOME_COMPENSATED.to_string(),
                operator_id: Uuid::nil(),
            },
            CheckoutOperationEvent::Reconciled {
                operation_id,
                cart_id,
                outcome: "refunded".to_string(),
                operator_id,
            },
        ] {
            assert!(invalid.validate().is_err());
        }
    }

    #[test]
    fn admission_changed_event_accepts_only_bounded_levels_and_positive_epochs() {
        for admission in CHECKOUT_OPERATION_ADMISSIONS {
            assert!(
                CheckoutOperationEvent::AdmissionChanged {
                    operation_id: Uuid::new_v4(),
                    cart_id: Uuid::new_v4(),
                    previous_admission: None,
                    admission: (*admission).to_string(),
                    admission_epoch: 1,
                    status: "pending".to_string(),
                }
                .validate()
                .is_ok()
            );
        }

        for invalid in [
            CheckoutOperationEvent::AdmissionChanged {
                operation_id: Uuid::new_v4(),
                cart_id: Uuid::new_v4(),
                previous_admission: Some("frozen".to_string()),
                admission: CHECKOUT_OPERATION_ADMISSION_OPEN.to_string(),
                admission_epoch: 1,
                status: "pending".to_string(),
            },
            CheckoutOperationEvent::AdmissionChanged {
                operation_id: Uuid::new_v4(),
                cart_id: Uuid::new_v4(),
                previous_admission: None,
                admission: "frozen".to_string(),
                admission_epoch: 1,
                status: "pending".to_string(),
            },
            CheckoutOperationEvent::AdmissionChanged {
                operation_id: Uuid::new_v4(),
                cart_id: Uuid::new_v4(),
                previous_admission: None,
                admission: CHECKOUT_OPERATION_ADMISSION_SETTLING.to_string(),
                admission_epoch: 0,
                status: "compensating".to_string(),
            },
            CheckoutOperationEvent::AdmissionChanged {
                operation_id: Uuid::new_v4(),
                cart_id: Uuid::new_v4(),
                previous_admission: None,
                admission: CHECKOUT_OPERATION_ADMISSION_SETTLING.to_string(),
                admission_epoch: 1,
                status: "compensating now".to_string(),
            },
        ] {
            assert!(invalid.validate().is_err());
        }
    }

    #[test]
    fn every_event_type_is_registered_with_a_schema() {
        for event in [
            CheckoutOperationEvent::Parked {
                operation_id: Uuid::new_v4(),
                cart_id: Uuid::new_v4(),
                reason: CHECKOUT_OPERATION_PARK_REASON_MANUAL_RECONCILIATION.to_string(),
            },
            CheckoutOperationEvent::AdmissionChanged {
                operation_id: Uuid::new_v4(),
                cart_id: Uuid::new_v4(),
                previous_admission: Some(CHECKOUT_OPERATION_ADMISSION_OPEN.to_string()),
                admission: CHECKOUT_OPERATION_ADMISSION_SETTLING.to_string(),
                admission_epoch: 2,
                status: "compensating".to_string(),
            },
            CheckoutOperationEvent::Reconciled {
                operation_id: Uuid::new_v4(),
                cart_id: Uuid::new_v4(),
                outcome: CHECKOUT_OPERATION_OUTCOME_COMPENSATED.to_string(),
                operator_id: Uuid::new_v4(),
            },
        ] {
            let schema = checkout_operation_event_schema(event.event_type())
                .expect("every checkout operation event type has a schema");
            assert_eq!(schema.version, event.schema_version());
        }
    }
}
