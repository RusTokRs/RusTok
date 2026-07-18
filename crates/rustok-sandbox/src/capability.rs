use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use url::Url;
use uuid::Uuid;

use crate::{
    ExecutionPhase, SandboxCancellation, SandboxContext, SandboxError, SandboxPolicy,
    SandboxResult, SandboxSubject,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct CapabilityName(String);

impl CapabilityName {
    pub fn new(value: impl Into<String>) -> SandboxResult<Self> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 96
            && value.chars().all(|character| {
                character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || matches!(character, '_' | '.' | ':')
            });
        if !valid {
            return Err(SandboxError::InvalidRequest(format!(
                "invalid capability name `{value}`"
            )));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CapabilityName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for CapabilityName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityGrant {
    pub name: CapabilityName,
    #[serde(default)]
    pub constraints: Value,
}

/// Typed policy for the `platform.http` capability.
///
/// A grant must name every allowed host, HTTP method and path prefix. Matching
/// is exact for hosts and methods and prefix-based for paths; there are no
/// implicit wildcards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpCapabilityConstraints {
    pub hosts: Vec<String>,
    pub methods: Vec<String>,
    pub path_prefixes: Vec<String>,
}

impl HttpCapabilityConstraints {
    fn from_grant(grant: &CapabilityGrant) -> SandboxResult<Self> {
        let constraints =
            serde_json::from_value::<Self>(grant.constraints.clone()).map_err(|error| {
                SandboxError::CapabilityConstraintDenied {
                    capability: grant.name.clone(),
                    reason: format!("invalid HTTP constraints: {error}"),
                }
            })?;
        if constraints.hosts.is_empty()
            || constraints.methods.is_empty()
            || constraints.path_prefixes.is_empty()
        {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: grant.name.clone(),
                reason: "HTTP constraints require non-empty hosts, methods, and path_prefixes"
                    .to_string(),
            });
        }
        if constraints.hosts.iter().any(|host| host.trim().is_empty())
            || constraints
                .methods
                .iter()
                .any(|method| method.trim().is_empty())
            || constraints
                .path_prefixes
                .iter()
                .any(|prefix| !prefix.starts_with('/'))
        {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: grant.name.clone(),
                reason:
                    "HTTP hosts and methods must be non-empty and path_prefixes must start with `/`"
                        .to_string(),
            });
        }
        Ok(constraints)
    }

    fn validate(&self, call: &CapabilityCall) -> SandboxResult<()> {
        let input =
            call.input
                .as_object()
                .ok_or_else(|| SandboxError::CapabilityConstraintDenied {
                    capability: call.capability.clone(),
                    reason: "HTTP input must be an object".to_string(),
                })?;
        let method = input.get("method").and_then(Value::as_str).ok_or_else(|| {
            SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: "HTTP input must contain a string method".to_string(),
            }
        })?;
        let raw_url = input.get("url").and_then(Value::as_str).ok_or_else(|| {
            SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: "HTTP input must contain a string url".to_string(),
            }
        })?;
        let url = Url::parse(raw_url).map_err(|_| SandboxError::CapabilityConstraintDenied {
            capability: call.capability.clone(),
            reason: "HTTP url must be absolute".to_string(),
        })?;
        let host = url
            .host_str()
            .ok_or_else(|| SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: "HTTP url must include a host".to_string(),
            })?;

        if !self
            .hosts
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(host))
        {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: format!("HTTP host `{host}` is not allowed"),
            });
        }
        if !self.methods.iter().any(|allowed| allowed == method) {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: format!("HTTP method `{method}` is not allowed"),
            });
        }
        if !self
            .path_prefixes
            .iter()
            .any(|prefix| url.path().starts_with(prefix))
        {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: format!("HTTP path `{}` is not allowed", url.path()),
            });
        }
        Ok(())
    }
}

/// Typed policy for the `platform.secrets` capability.
///
/// Guests may name only an admitted logical reference and operation. Resolver
/// aliases, resolver keys, and secret values never appear in the guest input
/// contract. The owner-provided handle broker returns only the logical reference
/// and revision; a value-consuming broker remains separate work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecretReferenceCapabilityConstraints {
    pub references: Vec<String>,
    pub operations: Vec<String>,
}

