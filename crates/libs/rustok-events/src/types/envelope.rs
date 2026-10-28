use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ulid::Ulid;
use uuid::Uuid;

use super::domain_event::DomainEvent;
use crate::validation::{EventValidationError, ValidateEvent, validators};

/// Keeps the JSON event contract human-readable while encoding timestamps as
/// UTC microseconds for non-human-readable formats such as MessagePack.
pub(crate) mod timestamp_serde {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

    pub fn serialize<S>(timestamp: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            timestamp.to_rfc3339().serialize(serializer)
        } else {
            timestamp.timestamp_micros().serialize(serializer)
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let value = String::deserialize(deserializer)?;
            DateTime::parse_from_rfc3339(&value)
                .map(|timestamp| timestamp.with_timezone(&Utc))
                .map_err(D::Error::custom)
        } else {
            let micros = i64::deserialize(deserializer)?;
            DateTime::from_timestamp_micros(micros)
                .ok_or_else(|| D::Error::custom("timestamp microseconds are out of range"))
        }
    }
}

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct EventEnvelope {
    pub id: Uuid,
    /// Event type string for fast filtering and routing
    pub event_type: String,
    /// Schema version for this event type (for evolution tracking)
    pub schema_version: u16,
    pub correlation_id: Uuid,
    pub causation_id: Option<Uuid>,
    pub tenant_id: Uuid,
    pub trace_id: Option<String>,
    #[serde(with = "timestamp_serde")]
    #[schemars(with = "DateTime<Utc>")]
    pub timestamp: DateTime<Utc>,
    pub actor_id: Option<Uuid>,
    pub event: DomainEvent,
    pub retry_count: u32,
}

impl EventEnvelope {
    pub fn new(tenant_id: Uuid, actor_id: Option<Uuid>, event: DomainEvent) -> Self {
        let id = Uuid::from_bytes(Ulid::generate().to_bytes());
        let event_type = event.event_type().to_string();
        let schema_version = event.schema_version();
        Self {
            id,
            event_type,
            schema_version,
            correlation_id: id,
            causation_id: None,
            tenant_id,
            trace_id: rustok_telemetry::current_trace_id(),
            timestamp: Utc::now(),
            actor_id,
            event,
            retry_count: 0,
        }
    }

    /// Validates envelope metadata, the typed payload, and its registered
    /// schema. Every durable and remote ingress path must call this method
    /// before accepting a root event.
    pub fn validate_registered_schema(&self) -> Result<(), EventEnvelopeError> {
        if self.id.is_nil() {
            return Err(EventValidationError::NilUuid("id").into());
        }
        if self.correlation_id.is_nil() {
            return Err(EventValidationError::NilUuid("correlation_id").into());
        }
        if self.tenant_id.is_nil() && !self.event.allows_platform_scope() {
            return Err(EventValidationError::NilUuid("tenant_id").into());
        }
        validators::validate_optional_uuid("causation_id", &self.causation_id)?;
        validators::validate_optional_uuid("actor_id", &self.actor_id)?;
        if let Some(trace_id) = &self.trace_id {
            validators::validate_not_empty("trace_id", trace_id)?;
            validators::validate_max_length("trace_id", trace_id, 512)?;
        }
        self.event.validate()?;

        let schema = crate::event_schema(&self.event_type)
            .ok_or_else(|| EventEnvelopeError::UnregisteredEventType(self.event_type.clone()))?;
        if self.schema_version != schema.version {
            return Err(EventEnvelopeError::SchemaVersionMismatch {
                event_type: self.event_type.clone(),
                envelope_version: self.schema_version,
                registered_version: schema.version,
            });
        }
        if self.event_type != self.event.event_type()
            || self.schema_version != self.event.schema_version()
        {
            return Err(EventEnvelopeError::PayloadMetadataMismatch {
                envelope_type: self.event_type.clone(),
                envelope_version: self.schema_version,
                payload_type: self.event.event_type().to_string(),
                payload_version: self.event.schema_version(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum EventEnvelopeError {
    #[error("event envelope validation failed: {0}")]
    Validation(#[from] EventValidationError),
    #[error("event type `{0}` is not registered")]
    UnregisteredEventType(String),
    #[error(
        "event schema version mismatch for `{event_type}`: envelope={envelope_version}, registered={registered_version}"
    )]
    SchemaVersionMismatch {
        event_type: String,
        envelope_version: u16,
        registered_version: u16,
    },
    #[error(
        "event payload metadata mismatch: envelope=`{envelope_type}`/{envelope_version}, payload=`{payload_type}`/{payload_version}"
    )]
    PayloadMetadataMismatch {
        envelope_type: String,
        envelope_version: u16,
        payload_type: String,
        payload_version: u16,
    },
}

impl std::fmt::Debug for EventEnvelope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventEnvelope")
            .field("id", &self.id)
            .field("type", &self.event_type)
            .field("tenant_id", &self.tenant_id)
            .field("actor_id", &self.actor_id)
            .field("timestamp", &self.timestamp)
            .finish_non_exhaustive()
    }
}
