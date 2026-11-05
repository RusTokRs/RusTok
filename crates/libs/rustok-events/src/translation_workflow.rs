use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::contract::{ContractEventPayload, EventContract, sealed};
use crate::validation::{EventValidationError, ValidateEvent, validators};
use crate::{EventSchema, FieldSchema};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(tag = "type", content = "data")]
pub enum TranslationWorkflowEvent {
    JobCreated {
        job_id: Uuid,
        source_locale: String,
        target_locale: String,
        revision: i64,
    },
    JobCancelled {
        job_id: Uuid,
        revision: i64,
        cancelled_item_count: u64,
    },
    JobCompleted {
        job_id: Uuid,
        revision: i64,
        total_item_count: u64,
    },
    ItemAssigned {
        job_id: Uuid,
        item_id: Uuid,
        assignee_actor_kind: String,
        assignee_actor_id: String,
        item_revision: i64,
    },
    ItemUnassigned {
        job_id: Uuid,
        item_id: Uuid,
        previous_actor_kind: String,
        previous_actor_id: String,
        item_revision: i64,
    },
    ItemRetryRequested {
        job_id: Uuid,
        item_id: Uuid,
        prior_status: String,
        item_revision: i64,
    },
    NoteCreated {
        note_id: Uuid,
        job_id: Uuid,
        item_id: Option<Uuid>,
        revision: i64,
    },
    NoteResolved {
        note_id: Uuid,
        job_id: Uuid,
        item_id: Option<Uuid>,
        revision: i64,
    },
    ProposalSubmitted {
        item_id: Uuid,
        proposal_id: Uuid,
        item_revision: i64,
    },
    ProposalApproved {
        item_id: Uuid,
        proposal_id: Uuid,
        item_revision: i64,
    },
    ApplyRequested {
        operation_id: Uuid,
        item_id: Uuid,
        proposal_id: Uuid,
        item_revision: i64,
    },
    ApplyCompleted {
        operation_id: Uuid,
        item_id: Uuid,
        proposal_id: Uuid,
        item_revision: i64,
    },
    ApplyFailed {
        operation_id: Uuid,
        item_id: Uuid,
        proposal_id: Uuid,
        status: String,
        error_code: String,
        retryable: bool,
        attempt_count: i64,
    },
    ApplyRecoveryRequested {
        operation_id: Uuid,
        item_id: Uuid,
        recovery_id: Uuid,
        observed_attempt_count: i64,
    },
}

impl TranslationWorkflowEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::JobCreated { .. } => "translation.job.created",
            Self::JobCancelled { .. } => "translation.job.cancelled",
            Self::JobCompleted { .. } => "translation.job.completed",
            Self::ItemAssigned { .. } => "translation.item.assigned",
            Self::ItemUnassigned { .. } => "translation.item.unassigned",
            Self::ItemRetryRequested { .. } => "translation.item.retry_requested",
            Self::NoteCreated { .. } => "translation.note.created",
            Self::NoteResolved { .. } => "translation.note.resolved",
            Self::ProposalSubmitted { .. } => "translation.proposal.submitted",
            Self::ProposalApproved { .. } => "translation.proposal.approved",
            Self::ApplyRequested { .. } => "translation.apply.requested",
            Self::ApplyCompleted { .. } => "translation.apply.completed",
            Self::ApplyFailed { .. } => "translation.apply.failed",
            Self::ApplyRecoveryRequested { .. } => "translation.apply.recovery_requested",
        }
    }

    pub fn schema_version(&self) -> u16 {
        1
    }
}

const JOB_CREATED_FIELDS: &[FieldSchema] = &[
    field("job_id", "uuid"),
    field("source_locale", "string"),
    field("target_locale", "string"),
    field("revision", "int64"),
];
const JOB_CANCELLED_FIELDS: &[FieldSchema] = &[
    field("job_id", "uuid"),
    field("revision", "int64"),
    field("cancelled_item_count", "uint64"),
];
const JOB_COMPLETED_FIELDS: &[FieldSchema] = &[
    field("job_id", "uuid"),
    field("revision", "int64"),
    field("total_item_count", "uint64"),
];
const ITEM_ASSIGNED_FIELDS: &[FieldSchema] = &[
    field("job_id", "uuid"),
    field("item_id", "uuid"),
    field("assignee_actor_kind", "string"),
    field("assignee_actor_id", "string"),
    field("item_revision", "int64"),
];
const ITEM_UNASSIGNED_FIELDS: &[FieldSchema] = &[
    field("job_id", "uuid"),
    field("item_id", "uuid"),
    field("previous_actor_kind", "string"),
    field("previous_actor_id", "string"),
    field("item_revision", "int64"),
];
const ITEM_RETRY_REQUESTED_FIELDS: &[FieldSchema] = &[
    field("job_id", "uuid"),
    field("item_id", "uuid"),
    field("prior_status", "string"),
    field("item_revision", "int64"),
];
const NOTE_FIELDS: &[FieldSchema] = &[
    field("note_id", "uuid"),
    field("job_id", "uuid"),
    optional_field("item_id", "uuid"),
    field("revision", "int64"),
];
const PROPOSAL_FIELDS: &[FieldSchema] = &[
    field("item_id", "uuid"),
    field("proposal_id", "uuid"),
    field("item_revision", "int64"),
];
const APPLY_FIELDS: &[FieldSchema] = &[
    field("operation_id", "uuid"),
    field("item_id", "uuid"),
    field("proposal_id", "uuid"),
    field("item_revision", "int64"),
];
const APPLY_FAILED_FIELDS: &[FieldSchema] = &[
    field("operation_id", "uuid"),
    field("item_id", "uuid"),
    field("proposal_id", "uuid"),
    field("status", "string"),
    field("error_code", "string"),
    field("retryable", "bool"),
    field("attempt_count", "int64"),
];
const APPLY_RECOVERY_FIELDS: &[FieldSchema] = &[
    field("operation_id", "uuid"),
    field("item_id", "uuid"),
    field("recovery_id", "uuid"),
    field("observed_attempt_count", "int64"),
];

