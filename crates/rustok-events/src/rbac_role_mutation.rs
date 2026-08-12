use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::contract::{ContractEventPayload, EventContract, sealed};
use crate::validation::{EventValidationError, ValidateEvent, validators};
use crate::{EventSchema, FieldSchema};

pub const RBAC_EVENT_USER_ROLE_REPLACED: &str = "rbac.user_role_replaced";
pub const RBAC_EVENT_USER_ROLE_ASSIGNMENT_REPAIRED: &str = "rbac.user_role_assignment_repaired";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(tag = "type", content = "data")]
pub enum RbacRoleMutationEvent {
    UserRoleReplaced {
        user_id: Uuid,
        previous_role: String,
        new_role: String,
        durable_generation: u64,
    },
    UserRoleAssignmentRepaired {
        user_id: Uuid,
        role: String,
        durable_generation: u64,
    },
}

impl RbacRoleMutationEvent {
    pub fn user_role_replaced(
        user_id: Uuid,
        previous_role: impl Into<String>,
        new_role: impl Into<String>,
        durable_generation: u64,
    ) -> Self {
        Self::UserRoleReplaced {
            user_id,
            previous_role: previous_role.into(),
            new_role: new_role.into(),
            durable_generation,
        }
    }

    pub fn user_role_assignment_repaired(
        user_id: Uuid,
        role: impl Into<String>,
        durable_generation: u64,
    ) -> Self {
        Self::UserRoleAssignmentRepaired {
            user_id,
            role: role.into(),
            durable_generation,
        }
    }

    pub fn event_type(&self) -> &'static str {
        match self {
            Self::UserRoleReplaced { .. } => RBAC_EVENT_USER_ROLE_REPLACED,
            Self::UserRoleAssignmentRepaired { .. } => RBAC_EVENT_USER_ROLE_ASSIGNMENT_REPAIRED,
        }
    }

    pub const fn schema_version(&self) -> u16 {
        1
    }
}

const RBAC_USER_ROLE_REPLACED_FIELDS: &[FieldSchema] = &[
    FieldSchema {
        name: "user_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "previous_role",
        data_type: "string",
        optional: false,
    },
    FieldSchema {
        name: "new_role",
        data_type: "string",
        optional: false,
    },
    FieldSchema {
        name: "durable_generation",
        data_type: "uint64",
        optional: false,
    },
];

const RBAC_USER_ROLE_ASSIGNMENT_REPAIRED_FIELDS: &[FieldSchema] = &[
    FieldSchema {
        name: "user_id",
        data_type: "uuid",
        optional: false,
    },
    FieldSchema {
        name: "role",
        data_type: "string",
        optional: false,
    },
    FieldSchema {
        name: "durable_generation",
        data_type: "uint64",
        optional: false,
    },
];

pub const RBAC_ROLE_MUTATION_EVENT_SCHEMAS: &[EventSchema] = &[
    EventSchema {
        event_type: RBAC_EVENT_USER_ROLE_REPLACED,
        version: 1,
        description: "A committed RBAC user role changed to a different canonical built-in role.",
        fields: RBAC_USER_ROLE_REPLACED_FIELDS,
    },
    EventSchema {
        event_type: RBAC_EVENT_USER_ROLE_ASSIGNMENT_REPAIRED,
        version: 1,
        description: "A committed RBAC mutation repaired malformed assignments while preserving the effective built-in role.",
        fields: RBAC_USER_ROLE_ASSIGNMENT_REPAIRED_FIELDS,
    },
];

impl sealed::Sealed for RbacRoleMutationEvent {}

impl EventContract for RbacRoleMutationEvent {
    fn event_type(&self) -> &'static str {
        RbacRoleMutationEvent::event_type(self)
    }

    fn schema_version(&self) -> u16 {
        RbacRoleMutationEvent::schema_version(self)
    }

    fn into_contract_payload(self) -> ContractEventPayload {
        ContractEventPayload::RbacRoleMutation(self)
    }
}

impl ValidateEvent for RbacRoleMutationEvent {
    fn validate(&self) -> Result<(), EventValidationError> {
        match self {
            Self::UserRoleReplaced {
                user_id,
                previous_role,
                new_role,
                durable_generation,
            } => {
                validators::validate_not_nil_uuid("user_id", user_id)?;
                validate_role_slug("previous_role", previous_role)?;
                validate_role_slug("new_role", new_role)?;
                if previous_role == new_role {
                    return Err(EventValidationError::InvalidValue(
                        "new_role",
                        "must differ from previous_role for a replacement event".to_string(),
                    ));
                }
                validate_generation(*durable_generation)
            }
            Self::UserRoleAssignmentRepaired {
                user_id,
                role,
                durable_generation,
            } => {
                validators::validate_not_nil_uuid("user_id", user_id)?;
                validate_role_slug("role", role)?;
                validate_generation(*durable_generation)
            }
        }
    }
}

fn validate_role_slug(field_name: &'static str, value: &str) -> Result<(), EventValidationError> {
    validators::validate_not_empty(field_name, value)?;
    if matches!(value, "super_admin" | "admin" | "manager" | "customer") {
        Ok(())
    } else {
        Err(EventValidationError::InvalidValue(
            field_name,
            "must be a canonical built-in RBAC role slug".to_string(),
        ))
    }
}

fn validate_generation(generation: u64) -> Result<(), EventValidationError> {
    if generation == 0 {
        Err(EventValidationError::InvalidValue(
            "durable_generation",
            "must be greater than zero".to_string(),
        ))
    } else {
        Ok(())
    }
}

pub fn rbac_role_mutation_event_schema(event_type: &str) -> Option<&'static EventSchema> {
    RBAC_ROLE_MUTATION_EVENT_SCHEMAS
        .iter()
        .find(|schema| schema.event_type == event_type)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ContractEventEnvelope;

    #[test]
    fn replacement_and_repair_contracts_validate() {
        let user_id = Uuid::new_v4();
        RbacRoleMutationEvent::user_role_replaced(user_id, "manager", "admin", 7)
            .validate()
            .unwrap();
        RbacRoleMutationEvent::user_role_assignment_repaired(user_id, "manager", 8)
            .validate()
            .unwrap();
    }

    #[test]
    fn registered_envelope_accepts_role_replacement_contract() {
        let envelope = ContractEventEnvelope::new(
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
            RbacRoleMutationEvent::user_role_replaced(Uuid::new_v4(), "manager", "admin", 9),
        )
        .expect("RBAC role mutation event must be registered");

        assert_eq!(envelope.event_type(), RBAC_EVENT_USER_ROLE_REPLACED);
        assert_eq!(envelope.schema_version(), 1);
    }

    #[test]
    fn replacement_rejects_same_role_and_zero_generation() {
        let same =
            RbacRoleMutationEvent::user_role_replaced(Uuid::new_v4(), "manager", "manager", 1);
        assert!(same.validate().is_err());

        let zero =
            RbacRoleMutationEvent::user_role_assignment_repaired(Uuid::new_v4(), "manager", 0);
        assert!(zero.validate().is_err());
    }

    #[test]
    fn role_slug_is_closed_to_canonical_builtins() {
        let event =
            RbacRoleMutationEvent::user_role_assignment_repaired(Uuid::new_v4(), "custom_owner", 1);
        assert!(event.validate().is_err());
    }
}
