use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::contract::{ContractEventPayload, EventContract, sealed};
use crate::validation::{EventValidationError, ValidateEvent, validators};
use crate::{EventSchema, FieldSchema};

/// Typed contract event for one stored form submission.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(tag = "type", content = "data")]
pub enum FormSubmissionEvent {
    SubmissionReceived {
        submission_id: Uuid,
        tenant_id: Uuid,
        form_id: String,
        page_id: Option<Uuid>,
        locale: String,
        state: String,
    },
}

impl FormSubmissionEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::SubmissionReceived { .. } => "forms.submission.received",
        }
    }

    pub const fn schema_version(&self) -> u16 {
        1
    }
}

const SUBMISSION_RECEIVED_FIELDS: &[FieldSchema] = &[
    FieldSchema {
        name: "submission_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "tenant_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "form_id",
        data_type: "string",
        optional: false,
    },
    FieldSchema {
        name: "page_id",
        data_type: "uuid",
        optional: true,
    },
    FieldSchema {
        name: "locale",
        data_type: "string",
        optional: false,
    },
    FieldSchema {
        name: "state",
        data_type: "string",
        optional: false,
    },
];

pub const FORMS_SUBMISSION_EVENT_SCHEMAS: &[EventSchema] = &[EventSchema {
    event_type: "forms.submission.received",
    version: 1,
    description: "One form submission was stored by the Forms intake endpoint.",
    fields: SUBMISSION_RECEIVED_FIELDS,
}];

impl sealed::Sealed for FormSubmissionEvent {}

impl EventContract for FormSubmissionEvent {
    fn event_type(&self) -> &'static str {
        FormSubmissionEvent::event_type(self)
    }

    fn schema_version(&self) -> u16 {
        FormSubmissionEvent::schema_version(self)
    }

    fn into_contract_payload(self) -> ContractEventPayload {
        ContractEventPayload::FormSubmission(self)
    }
}

impl ValidateEvent for FormSubmissionEvent {
    fn validate(&self) -> Result<(), EventValidationError> {
        match self {
            Self::SubmissionReceived {
                submission_id,
                tenant_id,
                form_id,
                page_id,
                locale,
                state,
            } => {
                validators::validate_not_nil_uuid("submission_id", submission_id)?;
                validators::validate_not_nil_uuid("tenant_id", tenant_id)?;
                validators::validate_not_empty("form_id", form_id)?;
                validators::validate_max_length("form_id", form_id, 64)?;
                if let Some(page_id) = page_id {
                    validators::validate_not_nil_uuid("page_id", page_id)?;
                }
                validators::validate_not_empty("locale", locale)?;
                validators::validate_max_length("locale", locale, 16)?;
                if !matches!(state.as_str(), "new" | "read" | "handled" | "spam") {
                    return Err(EventValidationError::InvalidValue(
                        "state",
                        "must be new, read, handled or spam".to_string(),
                    ));
                }
                Ok(())
            }
        }
    }
}

pub fn forms_submission_event_schema(event_type: &str) -> Option<&'static EventSchema> {
    FORMS_SUBMISSION_EVENT_SCHEMAS
        .iter()
        .find(|schema| schema.event_type == event_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_submission_received_contract() {
        assert!(
            FormSubmissionEvent::SubmissionReceived {
                submission_id: Uuid::new_v4(),
                tenant_id: Uuid::new_v4(),
                form_id: "contact".to_string(),
                page_id: Some(Uuid::new_v4()),
                locale: "en".to_string(),
                state: "new".to_string(),
            }
            .validate()
            .is_ok()
        );
    }

    #[test]
    fn rejects_unknown_state_and_empty_form_id() {
        assert!(
            FormSubmissionEvent::SubmissionReceived {
                submission_id: Uuid::new_v4(),
                tenant_id: Uuid::new_v4(),
                form_id: String::new(),
                page_id: None,
                locale: "en".to_string(),
                state: "new".to_string(),
            }
            .validate()
            .is_err()
        );
        assert!(
            FormSubmissionEvent::SubmissionReceived {
                submission_id: Uuid::new_v4(),
                tenant_id: Uuid::new_v4(),
                form_id: "contact".to_string(),
                page_id: None,
                locale: "en".to_string(),
                state: "archived".to_string(),
            }
            .validate()
            .is_err()
        );
    }
}