impl SecretReferenceCapabilityConstraints {
    fn from_grant(grant: &CapabilityGrant) -> SandboxResult<Self> {
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
        let mut references = std::collections::BTreeSet::new();
        if constraints.references.iter().any(|reference| {
            !valid_secret_reference_name(reference) || !references.insert(reference)
        }) {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: grant.name.clone(),
                reason: "secret-reference names must be unique lowercase logical identifiers"
                    .to_string(),
            });
        }
        let mut operations = std::collections::BTreeSet::new();
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

    fn validate(&self, call: &CapabilityCall) -> SandboxResult<()> {
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

/// Typed policy for the `platform.events` capability.
///
/// A grant names the exact event operations and event topics an artifact can
/// publish. Topics may use only a terminal `.*` wildcard, matching the admitted
/// artifact event-binding contract; a global wildcard is never valid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventCapabilityConstraints {
    pub topics: Vec<String>,
    pub operations: Vec<String>,
}

impl EventCapabilityConstraints {
    fn from_grant(grant: &CapabilityGrant) -> SandboxResult<Self> {
        let constraints =
            serde_json::from_value::<Self>(grant.constraints.clone()).map_err(|error| {
                SandboxError::CapabilityConstraintDenied {
                    capability: grant.name.clone(),
                    reason: format!("invalid event constraints: {error}"),
                }
            })?;
        if constraints.topics.is_empty() || constraints.operations.is_empty() {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: grant.name.clone(),
                reason: "event constraints require non-empty topics and operations".to_string(),
            });
        }
        let mut topics = std::collections::BTreeSet::new();
        if constraints
            .topics
            .iter()
            .any(|topic| !valid_event_topic(topic) || !topics.insert(topic))
        {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: grant.name.clone(),
                reason: "event topics must be unique exact or terminal-wildcard identifiers"
                    .to_string(),
            });
        }
        let mut operations = std::collections::BTreeSet::new();
        if constraints.operations.iter().any(|operation| {
            !valid_capability_operation(operation) || !operations.insert(operation)
        }) {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: grant.name.clone(),
                reason: "event operations must be unique lowercase logical identifiers".to_string(),
            });
        }
        Ok(constraints)
    }

    fn validate(&self, call: &CapabilityCall) -> SandboxResult<()> {
        let input =
            call.input
                .as_object()
                .ok_or_else(|| SandboxError::CapabilityConstraintDenied {
                    capability: call.capability.clone(),
                    reason: "event input must be an object".to_string(),
                })?;
        if input.keys().any(|key| key != "topic" && key != "payload") {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: "event input may contain only topic and payload".to_string(),
            });
        }
        let topic = input.get("topic").and_then(Value::as_str).ok_or_else(|| {
            SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: "event input must contain a string topic".to_string(),
            }
        })?;
        if !valid_event_topic(topic)
            || !self
                .topics
                .iter()
                .any(|allowed| event_topic_matches(allowed, topic))
        {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: "event topic is not allowed".to_string(),
            });
        }
        if !self
            .operations
            .iter()
            .any(|allowed| allowed == &call.operation)
        {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: "event operation is not allowed".to_string(),
            });
        }
        Ok(())
    }
}

fn valid_capability_operation(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '_' | '.')
        })
}

fn valid_event_topic(value: &str) -> bool {
    if value.is_empty() || value.len() > 128 || value == "*" {
        return false;
    }
    let segments = value.split('.').collect::<Vec<_>>();
    segments.iter().enumerate().all(|(index, segment)| {
        if *segment == "*" {
            return index + 1 == segments.len();
        }
        !segment.is_empty()
            && segment.len() <= 63
            && segment.chars().all(|character| {
                character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || matches!(character, '_' | '-')
            })
    })
}

