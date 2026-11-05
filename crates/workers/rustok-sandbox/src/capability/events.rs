use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{CapabilityCall, CapabilityGrant, valid_capability_operation};
use crate::{SandboxError, SandboxResult};

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
    pub(crate) fn from_grant(grant: &CapabilityGrant) -> SandboxResult<Self> {
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
        let mut topics = BTreeSet::new();
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
        let mut operations = BTreeSet::new();
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

    pub(crate) fn validate(&self, call: &CapabilityCall) -> SandboxResult<()> {
        let topic = Self::parse_event_topic(call)?;
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

    fn parse_event_topic(call: &CapabilityCall) -> SandboxResult<&str> {
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
        input.get("topic").and_then(Value::as_str).ok_or_else(|| {
            SandboxError::CapabilityConstraintDenied {
                capability: call.capability.clone(),
                reason: "event input must contain a string topic".to_string(),
            }
        })
    }
}

pub(crate) fn valid_event_topic(value: &str) -> bool {
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

pub(crate) fn event_topic_matches(subscription: &str, topic: &str) -> bool {
    subscription == topic
        || subscription
            .strip_suffix(".*")
            .is_some_and(|prefix| topic.starts_with(&format!("{prefix}.")))
}
