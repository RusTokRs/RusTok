use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{CapabilityCall, CapabilityGrant, valid_capability_operation};
use crate::{SandboxError, SandboxResult};

/// Typed policy for the `platform.secrets` capability.
///
/// Guests may name only an admitted logical reference and operation. Resolver
/// aliases, resolver keys, and secret values never appear in the guest input
/// contract. The owner-provided handle broker returns only the logical reference
/// and revision. Value consumption remains a separate host-only owner service,
/// never a sandbox `get_value` response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecretReferenceCapabilityConstraints {
    pub references: Vec<String>,
    pub operations: Vec<String>,
}

impl SecretReferenceCapabilityConstraints {
    pub(crate) fn from_grant(grant: &CapabilityGrant) -> SandboxResult<Self> {
        let constraints =
            serde_json::from_value::<Self>(grant.constraints.clone()).map_err(|error| {
                SandboxError::CapabilityConstraintDenied {
                    capability: grant.name.clone(),
                    reason: format!("invalid secret-reference constraints: {error}"),
                }
            })?;
        if constraints.references.is_empty() || constraints.operations.is_empty() {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: grant.name.clone(),
                reason: "secret-reference constraints require non-empty references and operations"
                    .to_string(),
            });
        }
        let mut references = BTreeSet::new();
        if constraints.references.iter().any(|reference| {
            !valid_secret_reference_name(reference) || !references.insert(reference)
        }) {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: grant.name.clone(),
                reason: "secret-reference names must be unique lowercase logical identifiers"
                    .to_string(),
            });
        }
        let mut operations = BTreeSet::new();
        if constraints.operations.iter().any(|operation| {
            !valid_capability_operation(operation) || !operations.insert(operation)
        }) {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: grant.name.clone(),
                reason: "secret-reference operations must be unique lowercase logical identifiers"
                    .to_string(),
            });
        }
        Ok(constraints)
    }

    pub(crate) fn validate(&self, call: &CapabilityCall) -> SandboxResult<()> {
        let input =
            call.input
                .as_object()
                .ok_or_else(|| SandboxError::CapabilityConstraintDenied {
                    capability: call.capability.clone(),
                    reason: "secret-reference input must be an object".to_string(),
                })?;
        if input.len() != 1 {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: "secret-reference input may contain only `reference`".to_string(),
            });
        }
        let reference = input
            .get("reference")
            .and_then(Value::as_str)
            .ok_or_else(|| SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: "secret-reference input must contain a string reference".to_string(),
            })?;
        if !self.references.iter().any(|allowed| allowed == reference) {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: format!("secret reference `{reference}` is not allowed"),
            });
        }
        if !self
            .operations
            .iter()
            .any(|allowed| allowed == &call.operation)
        {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: format!("secret operation `{}` is not allowed", call.operation),
            });
        }
        Ok(())
    }
}

fn valid_secret_reference_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && !value.starts_with('_')
        && !value.ends_with('_')
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '_' | '-')
        })
}