fn event_topic_matches(subscription: &str, topic: &str) -> bool {
    subscription == topic
        || subscription
            .strip_suffix(".*")
            .is_some_and(|prefix| topic.starts_with(&format!("{prefix}.")))
}

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
    fn from_grant(grant: &CapabilityGrant) -> SandboxResult<Self> {
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
        let mut prefixes = std::collections::BTreeSet::new();
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
        let mut operations = std::collections::BTreeSet::new();
        if constraints.operations.iter().any(|operation| {
            !matches!(operation.as_str(), "get" | "put" | "put_batch" | "list")
                || !operations.insert(operation)
        }) {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: grant.name.clone(),
                reason:
                    "data operations must be unique and limited to get, put, put_batch, or list"
                        .to_string(),
            });
        }
        Ok(constraints)
    }

    fn validate(&self, call: &CapabilityCall) -> SandboxResult<()> {
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
            "get" => {
                reject_unexpected_data_fields(call, input, &["key"])?;
                self.validate_key(call, required_data_string(call, input, "key")?)
            }
            "put" => {
                self.validate_write(call, input)?;
                Ok(())
            }
            "put_batch" => {
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
                let mut keys = std::collections::BTreeSet::new();
                let mut idempotency_keys = std::collections::BTreeSet::new();
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
            "list" => {
                reject_unexpected_data_fields(call, input, &["prefix", "after_key", "limit"])?;
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
                let limit = input.get("limit").and_then(Value::as_u64);
                if limit.filter(|limit| (1..=100).contains(limit)).is_none() {
                    return Err(data_constraint_error(
                        call,
                        "data list limit must be between 1 and 100",
                    ));
                }
                Ok(())
            }
            _ => Err(data_constraint_error(call, "data operation is unsupported")),
        }
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
        if let Some(revision) = input.get("expected_revision") {
            if revision.as_u64().filter(|revision| *revision > 0).is_none() {
                return Err(data_constraint_error(
                    call,
                    "data expected_revision must be a positive integer",
                ));
            }
        }
        Ok((key, idempotency_key))
    }
}

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
    fn from_grant(grant: &CapabilityGrant) -> SandboxResult<Self> {
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
        let mut prefixes = std::collections::BTreeSet::new();
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
        let mut operations = std::collections::BTreeSet::new();
        if constraints.operations.iter().any(|operation| {
            !matches!(operation.as_str(), "get_metadata" | "read" | "put" | "list")
                || !operations.insert(operation)
        }) {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: grant.name.clone(),
                reason: "object-data operations must be unique and limited to get_metadata, read, put, or list".to_string(),
            });
        }
        Ok(constraints)
    }

    fn validate(&self, call: &CapabilityCall) -> SandboxResult<()> {
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
            "get_metadata" | "read" => {
                reject_unexpected_data_fields(call, input, &["name"])?;
                self.validate_name(call, required_data_string(call, input, "name")?)
            }
            "put" => {
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
                let idempotency_key = required_data_string(call, input, "idempotency_key")?;
                if Uuid::parse_str(idempotency_key).is_err() {
                    return Err(data_constraint_error(
                        call,
                        "object-data idempotency_key must be a UUID",
                    ));
                }
                if let Some(revision) = input.get("expected_revision") {
                    if revision.as_u64().filter(|revision| *revision > 0).is_none() {
                        return Err(data_constraint_error(
                            call,
                            "object-data expected_revision must be a positive integer",
                        ));
                    }
                }
                Ok(())
            }
            "list" => {
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
            _ => Err(data_constraint_error(
                call,
                "object-data operation is unsupported",
            )),
        }
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

fn data_constraint_error(call: &CapabilityCall, reason: &str) -> SandboxError {
    SandboxError::CapabilityConstraintDenied {
        capability: call.capability.clone(),
        reason: reason.to_string(),
    }
}

fn required_data_string<'a>(
    call: &CapabilityCall,
    input: &'a serde_json::Map<String, Value>,
    field: &str,
) -> SandboxResult<&'a str> {
    input
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| data_constraint_error(call, &format!("data {field} must be a string")))
}

fn reject_unexpected_data_fields(
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

fn valid_data_prefix(value: &str) -> bool {
    value
        .strip_suffix('/')
        .is_some_and(|key| !key.ends_with('/') && valid_data_key(key))
}

fn valid_data_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && !value.starts_with('/')
        && value.split('/').all(|segment| {
            !segment.is_empty() && segment != "." && segment != ".." && !segment.contains('\\')
        })
}

/// One admitted MCP server/tool target. The transport endpoint, credentials,
/// and tool implementation remain deployment-owned and never appear here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpToolGrant {
    pub server: String,
    pub tool: String,
}

/// Typed policy for the `platform.mcp` capability.
///
/// Sandbox calls use a preconfigured server alias and an exact tool name. A
/// guest never chooses an MCP endpoint, transport, credential, or arbitrary
/// tool discovery target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpCapabilityConstraints {
    pub tools: Vec<McpToolGrant>,
    pub operations: Vec<String>,
}