const fn field(name: &'static str, data_type: &'static str) -> FieldSchema {
    FieldSchema {
        name,
        data_type,
        optional: false,
    }
}

const fn optional_field(name: &'static str, data_type: &'static str) -> FieldSchema {
    FieldSchema {
        name,
        data_type,
        optional: true,
    }
}

pub const TRANSLATION_WORKFLOW_EVENT_SCHEMAS: &[EventSchema] = &[
    schema(
        "translation.job.created",
        "A translation job was created.",
        JOB_CREATED_FIELDS,
    ),
    schema(
        "translation.job.cancelled",
        "A translation job and its remaining cancellable items were cancelled.",
        JOB_CANCELLED_FIELDS,
    ),
    schema(
        "translation.job.completed",
        "A translation job reached successful terminal completion.",
        JOB_COMPLETED_FIELDS,
    ),
    schema(
        "translation.item.assigned",
        "A translation job item was assigned.",
        ITEM_ASSIGNED_FIELDS,
    ),
    schema(
        "translation.item.unassigned",
        "A translation job item assignment was cleared.",
        ITEM_UNASSIGNED_FIELDS,
    ),
    schema(
        "translation.item.retry_requested",
        "A blocked translation job item was returned to its approved state for an explicit retry.",
        ITEM_RETRY_REQUESTED_FIELDS,
    ),
    schema(
        "translation.note.created",
        "A private translation workflow note was created without its body content.",
        NOTE_FIELDS,
    ),
    schema(
        "translation.note.resolved",
        "A private translation workflow note was resolved without its body content.",
        NOTE_FIELDS,
    ),
    schema(
        "translation.proposal.submitted",
        "A translation proposal entered review.",
        PROPOSAL_FIELDS,
    ),
    schema(
        "translation.proposal.approved",
        "A translation proposal was approved.",
        PROPOSAL_FIELDS,
    ),
    schema(
        "translation.apply.requested",
        "An approved translation owner-apply operation was requested.",
        APPLY_FIELDS,
    ),
    schema(
        "translation.apply.completed",
        "A translation owner-apply operation completed.",
        APPLY_FIELDS,
    ),
    schema(
        "translation.apply.failed",
        "A translation owner-apply attempt returned a retryable or terminal error.",
        APPLY_FAILED_FIELDS,
    ),
    schema(
        "translation.apply.recovery_requested",
        "A privileged translation owner-apply recovery was requested.",
        APPLY_RECOVERY_FIELDS,
    ),
];

const fn schema(
    event_type: &'static str,
    description: &'static str,
    fields: &'static [FieldSchema],
) -> EventSchema {
    EventSchema {
        event_type,
        version: 1,
        description,
        fields,
    }
}

impl sealed::Sealed for TranslationWorkflowEvent {}

impl EventContract for TranslationWorkflowEvent {
    fn event_type(&self) -> &'static str {
        TranslationWorkflowEvent::event_type(self)
    }

    fn schema_version(&self) -> u16 {
        TranslationWorkflowEvent::schema_version(self)
    }

    fn into_contract_payload(self) -> ContractEventPayload {
        ContractEventPayload::TranslationWorkflow(self)
    }
}

