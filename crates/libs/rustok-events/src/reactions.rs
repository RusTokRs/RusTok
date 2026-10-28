use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::contract::{ContractEventPayload, EventContract, sealed};
use crate::validation::{EventValidationError, ValidateEvent, validators};
use crate::{EventSchema, FieldSchema};

pub const REACTIONS_ACTOR_STATE_CHANGED_EVENT_TYPE: &str = "reactions.actor_state.changed";
pub const REACTIONS_SUBJECT_RECONCILED_EVENT_TYPE: &str = "reactions.subject.reconciled";
pub const REACTIONS_EVENT_SCHEMA_VERSION: u16 = 1;
pub const MAX_REACTIONS_EVENT_KEYS: usize = 64;
const MAX_REACTIONS_EVENT_SEGMENT_BYTES: usize = 64;

pub const REACTIONS_EVENT_SCHEMAS: &[EventSchema] = &[
    EventSchema {
        event_type: REACTIONS_ACTOR_STATE_CHANGED_EVENT_TYPE,
        version: REACTIONS_EVENT_SCHEMA_VERSION,
        description: "A committed tenant-scoped reaction actor-state transition and its bounded aggregate deltas.",
        fields: REACTIONS_ACTOR_STATE_CHANGED_FIELDS,
    },
    EventSchema {
        event_type: REACTIONS_SUBJECT_RECONCILED_EVENT_TYPE,
        version: REACTIONS_EVENT_SCHEMA_VERSION,
        description: "A committed bounded repair of one reaction subject aggregate projection.",
        fields: REACTIONS_SUBJECT_RECONCILED_FIELDS,
    },
];

const REACTIONS_ACTOR_STATE_CHANGED_FIELDS: &[FieldSchema] = &[
    FieldSchema {
        name: "command_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "source_slug",
        data_type: "string",
        optional: false,
    },
    FieldSchema {
        name: "subject_kind",
        data_type: "string",
        optional: false,
    },
    FieldSchema {
        name: "subject_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "subject_revision",
        data_type: "int64",
        optional: false,
    },
    FieldSchema {
        name: "actor_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "requested_reaction",
        data_type: "string",
        optional: false,
    },
    FieldSchema {
        name: "action",
        data_type: "string",
        optional: false,
    },
    FieldSchema {
        name: "state_revision",
        data_type: "int64",
        optional: false,
    },
    FieldSchema {
        name: "selected_keys",
        data_type: "array",
        optional: false,
    },
    FieldSchema {
        name: "added_keys",
        data_type: "array",
        optional: false,
    },
    FieldSchema {
        name: "removed_keys",
        data_type: "array",
        optional: false,
    },
];

const REACTIONS_SUBJECT_RECONCILED_FIELDS: &[FieldSchema] = &[
    FieldSchema {
        name: "repair_command_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "source_slug",
        data_type: "string",
        optional: false,
    },
    FieldSchema {
        name: "subject_kind",
        data_type: "string",
        optional: false,
    },
    FieldSchema {
        name: "subject_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "subject_revision",
        data_type: "int64",
        optional: false,
    },
    FieldSchema {
        name: "catalog_revision",
        data_type: "int64",
        optional: false,
    },
    FieldSchema {
        name: "actor_states_scanned",
        data_type: "int64",
        optional: false,
    },
    FieldSchema {
        name: "aggregate_rows_before",
        data_type: "int64",
        optional: false,
    },
    FieldSchema {
        name: "aggregate_rows_after",
        data_type: "int64",
        optional: false,
    },
    FieldSchema {
        name: "changed_key_count",
        data_type: "int64",
        optional: false,
    },
    FieldSchema {
        name: "changed_keys",
        data_type: "array",
        optional: false,
    },
    FieldSchema {
        name: "changed_keys_truncated",
        data_type: "bool",
        optional: false,
    },
];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(tag = "type", content = "data")]
pub enum ReactionsEvent {
    ActorStateChanged {
        command_id: Uuid,
        source_slug: String,
        subject_kind: String,
        subject_id: Uuid,
        subject_revision: i64,
        actor_id: Uuid,
        requested_reaction: String,
        action: String,
        state_revision: i64,
        selected_keys: Vec<String>,
        added_keys: Vec<String>,
        removed_keys: Vec<String>,
    },
    SubjectReconciled {
        repair_command_id: Uuid,
        source_slug: String,
        subject_kind: String,
        subject_id: Uuid,
        subject_revision: i64,
        catalog_revision: i64,
        actor_states_scanned: i64,
        aggregate_rows_before: i64,
        aggregate_rows_after: i64,
        changed_key_count: i64,
        changed_keys: Vec<String>,
        changed_keys_truncated: bool,
    },
}

impl ReactionsEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::ActorStateChanged { .. } => REACTIONS_ACTOR_STATE_CHANGED_EVENT_TYPE,
            Self::SubjectReconciled { .. } => REACTIONS_SUBJECT_RECONCILED_EVENT_TYPE,
        }
    }

    pub const fn schema_version(&self) -> u16 {
        REACTIONS_EVENT_SCHEMA_VERSION
    }
}

impl sealed::Sealed for ReactionsEvent {}

impl EventContract for ReactionsEvent {
    fn event_type(&self) -> &'static str {
        ReactionsEvent::event_type(self)
    }

    fn schema_version(&self) -> u16 {
        ReactionsEvent::schema_version(self)
    }

    fn into_contract_payload(self) -> ContractEventPayload {
        ContractEventPayload::Reactions(self)
    }
}