impl McpCapabilityConstraints {
    fn from_grant(grant: &CapabilityGrant) -> SandboxResult<Self> {
        let constraints =
            serde_json::from_value::<Self>(grant.constraints.clone()).map_err(|error| {
                SandboxError::CapabilityConstraintDenied {
                    capability: grant.name.clone(),
                    reason: format!("invalid MCP constraints: {error}"),
                }
            })?;
        if constraints.tools.is_empty() || constraints.operations.is_empty() {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: grant.name.clone(),
                reason: "MCP constraints require non-empty tools and operations".to_string(),
            });
        }
        if constraints.tools.iter().enumerate().any(|(index, target)| {
            !valid_mcp_name(&target.server)
                || !valid_mcp_name(&target.tool)
                || constraints.tools[..index]
                    .iter()
                    .any(|previous| previous == target)
        }) {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: grant.name.clone(),
                reason: "MCP server/tool targets must be unique logical identifiers".to_string(),
            });
        }
        let mut operations = std::collections::BTreeSet::new();
        if constraints
            .operations
            .iter()
            .any(|operation| operation != "call" || !operations.insert(operation))
        {
            return Err(SandboxError::CapabilityConstraintDenied {
                capability: grant.name.clone(),
                reason: "MCP operations must contain only the unique call operation".to_string(),
            });
        }
        Ok(constraints)
    }

    fn validate(&self, call: &CapabilityCall) -> SandboxResult<()> {
        if call.operation != "call" || !self.operations.iter().any(|operation| operation == "call")
        {
            return Err(mcp_constraint_error(call, "MCP operation is not allowed"));
        }
        let input = call
            .input
            .as_object()
            .ok_or_else(|| mcp_constraint_error(call, "MCP input must be an object"))?;
        if input
            .keys()
            .any(|field| field != "server" && field != "tool" && field != "arguments")
        {
            return Err(mcp_constraint_error(
                call,
                "MCP input may contain only server, tool, and arguments",
            ));
        }
        let server = required_mcp_string(call, input, "server")?;
        let tool = required_mcp_string(call, input, "tool")?;
        if !self
            .tools
            .iter()
            .any(|target| target.server == server && target.tool == tool)
        {
            return Err(mcp_constraint_error(
                call,
                "MCP server/tool target is not allowed",
            ));
        }
        Ok(())
    }
}

fn mcp_constraint_error(call: &CapabilityCall, reason: &str) -> SandboxError {
    SandboxError::CapabilityConstraintDenied {
        capability: call.capability.clone(),
        reason: reason.to_string(),
    }
}

fn required_mcp_string<'a>(
    call: &CapabilityCall,
    input: &'a serde_json::Map<String, Value>,
    field: &str,
) -> SandboxResult<&'a str> {
    input
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| valid_mcp_name(value))
        .ok_or_else(|| mcp_constraint_error(call, &format!("MCP {field} is invalid")))
}

fn valid_mcp_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && !matches!(value.chars().next(), Some('.' | '-' | '_'))
        && !matches!(value.chars().next_back(), Some('.' | '-' | '_'))
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '_' | '-' | '.')
        })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityCall {
    pub execution_id: Uuid,
    pub subject: SandboxSubject,
    pub context: CapabilityCallContext,
    pub capability: CapabilityName,
    pub operation: String,
    #[serde(default)]
    pub input: Value,
}

/// Request identity propagated to every broker call.
///
/// The host compares this value with the active sandbox request before it
/// evaluates a grant or invokes a broker, preventing an adapter from invoking
/// a granted capability on behalf of another tenant or actor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityCallContext {
    pub phase: ExecutionPhase,
    pub tenant_id: Option<Uuid>,
    pub actor_id: Option<String>,
    pub trace_id: Option<String>,
}

