use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ulid::Ulid;
use uuid::Uuid;

use crate::schema::{global_event_schema_registry, EventSchemaError};
use crate::validation::{EventValidationError, ValidateEvent};

pub trait EventContract: ValidateEvent + Serialize + Send + Sync + 'static {
    fn event_type(&self) -> &'static str;

    fn schema_version(&self) -> u16;

    fn into_contract_payload(self) -> ContractEventPayload
    where
        Self: Sized,
    {
        ContractEventPayload::from_event(self)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContractEventPayload {
    pub payload: Value,
}

impl ContractEventPayload {
    pub fn from_event<E>(event: E) -> Self
    where
        E: Serialize,
    {
        Self {
            payload: serde_json::to_value(event)
                .expect("serializing a validated event contract must succeed"),
        }
    }
}

impl ValidateEvent for ContractEventPayload {
    fn validate_event(&self) -> Result<(), EventValidationError> {
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EventContractEnvelopeError {
    #[error(transparent)]
    Validation(#[from] EventValidationError),
    #[error(transparent)]
    Schema(#[from] EventSchemaError),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContractEventEnvelope {
    id: Uuid,
    event_type: String,
    schema_version: u16,
    correlation_id: Uuid,
    causation_id: Option<Uuid>,
    tenant_id: Uuid,
    trace_id: Option<String>,
    timestamp: DateTime<Utc>,
    actor_id: Option<Uuid>,
    event: ContractEventPayload,
    retry_count: u32,
}

impl ContractEventEnvelope {
    pub fn new<E>(
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        event: E,
    ) -> Result<Self, EventContractEnvelopeError>
    where
        E: EventContract,
    {
        event.validate()?;
        let event_type = event.event_type().to_string();
        let schema_version = event.schema_version();
        let id = Uuid::from_bytes(Ulid::r#gen().to_bytes());
        let envelope = Self {
            id,
            event_type,
            schema_version,
            correlation_id: id,
            causation_id: None,
            tenant_id,
            trace_id: rustok_telemetry::current_trace_id(),
            timestamp: Utc::now(),
            actor_id,
            event: event.into_contract_payload(),
            retry_count: 0,
        };
        envelope.validate_registered_schema()?;
        Ok(envelope)
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn event_type(&self) -> &str {
        &self.event_type
    }

    pub fn schema_version(&self) -> u16 {
        self.schema_version
    }

    pub fn correlation_id(&self) -> Uuid {
        self.correlation_id
    }

    pub fn causation_id(&self) -> Option<Uuid> {
        self.causation_id
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn trace_id(&self) -> Option<&str> {
        self.trace_id.as_deref()
    }

    pub fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }

    pub fn actor_id(&self) -> Option<Uuid> {
        self.actor_id
    }

    pub fn event(&self) -> &ContractEventPayload {
        &self.event
    }

    pub fn retry_count(&self) -> u32 {
        self.retry_count
    }

    pub fn set_causation_id(&mut self, causation_id: Uuid) {
        self.causation_id = Some(causation_id);
    }

    pub fn increment_retry_count(&mut self) {
        self.retry_count = self.retry_count.saturating_add(1);
    }

    pub fn validate_registered_schema(&self) -> Result<(), EventSchemaError> {
        global_event_schema_registry().validate_payload(
            &self.event_type,
            self.schema_version,
            &self.event.payload,
        )
    }
}
