use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::{CapabilityCall, CapabilityGrant};
use crate::{SandboxError, SandboxResult};

/// Typed policy for the `platform.data` capability.
///
/// The tenant/module/data-contract namespace is injected by the host, never
/// named by a guest. Grants narrow that injected namespace to logical key
/// prefixes and the structured operations implemented by the owner data broker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataCapabilityConstraints {
    pub key_prefixes: Vec<String>,
    pub operations: Vec<String>,
}

impl DataCapabilityConstraints {
    pub(crate) fn from_grant(grant: &CapabilityGrant) -> SandboxResult<Self> {
        let constraints =
            serde_json::from_value::<Self>(grant.constraints.clone()).map_err(|error| {
                SandboxError::CapabilityConstraintDenied {
                    capability: grant.name.clone(),
                    reason: format!("invalid data constraints: {error}"),
                }
            })?;
        if constraints.key_prefixes.is_empty() || constraints.operations.is_empty() {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: grant.name.clone(),
                reason: "data constraints require non-empty key_prefixes and operations"
                    .to_string(),
            });
        }
        Self::validate_prefixes(&constraints, grant)?;
        Self::validate_operations(&constraints, grant)?;
        Ok(constraints)
    }

    fn validate_prefixes(constraints: &Self, grant: &CapabilityGrant) -> SandboxResult<()> {
        let mut prefixes = BTreeSet::new();
        if constraints
            .key_prefixes
            .iter()
            .any(|prefix| !valid_data_prefix(prefix) || !prefixes.insert(prefix))
        {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: grant.name.clone(),
                reason: "data key prefixes must be unique logical paths ending in `/`".to_string(),
            });
        }
        Ok(())
    }

    fn validate_operations(constraints: &Self, grant: &CapabilityGrant) -> SandboxResult<()> {
        let mut operations = BTreeSet::new();
        if constraints.operations.iter().any(|operation| {
            !matches!(
                operation.as_str(),
                "get" | "put" | "put_batch" | "delete" | "list" | "query_index"
            ) || !operations.insert(operation)
        }) {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: grant.name.clone(),
                reason:
                    "data operations must be unique and limited to get, put, put_batch, delete, list, or query_index"
                        .to_string(),
            });
        }
        Ok(())
    }

    pub(crate) fn validate(&self, call: &CapabilityCall) -> SandboxResult<()> {
        if !self
            .operations
            .iter()
            .any(|allowed| allowed == &call.operation)
        {
            return Err(data_constraint_error(call, "data operation is not allowed"));
        }
        let input = call
            .input
            .as_object()
            .ok_or_else(|| data_constraint_error(call, "data input must be an object"))?;
        match call.operation.as_str() {
            "get" => self.validate_get(call, input),
            "put" => {
                self.validate_write(call, input)?;
                Ok(())
            }
            "put_batch" => self.validate_put_batch(call, input),
            "delete" => self.validate_delete(call, input),
            "list" => self.validate_list(call, input),
            "query_index" => self.validate_query_index(call, input),
            _ => Err(data_constraint_error(call, "data operation is unsupported")),
        }
    }

    fn validate_get(
        &self,
        call: &CapabilityCall,
        input: &serde_json::Map<String, Value>,
    ) -> SandboxResult<()> {
        reject_unexpected_data_fields(call, input, &["key"])?;
        self.validate_key(call, required_data_string(call, input, "key")?)
    }

    fn validate_put_batch(
        &self,
        call: &CapabilityCall,
        input: &serde_json::Map<String, Value>,
    ) -> SandboxResult<()> {
        reject_unexpected_data_fields(call, input, &["writes"])?;
        let writes = input
            .get("writes")
            .and_then(Value::as_array)
            .ok_or_else(|| data_constraint_error(call, "data writes must be an array"))?;
        if writes.is_empty() || writes.len() > 32 {
            return Err(data_constraint_error(
                call,
                "data batch must contain between 1 and 32 writes",
            ));
        }
        let mut keys = BTreeSet::new();
        let mut idempotency_keys = BTreeSet::new();
        for write in writes {
            let write = write.as_object().ok_or_else(|| {
                data_constraint_error(call, "data batch entry must be an object")
            })?;
            let (key, idempotency_key) = self.validate_write(call, write)?;
            if !keys.insert(key) || !idempotency_keys.insert(idempotency_key) {
                return Err(data_constraint_error(
                    call,
                    "data batch keys and idempotency keys must be distinct",
                ));
            }
        }
        Ok(())
    }

    fn validate_delete(
        &self,
        call: &CapabilityCall,
        input: &serde_json::Map<String, Value>,
    ) -> SandboxResult<()> {
        reject_unexpected_data_fields(
            call,
            input,
            &["key", "expected_revision", "idempotency_key"],
        )?;
        self.validate_key(call, required_data_string(call, input, "key")?)?;
        if input
            .get("expected_revision")
            .and_then(Value::as_u64)
            .filter(|revision| *revision > 0)
            .is_none()
        {
            return Err(data_constraint_error(
                call,
                "data delete requires a positive expected_revision",
            ));
        }
        if Uuid::parse_str(required_data_string(call, input, "idempotency_key")?).is_err() {
            return Err(data_constraint_error(
                call,
                "data idempotency_key must be a UUID",
            ));
        }
        Ok(())
    }

    fn validate_list(
        &self,
        call: &CapabilityCall,
        input: &serde_json::Map<String, Value>,
    ) -> SandboxResult<()> {
        reject_unexpected_data_fields(call, input, &["prefix", "after_key", "limit"])?;
        self.validate_prefix_cursor(call, input)?;
        Self::validate_page_limit(call, input, "list limit")
    }

    fn validate_query_index(
        &self,
        call: &CapabilityCall,
        input: &serde_json::Map<String, Value>,
    ) -> SandboxResult<()> {
        reject_unexpected_data_fields(
            call,
            input,
            &["index", "value", "prefix", "after_key", "limit"],
        )?;
        let index = required_data_string(call, input, "index")?;
        if !valid_data_index_name(index) {
            return Err(data_constraint_error(call, "data index is invalid"));
        }
        if !matches!(
            input.get("value"),
            Some(Value::String(_) | Value::Number(_) | Value::Bool(_))
        ) {
            return Err(data_constraint_error(
                call,
                "data index query value must be a scalar",
            ));
        }
        self.validate_prefix_cursor(call, input)?;
        Self::validate_page_limit(call, input, "query limit")
    }

    fn validate_prefix_cursor(
        &self,
        call: &CapabilityCall,
        input: &serde_json::Map<String, Value>,
    ) -> SandboxResult<()> {
        let prefix = required_data_string(call, input, "prefix")?;
        if !self.key_prefixes.iter().any(|allowed| allowed == prefix) {
            return Err(data_constraint_error(call, "data prefix is not allowed"));
        }
        if let Some(after_key) = input.get("after_key") {
            let after_key = after_key.as_str().ok_or_else(|| {
                data_constraint_error(call, "data after_key must be a string")
            })?;
            if !valid_data_key(after_key) || !after_key.starts_with(prefix) {
                return Err(data_constraint_error(
                    call,
                    "data after_key is outside the allowed prefix",
                ));
            }
        }
        Ok(())
    }

    fn validate_page_limit(
        call: &CapabilityCall,
        input: &serde_json::Map<String, Value>,
        field_name: &str,
    ) -> SandboxResult<()> {
        let limit = input.get("limit").and_then(Value::as_u64);
        if limit.filter(|limit| (1..=100).contains(limit)).is_none() {
            return Err(data_constraint_error(
                call,
                &format!("data {field_name} must be between 1 and 100"),
            ));
        }
        Ok(())
    }

    fn validate_key(&self, call: &CapabilityCall, key: &str) -> SandboxResult<()> {
        if !valid_data_key(key)
            || !self
                .key_prefixes
                .iter()
                .any(|prefix| key.starts_with(prefix))
        {
            return Err(data_constraint_error(call, "data key is not allowed"));
        }
        Ok(())
    }

    fn validate_write<'a>(
        &self,
        call: &CapabilityCall,
        input: &'a serde_json::Map<String, Value>,
    ) -> SandboxResult<(&'a str, &'a str)> {
        reject_unexpected_data_fields(
            call,
            input,
            &["key", "value", "expected_revision", "idempotency_key"],
        )?;
        if !input.contains_key("value") {
            return Err(data_constraint_error(call, "data put input requires value"));
        }
        let key = required_data_string(call, input, "key")?;
        self.validate_key(call, key)?;
        let idempotency_key = required_data_string(call, input, "idempotency_key")?;
        if Uuid::parse_str(idempotency_key).is_err() {
            return Err(data_constraint_error(
                call,
                "data idempotency_key must be a UUID",
            ));
        }
        if let Some(revision) = input.get("expected_revision")
            && revision.as_u64().filter(|revision| *revision > 0).is_none()
        {
            return Err(data_constraint_error(
                call,
                "data expected_revision must be a positive integer",
            ));
        }
        Ok((key, idempotency_key))
    }
}