impl From<&SandboxContext> for CapabilityCallContext {
    fn from(context: &SandboxContext) -> Self {
        Self {
            phase: context.phase,
            tenant_id: context.tenant_id,
            actor_id: context.actor_id.clone(),
            trace_id: context.trace_id.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityResponse {
    #[serde(default)]
    pub output: Value,
}

#[async_trait]
pub trait CapabilityBroker: Send + Sync {
    async fn invoke(
        &self,
        call: &CapabilityCall,
        grant: &CapabilityGrant,
    ) -> SandboxResult<CapabilityResponse>;
}

/// Composes owner-provided capability adapters without giving any adapter a
/// platform-global fallback. A call is routed by its exact capability name;
/// an unregistered capability remains denied even when another adapter is
/// registered for the same execution.
#[derive(Clone, Default)]
pub struct CapabilityBrokerRouter {
    routes: Arc<HashMap<CapabilityName, Arc<dyn CapabilityBroker>>>,
}

impl CapabilityBrokerRouter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers exactly one owner adapter for a capability name.
    ///
    /// Duplicate registration is rejected instead of silently replacing the
    /// first owner. This keeps capability ownership explicit in host
    /// composition.
    pub fn route(
        mut self,
        capability: CapabilityName,
        broker: Arc<dyn CapabilityBroker>,
    ) -> SandboxResult<Self> {
        let routes = Arc::make_mut(&mut self.routes);
        if routes.insert(capability.clone(), broker).is_some() {
            return Err(SandboxError::InvalidRequest(format!(
                "capability broker route `{capability}` is already registered"
            )));
        }
        Ok(self)
    }
}

#[async_trait]
impl CapabilityBroker for CapabilityBrokerRouter {
    async fn invoke(
        &self,
        call: &CapabilityCall,
        grant: &CapabilityGrant,
    ) -> SandboxResult<CapabilityResponse> {
        if grant.name != call.capability {
            return Err(SandboxError::CapabilityDenied(call.capability.clone()));
        }
        let broker = self
            .routes
            .get(&call.capability)
            .ok_or_else(|| SandboxError::CapabilityDenied(call.capability.clone()))?;
        broker.invoke(call, grant).await
    }
}

/// Redacted evidence for one capability attempt.
///
/// This record intentionally excludes capability input, output, credentials and
/// broker error text. Durable observers can correlate a denial without turning
/// protected payload into audit data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityAuditRecord {
    pub execution_id: Uuid,
    pub subject: SandboxSubject,
    pub context: CapabilityCallContext,
    pub capability: CapabilityName,
    pub operation: String,
    pub timestamp: DateTime<Utc>,
    pub outcome: CapabilityAuditOutcome,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityAuditOutcome {
    Succeeded,
    Denied,
    Failed,
}

#[async_trait]
pub trait CapabilityObserver: Send + Sync {
    async fn observe(&self, record: &CapabilityAuditRecord);
}

#[derive(Clone)]
pub struct SandboxHost {
    policy: Arc<SandboxPolicy>,
    broker: Arc<dyn CapabilityBroker>,
    execution_id: Uuid,
    subject: SandboxSubject,
    context: CapabilityCallContext,
    budget: Arc<CapabilityBudget>,
    observers: Arc<Vec<Arc<dyn CapabilityObserver>>>,
    cancellation: SandboxCancellation,
}

#[derive(Debug, Default)]
struct CapabilityBudget {
    calls: AtomicU32,
    blocking_bridges: AtomicU32,
    rate_window: Mutex<VecDeque<Instant>>,
}

struct BlockingBridgePermit<'a>(&'a AtomicU32);

impl Drop for BlockingBridgePermit<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Release);
    }
}

impl SandboxHost {
    pub(crate) fn new(
        policy: Arc<SandboxPolicy>,
        broker: Arc<dyn CapabilityBroker>,
        subject: SandboxSubject,
        context: &SandboxContext,
        observers: Arc<Vec<Arc<dyn CapabilityObserver>>>,
        cancellation: SandboxCancellation,
    ) -> Self {
        Self {
            policy,
            broker,
            execution_id: context.execution_id,
            subject,
            context: CapabilityCallContext::from(context),
            budget: Arc::new(CapabilityBudget::default()),
            observers,
            cancellation,
        }
    }

    pub fn policy(&self) -> &SandboxPolicy {
        &self.policy
    }

    pub fn cancellation(&self) -> SandboxCancellation {
        self.cancellation.clone()
    }

    pub(crate) fn capability_calls(&self) -> u32 {
        self.budget.calls.load(Ordering::Acquire)
    }

    pub async fn invoke(&self, call: &CapabilityCall) -> SandboxResult<CapabilityResponse> {
        let result = self.invoke_inner(call).await;
        self.observe_capability(call, &result).await;
        result
    }

    async fn invoke_inner(&self, call: &CapabilityCall) -> SandboxResult<CapabilityResponse> {
        if self.cancellation.is_cancelled() {
            return Err(SandboxError::Cancelled);
        }
        self.validate_call_context(call)?;
        self.admit_capability_call(call)?;
        let grant = self
            .policy
            .grant(&call.capability)
            .ok_or_else(|| SandboxError::CapabilityDenied(call.capability.clone()))?;
        self.validate_constraints(call, grant)?;
        self.broker.invoke(call, grant).await
    }

    async fn observe_capability(
        &self,
        call: &CapabilityCall,
        result: &SandboxResult<CapabilityResponse>,
    ) {
        let (outcome, error_code) = match result {
            Ok(_) => (CapabilityAuditOutcome::Succeeded, None),
            Err(error) if is_denied(error) => (
                CapabilityAuditOutcome::Denied,
                Some(error.code().to_string()),
            ),
            Err(error) => (
                CapabilityAuditOutcome::Failed,
                Some(error.code().to_string()),
            ),
        };
        let record = CapabilityAuditRecord {
            execution_id: self.execution_id,
            subject: self.subject.clone(),
            context: self.context.clone(),
            capability: call.capability.clone(),
            operation: call.operation.clone(),
            timestamp: Utc::now(),
            outcome,
            error_code,
        };
        for observer in self.observers.iter() {
            observer.observe(&record).await;
        }
    }

