use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::contract::{ContractEventPayload, EventContract, sealed};
use crate::validation::{EventValidationError, ValidateEvent, validators};
use crate::{EventSchema, FieldSchema};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(tag = "type", content = "data")]
pub enum ForumSearchProjectionEvent {
    InvalidationIssued {
        owner_revision: i64,
        target_type: String,
        target_id: Option<Uuid>,
    },
}

impl ForumSearchProjectionEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::InvalidationIssued { .. } => "forum.search_projection.invalidation_issued",
        }
    }

    pub const fn schema_version(&self) -> u16 {
        1
    }
}

const INVALIDATION_ISSUED_FIELDS: &[FieldSchema] = &[
    FieldSchema {
        name: "owner_revision",
        data_type: "int64",
        optional: false,
    },
    FieldSchema {
        name: "target_type",
        data_type: "string",
        optional: false,
    },
    FieldSchema {
        name: "target_id",
        data_type: "uuid",
        optional: true,
    },
];

pub const FORUM_SEARCH_PROJECTION_EVENT_SCHEMAS: &[EventSchema] = &[EventSchema {
    event_type: "forum.search_projection.invalidation_issued",
    version: 1,
    description: "Forum issued one monotonic owner revision that invalidates a Search projection scope.",
    fields: INVALIDATION_ISSUED_FIELDS,
}];

impl sealed::Sealed for ForumSearchProjectionEvent {}

impl EventContract for ForumSearchProjectionEvent {
    fn event_type(&self) -> &'static str {
        ForumSearchProjectionEvent::event_type(self)
    }

    fn schema_version(&self) -> u16 {
        ForumSearchProjectionEvent::schema_version(self)
    }

    fn into_contract_payload(self) -> ContractEventPayload {
        ContractEventPayload::ForumSearchProjection(self)
    }
}

impl ValidateEvent for ForumSearchProjectionEvent {
    fn validate(&self) -> Result<(), EventValidationError> {
        let Self::InvalidationIssued {
            owner_revision,
            target_type,
            target_id,
        } = self;

        validators::validate_range("owner_revision", *owner_revision, 1, i64::MAX)?;
        match (target_type.as_str(), target_id) {
            ("forum", None) => Ok(()),
            ("forum_category" | "forum_topic", Some(target_id)) => {
                validators::validate_not_nil_uuid("target_id", target_id)
            }
            ("forum", Some(_)) => Err(EventValidationError::InvalidValue(
                "target_id",
                "must be absent for a full Forum projection invalidation".to_string(),
            )),
            ("forum_category" | "forum_topic", None) => {
                Err(EventValidationError::MissingField("target_id"))
            }
            _ => Err(EventValidationError::InvalidValue(
                "target_type",
                "must be forum, forum_category, or forum_topic".to_string(),
            )),
        }
    }
}

pub fn forum_search_projection_event_schema(event_type: &str) -> Option<&'static EventSchema> {
    FORUM_SEARCH_PROJECTION_EVENT_SCHEMAS
        .iter()
        .find(|schema| schema.event_type == event_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_each_supported_projection_scope() {
        assert!(
            ForumSearchProjectionEvent::InvalidationIssued {
                owner_revision: 1,
                target_type: "forum".to_string(),
                target_id: None,
            }
            .validate()
            .is_ok()
        );

        for target_type in ["forum_category", "forum_topic"] {
            assert!(
                ForumSearchProjectionEvent::InvalidationIssued {
                    owner_revision: 2,
                    target_type: target_type.to_string(),
                    target_id: Some(Uuid::new_v4()),
                }
                .validate()
                .is_ok()
            );
        }
    }

    #[test]
    fn rejects_invalid_revision_and_scope_identity() {
        for event in [
            ForumSearchProjectionEvent::InvalidationIssued {
                owner_revision: 0,
                target_type: "forum".to_string(),
                target_id: None,
            },
            ForumSearchProjectionEvent::InvalidationIssued {
                owner_revision: 1,
                target_type: "forum".to_string(),
                target_id: Some(Uuid::new_v4()),
            },
            ForumSearchProjectionEvent::InvalidationIssued {
                owner_revision: 1,
                target_type: "forum_topic".to_string(),
                target_id: None,
            },
            ForumSearchProjectionEvent::InvalidationIssued {
                owner_revision: 1,
                target_type: "forum_category".to_string(),
                target_id: Some(Uuid::nil()),
            },
            ForumSearchProjectionEvent::InvalidationIssued {
                owner_revision: 1,
                target_type: "post".to_string(),
                target_id: Some(Uuid::new_v4()),
            },
        ] {
            assert!(event.validate().is_err());
        }
    }
}
