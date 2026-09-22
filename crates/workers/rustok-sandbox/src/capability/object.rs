use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::data::{
    data_constraint_error, reject_unexpected_data_fields, required_data_string, valid_data_key,
    valid_data_prefix,
};
use super::{CapabilityCall, CapabilityGrant};
use crate::{SandboxError, SandboxResult};

/// Typed policy for the `platform.data.objects` capability. Object names are
/// logical paths only; the owner never accepts a bucket, URL, or storage key
/// from a sandbox call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectCapabilityConstraints {
    pub object_prefixes: Vec<String>,
    pub operations: Vec<String>,
}

impl ObjectCapabilityConstraints {
    pub(crate) fn from_grant(grant: &CapabilityGrant) -> SandboxResult<Self> {
        let constraints =
            serde_json::from_value::<Self>(grant.constraints.clone()).map_err(|error| {
                SandboxError::CapabilityConstraintDenied {
                    capability: grant.name.clone(),
                    reason: format!("invalid object-data constraints: {error}"),
                }
            })?;
        if constraints.object_prefixes.is_empty() || constraints.operations.is_empty() {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: grant.name.clone(),
                reason: "object-data constraints require non-empty object_prefixes and operations"
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
            .object_prefixes
            .iter()
            .any(|prefix| !valid_data_prefix(prefix) || !prefixes.insert(prefix))
        {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: grant.name.clone(),
                reason: "object-data prefixes must be unique logical paths ending in `/`"
                    .to_string(),
            });
        }
        Ok(())
    }

    fn validate_operations(constraints: &Self, grant: &CapabilityGrant) -> SandboxResult<()> {
        let mut operations = BTreeSet::new();
        if constraints.operations.iter().any(|operation| {
            !matches!(
                operation.as_str(),
                "get_metadata"
                    | "read"
                    | "put"
                    | "delete"
                    | "list"
                    | "begin_upload"
                    | "append_chunk"
                    | "complete_upload"
            ) || !operations.insert(operation)
        }) {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: grant.name.clone(),
                reason: "object-data operations must be unique and limited to get_metadata, read, put, delete, list, begin_upload, append_chunk, or complete_upload".to_string(),
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
            return Err(data_constraint_error(
                call,
                "object-data operation is not allowed",
            ));
        }
        let input = call
            .input
            .as_object()
            .ok_or_else(|| data_constraint_error(call, "object-data input must be an object"))?;
        match call.operation.as_str() {
            "get_metadata" | "read" => self.validate_read(call, input),
            "put" => self.validate_put(call, input),
            "delete" => self.validate_delete(call, input),
            "begin_upload" => self.validate_begin_upload(call, input),
            "append_chunk" => self.validate_append_chunk(call, input),
            "complete_upload" => self.validate_complete_upload(call, input),
            "list" => self.validate_list(call, input),
            _ => Err(data_constraint_error(
                call,
                "object-data operation is unsupported",
            )),
        }
    }

    fn validate_read(
        &self,
        call: &CapabilityCall,
        input: &serde_json::Map<String, Value>,
    ) -> SandboxResult<()> {
        reject_unexpected_data_fields(call, input, &["name"])?;
        self.validate_name(call, required_data_string(call, input, "name")?)
    }

    fn validate_put(
        &self,
        call: &CapabilityCall,
        input: &serde_json::Map<String, Value>,
    ) -> SandboxResult<()> {
        reject_unexpected_data_fields(
            call,
            input,
            &[
                "name",
                "content_type",
                "data_base64",
                "expected_revision",
                "idempotency_key",
            ],
        )?;
        self.validate_name(call, required_data_string(call, input, "name")?)?;
        Self::validate_put_payload(call, input)?;
        Self::validate_put_metadata(call, input)
    }

    fn validate_put_payload(
        call: &CapabilityCall,
        input: &serde_json::Map<String, Value>,
    ) -> SandboxResult<()> {
        if input
            .get("content_type")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
            || input
                .get("data_base64")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
        {
            return Err(data_constraint_error(
                call,
                "object-data put requires non-empty content_type and data_base64",
            ));
        }
        Ok(())
    }

    fn validate_put_metadata(
        call: &CapabilityCall,
        input: &serde_json::Map<String, Value>,
    ) -> SandboxResult<()> {
        let idempotency_key = required_data_string(call, input, "idempotency_key")?;
        if Uuid::parse_str(idempotency_key).is_err() {
            return Err(data_constraint_error(
                call,
                "object-data idempotency_key must be a UUID",
            ));
        }
        if let Some(revision) = input.get("expected_revision")
            && revision.as_u64().filter(|revision| *revision > 0).is_none()
        {
            return Err(data_constraint_error(
                call,
                "object-data expected_revision must be a positive integer",
            ));
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
            &["name", "expected_revision", "idempotency_key"],
        )?;
        self.validate_name(call, required_data_string(call, input, "name")?)?;
        if input
            .get("expected_revision")
            .and_then(Value::as_u64)
            .filter(|revision| *revision > 0)
            .is_none()
        {
            return Err(data_constraint_error(
                call,
                "object-data delete requires a positive expected_revision",
            ));
        }
        if Uuid::parse_str(required_data_string(call, input, "idempotency_key")?).is_err() {
            return Err(data_constraint_error(
                call,
                "object-data idempotency_key must be a UUID",
            ));
        }
        Ok(())
    }

    fn validate_begin_upload(
        &self,
        call: &CapabilityCall,
        input: &serde_json::Map<String, Value>,
    ) -> SandboxResult<()> {
        reject_unexpected_data_fields(
            call,
            input,
            &[
                "name",
                "content_type",
                "expected_revision",
                "idempotency_key",
            ],
        )?;
        self.validate_name(call, required_data_string(call, input, "name")?)?;
        if input
            .get("content_type")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        {
            return Err(data_constraint_error(
                call,
                "object-data begin_upload requires a non-empty content_type",
            ));
        }
        if Uuid::parse_str(required_data_string(call, input, "idempotency_key")?).is_err() {
            return Err(data_constraint_error(
                call,
                "object-data idempotency_key must be a UUID",
            ));
        }
        Ok(())
    }

    fn validate_append_chunk(
        &self,
        call: &CapabilityCall,
        input: &serde_json::Map<String, Value>,
    ) -> SandboxResult<()> {
        reject_unexpected_data_fields(
            call,
            input,
            &["session_id", "sequence", "data_base64"],
        )?;
        if Uuid::parse_str(required_data_string(call, input, "session_id")?).is_err()
            || input.get("sequence").and_then(Value::as_u64).is_none()
            || input
                .get("data_base64")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
        {
            return Err(data_constraint_error(
                call,
                "object-data append_chunk requires a UUID session_id, sequence, and data_base64",
            ));
        }
        Ok(())
    }

    fn validate_complete_upload(
        &self,
        call: &CapabilityCall,
        input: &serde_json::Map<String, Value>,
    ) -> SandboxResult<()> {
        reject_unexpected_data_fields(
            call,
            input,
            &["session_id", "size_bytes", "digest_sha256"],
        )?;
        if Uuid::parse_str(required_data_string(call, input, "session_id")?).is_err()
            || input
                .get("size_bytes")
                .and_then(Value::as_u64)
                .filter(|size| *size > 0)
                .is_none()
            || input
                .get("digest_sha256")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
        {
            return Err(data_constraint_error(
                call,
                "object-data complete_upload requires a UUID session_id, size_bytes, and digest_sha256",
            ));
        }
        Ok(())
    }

    fn validate_list(
        &self,
        call: &CapabilityCall,
        input: &serde_json::Map<String, Value>,
    ) -> SandboxResult<()> {
        reject_unexpected_data_fields(call, input, &["prefix", "after_name", "limit"])?;
        let prefix = required_data_string(call, input, "prefix")?;
        if !self.object_prefixes.iter().any(|allowed| allowed == prefix) {
            return Err(data_constraint_error(
                call,
                "object-data prefix is not allowed",
            ));
        }
        if let Some(after_name) = input.get("after_name") {
            let after_name = after_name.as_str().ok_or_else(|| {
                data_constraint_error(call, "object-data after_name must be a string")
            })?;
            if !valid_data_key(after_name) || !after_name.starts_with(prefix) {
                return Err(data_constraint_error(
                    call,
                    "object-data after_name is outside the allowed prefix",
                ));
            }
        }
        if input
            .get("limit")
            .and_then(Value::as_u64)
            .filter(|limit| (1..=100).contains(limit))
            .is_none()
        {
            return Err(data_constraint_error(
                call,
                "object-data list limit must be between 1 and 100",
            ));
        }
        Ok(())
    }

    fn validate_name(&self, call: &CapabilityCall, name: &str) -> SandboxResult<()> {
        if !valid_data_key(name)
            || !self
                .object_prefixes
                .iter()
                .any(|prefix| name.starts_with(prefix))
        {
            return Err(data_constraint_error(
                call,
                "object-data name is not allowed",
            ));
        }
        Ok(())
    }
}