    fn validate_call_context(&self, call: &CapabilityCall) -> SandboxResult<()> {
        if call.execution_id != self.execution_id {
            return Err(SandboxError::CapabilityContextMismatch {
                field: "execution_id",
            });
        }
        if call.subject != self.subject {
            return Err(SandboxError::CapabilityContextMismatch { field: "subject" });
        }
        if call.context != self.context {
            return Err(SandboxError::CapabilityContextMismatch { field: "context" });
        }
        Ok(())
    }

    fn admit_capability_call(&self, call: &CapabilityCall) -> SandboxResult<()> {
        let input_bytes = serde_json::to_vec(&call.input)
            .map_err(|error| SandboxError::Internal(error.to_string()))?
            .len() as u64;
        let limits = &self.policy.limits;
        if input_bytes > limits.max_capability_input_bytes {
            return Err(SandboxError::LimitExceeded {
                resource: "capability_input_bytes".to_string(),
                limit: limits.max_capability_input_bytes,
            });
        }

        self.admit_capability_rate(limits.max_capability_calls_per_second)?;

        let previous = self.budget.calls.fetch_add(1, Ordering::AcqRel);
        if previous >= limits.max_capability_calls {
            self.budget.calls.fetch_sub(1, Ordering::AcqRel);
            return Err(SandboxError::LimitExceeded {
                resource: "capability_calls".to_string(),
                limit: limits.max_capability_calls.into(),
            });
        }
        Ok(())
    }

    fn admit_capability_rate(&self, max_calls_per_second: u32) -> SandboxResult<()> {
        let now = Instant::now();
        let mut calls = self.budget.rate_window.lock().map_err(|_| {
            SandboxError::Internal("sandbox capability rate budget lock is poisoned".to_string())
        })?;
        while calls
            .front()
            .is_some_and(|started_at| now.duration_since(*started_at) >= Duration::from_secs(1))
        {
            calls.pop_front();
        }
        if calls.len() >= max_calls_per_second as usize {
            return Err(SandboxError::LimitExceeded {
                resource: "capability_calls_per_second".to_string(),
                limit: max_calls_per_second.into(),
            });
        }
        calls.push_back(now);
        Ok(())
    }

    fn validate_constraints(
        &self,
        call: &CapabilityCall,
        grant: &CapabilityGrant,
    ) -> SandboxResult<()> {
        if call.capability.as_str() == "platform.http" {
            HttpCapabilityConstraints::from_grant(grant)?.validate(call)?;
        }
        if call.capability.as_str() == "platform.secrets" {
            SecretReferenceCapabilityConstraints::from_grant(grant)?.validate(call)?;
        }
        if call.capability.as_str() == "platform.events" {
            EventCapabilityConstraints::from_grant(grant)?.validate(call)?;
        }
        if call.capability.as_str() == "platform.data" {
            DataCapabilityConstraints::from_grant(grant)?.validate(call)?;
        }
        if call.capability.as_str() == "platform.data.objects" {
            ObjectCapabilityConstraints::from_grant(grant)?.validate(call)?;
        }
        if call.capability.as_str() == "platform.mcp" {
            McpCapabilityConstraints::from_grant(grant)?.validate(call)?;
        }
        Ok(())
    }

    fn admit_blocking_bridge(&self) -> SandboxResult<BlockingBridgePermit<'_>> {
        self.budget
            .blocking_bridges
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| BlockingBridgePermit(&self.budget.blocking_bridges))
            .map_err(|_| SandboxError::LimitExceeded {
                resource: "blocking_capability_bridges".to_string(),
                limit: 1,
            })
    }

    /// Calls an async broker from a synchronous language binding.
    ///
    /// Rhai and synchronous Component Model imports use this bridge instead of
    /// opening their own network or storage clients. At most one native bridge
    /// thread may be active per execution. It requires an active Tokio runtime
    /// because the broker may perform async host I/O.
    pub fn invoke_blocking(&self, call: &CapabilityCall) -> SandboxResult<CapabilityResponse> {
        let handle = tokio::runtime::Handle::try_current().map_err(|error| {
            SandboxError::Internal(format!(
                "sandbox host capability requires an active Tokio runtime: {error}"
            ))
        })?;
        let _permit = self.admit_blocking_bridge()?;
        let host = self.clone();
        std::thread::scope(|scope| {
            scope
                .spawn(|| handle.block_on(host.invoke(call)))
                .join()
                .map_err(|_| {
                    SandboxError::Internal("sandbox host capability thread panicked".to_string())
                })?
        })
    }
}

