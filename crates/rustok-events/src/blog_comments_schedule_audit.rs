use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::contract::{ContractEventPayload, EventContract, sealed};
use crate::validation::{EventValidationError, ValidateEvent, validators};
use crate::{EventSchema, FieldSchema};

pub const BLOG_COMMENTS_SCHEDULE_AUDIT_EVENT_TYPE: &str =
    "blog.comments_delegation_schedule.replacement_succeeded";
pub const BLOG_COMMENTS_SCHEDULE_AUDIT_SCHEMA_VERSION: u16 = 1;
pub const BLOG_COMMENTS_SCHEDULE_AUDIT_STATE_KEY: &str = "comments_tcp_delegation_schedule";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(tag = "type", content = "data")]
pub enum BlogCommentsDelegationScheduleAuditEvent {
    ReplacementSucceeded {
        audit_schema_version: u16,
        request_id: Uuid,
        state_key: String,
        occurred_at_unix_ms: i64,
        principal_kind: String,
        operation: String,
        source: String,
        previous_generation: i64,
        candidate_generation: i64,
    },
}

impl BlogCommentsDelegationScheduleAuditEvent {
    pub const fn event_type(&self) -> &'static str {
        match self {
            Self::ReplacementSucceeded { .. } => BLOG_COMMENTS_SCHEDULE_AUDIT_EVENT_TYPE,
        }
    }

    pub const fn schema_version(&self) -> u16 {
        BLOG_COMMENTS_SCHEDULE_AUDIT_SCHEMA_VERSION
    }

    pub fn request_id(&self) -> Uuid {
        match self {
            Self::ReplacementSucceeded { request_id, .. } => *request_id,
        }
    }
}

const REPLACEMENT_SUCCEEDED_FIELDS: &[FieldSchema] = &[
    FieldSchema {
        name: "audit_schema_version",
        data_type: "int32",
        optional: false,
    },
    FieldSchema {
        name: "request_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "state_key",
        data_type: "string",
        optional: false,
    },
    FieldSchema {
        name: "occurred_at_unix_ms",
        data_type: "int64",
        optional: false,
    },
    FieldSchema {
        name: "principal_kind",
        data_type: "string",
        optional: false,
    },
    FieldSchema {
        name: "operation",
        data_type: "string",
        optional: false,
    },
    FieldSchema {
        name: "source",
        data_type: "string",
        optional: false,
    },
    FieldSchema {
        name: "previous_generation",
        data_type: "int64",
        optional: false,
    },
    FieldSchema {
        name: "candidate_generation",
        data_type: "int64",
        optional: false,
    },
];

pub const BLOG_COMMENTS_SCHEDULE_AUDIT_EVENT_SCHEMAS: &[EventSchema] = &[EventSchema {
    event_type: BLOG_COMMENTS_SCHEDULE_AUDIT_EVENT_TYPE,
    version: BLOG_COMMENTS_SCHEDULE_AUDIT_SCHEMA_VERSION,
    description: "Blog published one successful authorized Comments delegation schedule replacement for canonical outbox delivery.",
    fields: REPLACEMENT_SUCCEEDED_FIELDS,
}];

impl sealed::Sealed for BlogCommentsDelegationScheduleAuditEvent {}

impl EventContract for BlogCommentsDelegationScheduleAuditEvent {
    fn event_type(&self) -> &'static str {
        BlogCommentsDelegationScheduleAuditEvent::event_type(self)
    }

    fn schema_version(&self) -> u16 {
        BlogCommentsDelegationScheduleAuditEvent::schema_version(self)
    }

    fn into_contract_payload(self) -> ContractEventPayload {
        ContractEventPayload::BlogCommentsDelegationScheduleAudit(self)
    }
}

impl ValidateEvent for BlogCommentsDelegationScheduleAuditEvent {
    fn validate(&self) -> Result<(), EventValidationError> {
        let Self::ReplacementSucceeded {
            audit_schema_version,
            request_id,
            state_key,
            occurred_at_unix_ms,
            principal_kind,
            operation,
            source,
            previous_generation,
            candidate_generation,
        } = self;

        if *audit_schema_version != BLOG_COMMENTS_SCHEDULE_AUDIT_SCHEMA_VERSION {
            return Err(EventValidationError::InvalidValue(
                "audit_schema_version",
                format!("must equal {BLOG_COMMENTS_SCHEDULE_AUDIT_SCHEMA_VERSION}"),
            ));
        }
        validators::validate_not_nil_uuid("request_id", request_id)?;
        if state_key != BLOG_COMMENTS_SCHEDULE_AUDIT_STATE_KEY {
            return Err(EventValidationError::InvalidValue(
                "state_key",
                format!("must equal {BLOG_COMMENTS_SCHEDULE_AUDIT_STATE_KEY}"),
            ));
        }
        validators::validate_range("occurred_at_unix_ms", *occurred_at_unix_ms, 1, i64::MAX)?;
        if !matches!(principal_kind.as_str(), "direct_user" | "service") {
            return Err(EventValidationError::InvalidValue(
                "principal_kind",
                "must be direct_user or service".to_string(),
            ));
        }
        if !matches!(operation.as_str(), "reload_file" | "replace_host_schedule") {
            return Err(EventValidationError::InvalidValue(
                "operation",
                "must be reload_file or replace_host_schedule".to_string(),
            ));
        }
        if !matches!(source.as_str(), "host_provided" | "file") {
            return Err(EventValidationError::InvalidValue(
                "source",
                "must be host_provided or file".to_string(),
            ));
        }
        validators::validate_range("previous_generation", *previous_generation, 1, i64::MAX)?;
        validators::validate_range("candidate_generation", *candidate_generation, 1, i64::MAX)?;
        if candidate_generation <= previous_generation {
            return Err(EventValidationError::InvalidValue(
                "candidate_generation",
                "must be greater than previous_generation".to_string(),
            ));
        }
        Ok(())
    }
}

pub fn blog_comments_schedule_audit_event_schema(event_type: &str) -> Option<&'static EventSchema> {
    BLOG_COMMENTS_SCHEDULE_AUDIT_EVENT_SCHEMAS
        .iter()
        .find(|schema| schema.event_type == event_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_event() -> BlogCommentsDelegationScheduleAuditEvent {
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
    fn accepts_the_bounded_success_contract() {
        assert!(valid_event().validate().is_ok());
    }

    #[test]
    fn rejects_nil_identity_and_invalid_generation_order() {
        let mut nil_request = valid_event();
        let BlogCommentsDelegationScheduleAuditEvent::ReplacementSucceeded { request_id, .. } =
            &mut nil_request;
        *request_id = Uuid::nil();
        assert!(nil_request.validate().is_err());

        let mut invalid_generation = valid_event();
        let BlogCommentsDelegationScheduleAuditEvent::ReplacementSucceeded {
            previous_generation,
            candidate_generation,
            ..
        } = &mut invalid_generation;
        *previous_generation = 2;
        *candidate_generation = 2;
        assert!(invalid_generation.validate().is_err());
    }
}