pub(crate) fn data_constraint_error(call: &CapabilityCall, reason: &str) -> SandboxError {
    SandboxError::CapabilityConstraintDenied {
        capability: call.capability.clone(),
        reason: reason.to_string(),
    }
}

pub(crate) fn required_data_string<'a>(
    call: &CapabilityCall,
    input: &'a serde_json::Map<String, Value>,
    field: &str,
) -> SandboxResult<&'a str> {
    input
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| data_constraint_error(call, &format!("data {field} must be a string")))
}

pub(crate) fn reject_unexpected_data_fields(
    call: &CapabilityCall,
    input: &serde_json::Map<String, Value>,
    allowed: &[&str],
) -> SandboxResult<()> {
    if input.keys().any(|field| !allowed.contains(&field.as_str())) {
        return Err(data_constraint_error(
            call,
            "data input contains an unsupported field",
        ));
    }
    Ok(())
}

pub(crate) fn valid_data_prefix(value: &str) -> bool {
    value
        .strip_suffix('/')
        .is_some_and(|key| !key.ends_with('/') && valid_data_key(key))
}

pub(crate) fn valid_data_index_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

pub(crate) fn valid_data_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && !value.starts_with('/')
        && value.split('/').all(|segment| {
            !segment.is_empty() && segment != "." && segment != ".." && !segment.contains('\\')
        })
}
