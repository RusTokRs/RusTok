use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::contract::{ContractEventPayload, EventContract, sealed};
use crate::validation::{EventValidationError, ValidateEvent, validators};
use crate::{EventSchema, FieldSchema};

const MAX_PERMISSION_KEY_LENGTH: usize = 256;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(tag = "type", content = "data")]
pub enum RbacArtifactPermissionEvent {
    AssignmentChanged {
        operation_id: Uuid,
        artifact_permission_id: Uuid,
        role_id: Uuid,
        installation_id: Uuid,
        permission_key: String,
        granted: bool,
    },
}

impl RbacArtifactPermissionEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::AssignmentChanged { .. } => "rbac.artifact_role_permission.assignment_changed",
        }
    }

    pub const fn schema_version(&self) -> u16 {
        1
    }
}

const ASSIGNMENT_CHANGED_FIELDS: &[FieldSchema] = &[
    FieldSchema {
        name: "operation_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "artifact_permission_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "role_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "installation_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "permission_key",
        data_type: "string",
        optional: false,
    },
    FieldSchema {
        name: "granted",
        data_type: "bool",
        optional: false,
    },
];

pub const RBAC_ARTIFACT_PERMISSION_EVENT_SCHEMAS: &[EventSchema] = &[EventSchema {
    event_type: "rbac.artifact_role_permission.assignment_changed",
    version: 1,
    description: "RBAC atomically changed one tenant role grant for an admitted artifact permission.",
    fields: ASSIGNMENT_CHANGED_FIELDS,
}];

impl sealed::Sealed for RbacArtifactPermissionEvent {}

impl EventContract for RbacArtifactPermissionEvent {
    fn event_type(&self) -> &'static str {
        RbacArtifactPermissionEvent::event_type(self)
    }

    fn schema_version(&self) -> u16 {
        RbacArtifactPermissionEvent::schema_version(self)
    }

    fn into_contract_payload(self) -> ContractEventPayload {
        ContractEventPayload::RbacArtifactPermission(self)
    }
}

impl ValidateEvent for RbacArtifactPermissionEvent {
    fn validate(&self) -> Result<(), EventValidationError> {
        let Self::AssignmentChanged {
            operation_id,
            artifact_permission_id,
            role_id,
            installation_id,
            permission_key,
            ..
        } = self;

        validators::validate_not_nil_uuid("operation_id", operation_id)?;
        validators::validate_not_nil_uuid("artifact_permission_id", artifact_permission_id)?;
        validators::validate_not_nil_uuid("role_id", role_id)?;
        validators::validate_not_nil_uuid("installation_id", installation_id)?;
        validators::validate_not_empty("permission_key", permission_key)?;
        validators::validate_max_length(
            "permission_key",
            permission_key,
            MAX_PERMISSION_KEY_LENGTH,
        )?;
        if permission_key.trim() != permission_key || permission_key.chars().any(char::is_control) {
            return Err(EventValidationError::InvalidCharacters("permission_key"));
        }
        Ok(())
    }
}

pub fn rbac_artifact_permission_event_schema(event_type: &str) -> Option<&'static EventSchema> {
    RBAC_ARTIFACT_PERMISSION_EVENT_SCHEMAS
        .iter()
        .find(|schema| schema.event_type == event_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event() -> RbacArtifactPermissionEvent {
        RbacArtifactPermissionEvent::AssignmentChanged {
            operation_id: Uuid::new_v4(),
            artifact_permission_id: Uuid::new_v4(),
            role_id: Uuid::new_v4(),
            installation_id: Uuid::new_v4(),
            permission_key: "sample.events.handle".to_string(),
            granted: true,
        }
    }

    #[test]
    fn validates_registered_assignment_change() {
        let event = event();
        assert!(event.validate().is_ok());
        assert_eq!(
            event.event_type(),
            "rbac.artifact_role_permission.assignment_changed"
        );
        assert_eq!(event.schema_version(), 1);
    }

    #[test]
    fn rejects_nil_identity_and_unbounded_permission_key() {
        let RbacArtifactPermissionEvent::AssignmentChanged {
            artifact_permission_id,
            role_id,
            installation_id,
            permission_key,
            granted,
            ..
        } = event();
        assert!(
            RbacArtifactPermissionEvent::AssignmentChanged {
                operation_id: Uuid::nil(),
                artifact_permission_id,
                role_id,
                installation_id,
                permission_key,
                granted,
            }
            .validate()
            .is_err()
        );

        let RbacArtifactPermissionEvent::AssignmentChanged {
            operation_id,
            role_id,
            installation_id,
            permission_key,
            granted,
            ..
        } = event();
        assert!(
            RbacArtifactPermissionEvent::AssignmentChanged {
                operation_id,
                artifact_permission_id: Uuid::nil(),
                role_id,
                installation_id,
                permission_key,
                granted,
            }
            .validate()
            .is_err()
        );

        let RbacArtifactPermissionEvent::AssignmentChanged {
            operation_id,
            artifact_permission_id,
            role_id,
            installation_id,
            granted,
            ..
        } = event();
        assert!(
            RbacArtifactPermissionEvent::AssignmentChanged {
                operation_id,
                artifact_permission_id,
                role_id,
                installation_id,
                permission_key: "x".repeat(MAX_PERMISSION_KEY_LENGTH + 1),
                granted,
            }
            .validate()
            .is_err()
        );
    }
}
