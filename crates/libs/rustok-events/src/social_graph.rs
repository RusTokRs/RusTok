use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::contract::{ContractEventPayload, EventContract, sealed};
use crate::validation::{EventValidationError, ValidateEvent, validators};
use crate::{EventSchema, FieldSchema};

pub const SOCIAL_GRAPH_RELATION_EVENT_SCHEMAS: &[EventSchema] = &[EventSchema {
    event_type: "social_graph.relation.state_changed",
    version: 1,
    description: "A tenant-scoped social relation state fact for one persisted revision.",
    fields: SOCIAL_GRAPH_RELATION_EVENT_FIELDS,
}];

const SOCIAL_GRAPH_RELATION_EVENT_FIELDS: &[FieldSchema] = &[
    FieldSchema {
        name: "relation_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "source_user_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "target_user_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "relation_kind",
        data_type: "string",
        optional: false,
    },
    FieldSchema {
        name: "active",
        data_type: "bool",
        optional: false,
    },
    FieldSchema {
        name: "revision",
        data_type: "int64",
        optional: false,
    },
];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(tag = "type", content = "data")]
pub enum SocialGraphRelationEvent {
    RelationStateChanged {
        relation_id: Uuid,
        source_user_id: Uuid,
        target_user_id: Uuid,
        relation_kind: String,
        active: bool,
        revision: i64,
    },
}

impl SocialGraphRelationEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::RelationStateChanged { .. } => "social_graph.relation.state_changed",
        }
    }

    pub fn schema_version(&self) -> u16 {
        1
    }
}

impl sealed::Sealed for SocialGraphRelationEvent {}

impl EventContract for SocialGraphRelationEvent {
    fn event_type(&self) -> &'static str {
        SocialGraphRelationEvent::event_type(self)
    }

    fn schema_version(&self) -> u16 {
        SocialGraphRelationEvent::schema_version(self)
    }

    fn into_contract_payload(self) -> ContractEventPayload {
        ContractEventPayload::SocialGraphRelation(self)
    }
}

impl ValidateEvent for SocialGraphRelationEvent {
    fn validate(&self) -> Result<(), EventValidationError> {
        match self {
            Self::RelationStateChanged {
                relation_id,
                source_user_id,
                target_user_id,
                relation_kind,
                revision,
                ..
            } => {
                validators::validate_not_nil_uuid("relation_id", relation_id)?;
                validators::validate_not_nil_uuid("source_user_id", source_user_id)?;
                validators::validate_not_nil_uuid("target_user_id", target_user_id)?;
                if source_user_id == target_user_id {
                    return Err(EventValidationError::InvalidValue(
                        "target_user_id",
                        "social relation source and target must differ".to_string(),
                    ));
                }
                if !matches!(relation_kind.as_str(), "block" | "mute" | "follow") {
                    return Err(EventValidationError::InvalidValue(
                        "relation_kind",
                        "must be block, mute, or follow".to_string(),
                    ));
                }
                validators::validate_range("revision", *revision, 1, i64::MAX)?;
                Ok(())
            }
        }
    }
}

pub fn social_graph_relation_event_schema(event_type: &str) -> Option<&'static EventSchema> {
    SOCIAL_GRAPH_RELATION_EVENT_SCHEMAS
        .iter()
        .find(|schema| schema.event_type == event_type)
}