impl ValidateEvent for ReactionsEvent {
    fn validate(&self) -> Result<(), EventValidationError> {
        match self {
            Self::ActorStateChanged {
                command_id,
                source_slug,
                subject_kind,
                subject_id,
                subject_revision,
                actor_id,
                requested_reaction,
                action,
                state_revision,
                selected_keys,
                added_keys,
                removed_keys,
            } => {
                validators::validate_not_nil_uuid("command_id", command_id)?;
                validators::validate_not_nil_uuid("subject_id", subject_id)?;
                validators::validate_not_nil_uuid("actor_id", actor_id)?;
                validate_segment("source_slug", source_slug)?;
                validate_segment("subject_kind", subject_kind)?;
                validate_segment("requested_reaction", requested_reaction)?;
                if !matches!(action.as_str(), "add" | "remove") {
                    return Err(EventValidationError::InvalidValue(
                        "action",
                        "must be add or remove".to_string(),
                    ));
                }
                validators::validate_range("subject_revision", *subject_revision, 1, i64::MAX)?;
                validators::validate_range("state_revision", *state_revision, 1, i64::MAX)?;
                validate_keys("selected_keys", selected_keys)?;
                validate_keys("added_keys", added_keys)?;
                validate_keys("removed_keys", removed_keys)?;

                let selected = selected_keys.iter().collect::<BTreeSet<_>>();
                let added = added_keys.iter().collect::<BTreeSet<_>>();
                let removed = removed_keys.iter().collect::<BTreeSet<_>>();
                if !added.is_subset(&selected) || !added.is_disjoint(&removed) {
                    return Err(EventValidationError::InvalidValue(
                        "added_keys",
                        "added keys must be selected and disjoint from removed keys".to_string(),
                    ));
                }
                if removed.iter().any(|key| selected.contains(key)) {
                    return Err(EventValidationError::InvalidValue(
                        "removed_keys",
                        "removed keys must not remain selected".to_string(),
                    ));
                }
                if added.is_empty() && removed.is_empty() {
                    return Err(EventValidationError::InvalidValue(
                        "added_keys",
                        "a changed actor state must contain an added or removed key".to_string(),
                    ));
                }
                Ok(())
            }
            Self::SubjectReconciled {
                repair_command_id,
                source_slug,
                subject_kind,
                subject_id,
                subject_revision,
                catalog_revision,
                actor_states_scanned,
                aggregate_rows_before,
                aggregate_rows_after,
                changed_key_count,
                changed_keys,
                changed_keys_truncated,
            } => {
                validators::validate_not_nil_uuid("repair_command_id", repair_command_id)?;
                validators::validate_not_nil_uuid("subject_id", subject_id)?;
                validate_segment("source_slug", source_slug)?;
                validate_segment("subject_kind", subject_kind)?;
                validators::validate_range("subject_revision", *subject_revision, 1, i64::MAX)?;
                validators::validate_range("catalog_revision", *catalog_revision, 1, i64::MAX)?;
                validators::validate_range(
                    "actor_states_scanned",
                    *actor_states_scanned,
                    0,
                    i64::MAX,
                )?;
                validators::validate_range(
                    "aggregate_rows_before",
                    *aggregate_rows_before,
                    0,
                    i64::MAX,
                )?;
                validators::validate_range(
                    "aggregate_rows_after",
                    *aggregate_rows_after,
                    0,
                    i64::MAX,
                )?;
                validators::validate_range("changed_key_count", *changed_key_count, 1, i64::MAX)?;
                validate_keys("changed_keys", changed_keys)?;
                if changed_keys.is_empty() {
                    return Err(EventValidationError::InvalidValue(
                        "changed_keys",
                        "a reconciliation event must contain a bounded changed-key sample"
                            .to_string(),
                    ));
                }
                if !*changed_keys_truncated
                    && i64::try_from(changed_keys.len()).ok() != Some(*changed_key_count)
                {
                    return Err(EventValidationError::InvalidValue(
                        "changed_key_count",
                        "untruncated changed-key count must equal the sample length".to_string(),
                    ));
                }
                if *changed_keys_truncated
                    && i64::try_from(changed_keys.len())
                        .is_ok_and(|length| length >= *changed_key_count)
                {
                    return Err(EventValidationError::InvalidValue(
                        "changed_keys_truncated",
                        "a truncated sample must be smaller than the changed-key count".to_string(),
                    ));
                }
                Ok(())
            }
        }
    }
}

fn validate_segment(field: &'static str, value: &str) -> Result<(), EventValidationError> {
    validators::validate_not_empty(field, value)?;
    validators::validate_max_length(field, value, MAX_REACTIONS_EVENT_SEGMENT_BYTES)?;
    if value.trim() != value
        || value.starts_with('-')
        || value.starts_with('_')
        || value.ends_with('-')
        || value.ends_with('_')
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_'
        })
    {
        return Err(EventValidationError::InvalidCharacters(field));
    }
    Ok(())
}

fn validate_keys(field: &'static str, keys: &[String]) -> Result<(), EventValidationError> {
    if keys.len() > MAX_REACTIONS_EVENT_KEYS {
        return Err(EventValidationError::InvalidValue(
            field,
            format!("must contain at most {MAX_REACTIONS_EVENT_KEYS} keys"),
        ));
    }
    if keys.iter().collect::<BTreeSet<_>>().len() != keys.len() {
        return Err(EventValidationError::InvalidValue(
            field,
            "must not contain duplicate keys".to_string(),
        ));
    }
    for key in keys {
        validate_segment(field, key)?;
    }
    Ok(())
}

pub fn reactions_event_schema(event_type: &str) -> Option<&'static EventSchema> {
    REACTIONS_EVENT_SCHEMAS
        .iter()
        .find(|schema| schema.event_type == event_type)
}