impl ValidateEvent for TranslationWorkflowEvent {
    fn validate(&self) -> Result<(), EventValidationError> {
        match self {
            Self::JobCreated {
                job_id,
                source_locale,
                target_locale,
                revision,
            } => {
                validate_uuid("job_id", job_id)?;
                validate_locale("source_locale", source_locale)?;
                validate_locale("target_locale", target_locale)?;
                if source_locale == target_locale {
                    return Err(EventValidationError::InvalidValue(
                        "target_locale",
                        "must differ from source_locale".to_string(),
                    ));
                }
                validate_revision("revision", *revision)
            }
            Self::JobCancelled {
                job_id, revision, ..
            } => {
                validate_uuid("job_id", job_id)?;
                validate_revision("revision", *revision)
            }
            Self::JobCompleted {
                job_id,
                revision,
                total_item_count,
            } => {
                validate_uuid("job_id", job_id)?;
                validate_revision("revision", *revision)?;
                if *total_item_count == 0 {
                    return Err(EventValidationError::InvalidValue(
                        "total_item_count",
                        "must be greater than zero".to_string(),
                    ));
                }
                Ok(())
            }
            Self::ItemAssigned {
                job_id,
                item_id,
                assignee_actor_kind,
                assignee_actor_id,
                item_revision,
            } => {
                validate_uuid("job_id", job_id)?;
                validate_uuid("item_id", item_id)?;
                validate_actor(assignee_actor_kind, assignee_actor_id)?;
                validate_revision("item_revision", *item_revision)
            }
            Self::ItemUnassigned {
                job_id,
                item_id,
                previous_actor_kind,
                previous_actor_id,
                item_revision,
            } => {
                validate_uuid("job_id", job_id)?;
                validate_uuid("item_id", item_id)?;
                validate_actor(previous_actor_kind, previous_actor_id)?;
                validate_revision("item_revision", *item_revision)
            }
            Self::ItemRetryRequested {
                job_id,
                item_id,
                prior_status,
                item_revision,
            } => {
                validate_uuid("job_id", job_id)?;
                validate_uuid("item_id", item_id)?;
                if prior_status != "blocked" {
                    return Err(EventValidationError::InvalidValue(
                        "prior_status",
                        "must be `blocked`".to_string(),
                    ));
                }
                validate_revision("item_revision", *item_revision)
            }
            Self::NoteCreated {
                note_id,
                job_id,
                item_id,
                revision,
            }
            | Self::NoteResolved {
                note_id,
                job_id,
                item_id,
                revision,
            } => {
                validate_uuid("note_id", note_id)?;
                validate_uuid("job_id", job_id)?;
                if let Some(item_id) = item_id {
                    validate_uuid("item_id", item_id)?;
                }
                validate_revision("revision", *revision)
            }
            Self::ProposalSubmitted {
                item_id,
                proposal_id,
                item_revision,
            }
            | Self::ProposalApproved {
                item_id,
                proposal_id,
                item_revision,
            } => {
                validate_uuid("item_id", item_id)?;
                validate_uuid("proposal_id", proposal_id)?;
                validate_revision("item_revision", *item_revision)
            }
            Self::ApplyRequested {
                operation_id,
                item_id,
                proposal_id,
                item_revision,
            }
            | Self::ApplyCompleted {
                operation_id,
                item_id,
                proposal_id,
                item_revision,
            } => {
                validate_uuid("operation_id", operation_id)?;
                validate_uuid("item_id", item_id)?;
                validate_uuid("proposal_id", proposal_id)?;
                validate_revision("item_revision", *item_revision)
            }
            Self::ApplyFailed {
                operation_id,
                item_id,
                proposal_id,
                status,
                error_code,
                attempt_count,
                ..
            } => {
                validate_uuid("operation_id", operation_id)?;
                validate_uuid("item_id", item_id)?;
                validate_uuid("proposal_id", proposal_id)?;
                validate_bounded("status", status, 16)?;
                validate_bounded("error_code", error_code, 191)?;
                validators::validate_range("attempt_count", *attempt_count, 1, i64::MAX)?;
                Ok(())
            }
            Self::ApplyRecoveryRequested {
                operation_id,
                item_id,
                recovery_id,
                observed_attempt_count,
            } => {
                validate_uuid("operation_id", operation_id)?;
                validate_uuid("item_id", item_id)?;
                validate_uuid("recovery_id", recovery_id)?;
                validators::validate_range(
                    "observed_attempt_count",
                    *observed_attempt_count,
                    0,
                    i64::MAX,
                )?;
                Ok(())
            }
        }
    }
}

fn validate_uuid(field_name: &'static str, value: &Uuid) -> Result<(), EventValidationError> {
    validators::validate_not_nil_uuid(field_name, value)
}

fn validate_locale(field_name: &'static str, value: &str) -> Result<(), EventValidationError> {
    validate_bounded(field_name, value, 32)
}

fn validate_actor(kind: &str, id: &str) -> Result<(), EventValidationError> {
    if !matches!(kind, "user" | "service" | "system") {
        return Err(EventValidationError::InvalidValue(
            "actor_kind",
            "must be user, service, or system".to_string(),
        ));
    }
    validate_bounded("actor_id", id, 191)
}

fn validate_revision(field_name: &'static str, value: i64) -> Result<(), EventValidationError> {
    validators::validate_range(field_name, value, 0, i64::MAX)?;
    Ok(())
}

fn validate_bounded(
    field_name: &'static str,
    value: &str,
    max: usize,
) -> Result<(), EventValidationError> {
    validators::validate_not_empty(field_name, value)?;
    validators::validate_max_length(field_name, value, max)?;
    Ok(())
}

pub fn translation_workflow_event_schema(event_type: &str) -> Option<&'static EventSchema> {
    TRANSLATION_WORKFLOW_EVENT_SCHEMAS
        .iter()
        .find(|schema| schema.event_type == event_type)
}