fn is_denied(error: &SandboxError) -> bool {
    matches!(
        error,
        SandboxError::CapabilityDenied(_)
            | SandboxError::CapabilityConstraintDenied { .. }
            | SandboxError::CapabilityContextMismatch { .. }
    ) || matches!(error, SandboxError::LimitExceeded { resource, .. } if resource.starts_with("capability_"))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use serde_json::json;
    use uuid::Uuid;

    use super::{
        CapabilityBroker, CapabilityBrokerRouter, CapabilityCall, CapabilityCallContext,
        CapabilityGrant, CapabilityName, CapabilityResponse, DataCapabilityConstraints,
        EventCapabilityConstraints, McpCapabilityConstraints, ObjectCapabilityConstraints,
        SecretReferenceCapabilityConstraints,
    };
    use crate::{ExecutionPhase, SandboxError, SandboxResult, SandboxSubject};

    fn call(operation: &str, input: serde_json::Value) -> CapabilityCall {
        CapabilityCall {
            execution_id: Uuid::nil(),
            subject: SandboxSubject::ModuleArtifact {
                installation_id: Uuid::new_v4(),
                slug: "sample_module".to_string(),
                version: "1.0.0".to_string(),
                digest: "sha256:sample".to_string(),
            },
            context: CapabilityCallContext {
                phase: ExecutionPhase::Lifecycle,
                tenant_id: None,
                actor_id: None,
                trace_id: None,
            },
            capability: CapabilityName::new("platform.secrets").expect("capability name"),
            operation: operation.to_string(),
            input,
        }
    }

    #[test]
    fn secret_reference_constraints_allow_only_declared_logical_handle_calls() {
        let grant = CapabilityGrant {
            name: CapabilityName::new("platform.secrets").expect("capability name"),
            constraints: json!({
                "references": ["payment_api"],
                "operations": ["acquire_handle"]
            }),
        };
        let constraints =
            SecretReferenceCapabilityConstraints::from_grant(&grant).expect("valid constraints");

        assert!(constraints
            .validate(&call(
                "acquire_handle",
                json!({ "reference": "payment_api" })
            ))
            .is_ok());
        assert!(constraints
            .validate(&call("read", json!({ "reference": "payment_api" })))
            .is_err());
        assert!(constraints
            .validate(&call("acquire_handle", json!({ "reference": "other" })))
            .is_err());
        assert!(constraints
            .validate(&call(
                "acquire_handle",
                json!({ "reference": "payment_api", "resolver": "env" }),
            ))
            .is_err());
    }

    #[test]
    fn event_constraints_allow_only_declared_publish_topics() {
        let grant = CapabilityGrant {
            name: CapabilityName::new("platform.events").expect("capability name"),
            constraints: json!({
                "topics": ["order.*"],
                "operations": ["publish"]
            }),
        };
        let constraints =
            EventCapabilityConstraints::from_grant(&grant).expect("valid event constraints");
        let mut event_call = call(
            "publish",
            json!({ "topic": "order.completed", "payload": {} }),
        );
        event_call.capability = CapabilityName::new("platform.events").expect("capability name");
        assert!(constraints.validate(&event_call).is_ok());

        event_call.input = json!({ "topic": "orders.completed" });
        assert!(constraints.validate(&event_call).is_err());
        event_call.input = json!({ "topic": "order.completed", "resolver": "vault" });
        assert!(constraints.validate(&event_call).is_err());
    }

    #[test]
    fn data_constraints_keep_calls_inside_declared_logical_prefixes() {
        let grant = CapabilityGrant {
            name: CapabilityName::new("platform.data").expect("capability name"),
            constraints: json!({
                "key_prefixes": ["state/"],
                "operations": ["get", "put", "put_batch", "list"]
            }),
        };
        let constraints =
            DataCapabilityConstraints::from_grant(&grant).expect("valid data constraints");
        let mut data_call = call(
            "put",
            json!({
                "key": "state/answer",
                "value": 42,
                "idempotency_key": Uuid::new_v4().to_string()
            }),
        );
        data_call.capability = CapabilityName::new("platform.data").expect("capability name");
        assert!(constraints.validate(&data_call).is_ok());

        data_call.input = json!({ "key": "other/answer" });
        data_call.operation = "get".to_string();
        assert!(constraints.validate(&data_call).is_err());
        data_call.input = json!({ "prefix": "state/", "table": "module_artifact_data" });
        data_call.operation = "list".to_string();
        assert!(constraints.validate(&data_call).is_err());

        data_call.operation = "put_batch".to_string();
        data_call.input = json!({
            "writes": [
                {
                    "key": "state/one",
                    "value": 1,
                    "idempotency_key": Uuid::new_v4().to_string()
                },
                {
                    "key": "state/two",
                    "value": 2,
                    "idempotency_key": Uuid::new_v4().to_string()
                }
            ]
        });
        assert!(constraints.validate(&data_call).is_ok());

        data_call.input = json!({
            "writes": [{
                "key": "other/one",
                "value": 1,
                "idempotency_key": Uuid::new_v4().to_string()
            }]
        });
        assert!(constraints.validate(&data_call).is_err());
    }

    #[test]
    fn object_data_constraints_reject_physical_identity_and_ungranted_names() {
        let grant = CapabilityGrant {
            name: CapabilityName::new("platform.data.objects").expect("capability name"),
            constraints: json!({
                "object_prefixes": ["exports/"],
                "operations": ["get_metadata", "read", "put", "list"]
            }),
        };
        let constraints =
            ObjectCapabilityConstraints::from_grant(&grant).expect("valid object constraints");
        let mut object_call = call(
            "put",
            json!({
                "name": "exports/report.json",
                "content_type": "application/json",
                "data_base64": "e30=",
                "idempotency_key": Uuid::new_v4().to_string(),
            }),
        );
        object_call.capability =
            CapabilityName::new("platform.data.objects").expect("capability name");
        assert!(constraints.validate(&object_call).is_ok());

        object_call.input = json!({ "name": "other/report.json" });
        object_call.operation = "read".to_string();
        assert!(constraints.validate(&object_call).is_err());
        object_call.input = json!({
            "name": "exports/report.json",
            "content_type": "application/json",
            "data_base64": "e30=",
            "idempotency_key": Uuid::new_v4().to_string(),
            "storage_key": "host/private/key",
        });
        object_call.operation = "put".to_string();
        assert!(constraints.validate(&object_call).is_err());
    }

    #[test]
    fn mcp_constraints_allow_only_declared_server_tool_pairs() {
        let grant = CapabilityGrant {
            name: CapabilityName::new("platform.mcp").expect("capability name"),
            constraints: json!({
                "tools": [{ "server": "rustok", "tool": "module_details" }],
                "operations": ["call"]
            }),
        };
        let constraints =
            McpCapabilityConstraints::from_grant(&grant).expect("valid MCP constraints");
        let mut mcp_call = call(
            "call",
            json!({
                "server": "rustok",
                "tool": "module_details",
                "arguments": { "slug": "content" }
            }),
        );
        mcp_call.capability = CapabilityName::new("platform.mcp").expect("capability name");
        assert!(constraints.validate(&mcp_call).is_ok());

        mcp_call.input = json!({ "server": "rustok", "tool": "list_modules" });
        assert!(constraints.validate(&mcp_call).is_err());
        mcp_call.input = json!({
            "server": "rustok",
            "tool": "module_details",
            "endpoint": "https://attacker.invalid"
        });
        assert!(constraints.validate(&mcp_call).is_err());
    }

    struct StaticBroker(&'static str);

    #[async_trait]
    impl CapabilityBroker for StaticBroker {
        async fn invoke(
            &self,
            _call: &CapabilityCall,
            _grant: &CapabilityGrant,
        ) -> SandboxResult<CapabilityResponse> {
            Ok(CapabilityResponse {
                output: json!({ "owner": self.0 }),
            })
        }
    }

    #[tokio::test]
    async fn router_uses_only_the_exact_capability_owner() {
        let secrets = CapabilityName::new("platform.secrets").expect("capability name");
        let data = CapabilityName::new("platform.data").expect("capability name");
        let router = CapabilityBrokerRouter::new()
            .route(secrets.clone(), Arc::new(StaticBroker("secrets")))
            .expect("first route")
            .route(data.clone(), Arc::new(StaticBroker("data")))
            .expect("second route");

        let mut secret_call = call("acquire_handle", json!({ "reference": "payment_api" }));
        secret_call.capability = secrets.clone();
        let response = router
            .invoke(
                &secret_call,
                &CapabilityGrant {
                    name: secrets,
                    constraints: json!({}),
                },
            )
            .await
            .expect("secret owner response");
        assert_eq!(response.output, json!({ "owner": "secrets" }));

        secret_call.capability = CapabilityName::new("platform.events").expect("capability name");
        let error = router
            .invoke(
                &secret_call,
                &CapabilityGrant {
                    name: secret_call.capability.clone(),
                    constraints: json!({}),
                },
            )
            .await
            .expect_err("unregistered capability must remain denied");
        assert!(matches!(error, SandboxError::CapabilityDenied(_)));
    }

    #[test]
    fn router_rejects_duplicate_capability_owners() {
        let capability = CapabilityName::new("platform.data").expect("capability name");
        let result = CapabilityBrokerRouter::new()
            .route(capability.clone(), Arc::new(StaticBroker("first")))
            .expect("first route")
            .route(capability, Arc::new(StaticBroker("second")));
        assert!(matches!(result, Err(SandboxError::InvalidRequest(_))));
    }
}
