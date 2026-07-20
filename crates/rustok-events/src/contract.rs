use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;
use ulid::Ulid;
use uuid::Uuid;

use crate::{
    DomainEvent, EventEnvelope, EventValidationError, MarketplaceListingEvent, ValidateEvent,
};

pub(crate) mod sealed {
    pub trait Sealed {}
}

/// Closed platform contract for typed events accepted by durable transports.
///
/// Implementations live in `rustok-events`; domain modules cannot publish
/// arbitrary string event names or unregistered payloads.
#[allow(private_bounds)]
pub trait EventContract:
    sealed::Sealed + Clone + Serialize + DeserializeOwned + ValidateEvent + Send + Sync + 'static
{
    fn event_type(&self) -> &'static str;
    fn schema_version(&self) -> u16;
    fn into_contract_payload(self) -> ContractEventPayload;
}

/// Typed family wrapper used by durable and streaming transports.
///
/// Adding a bounded family requires one platform variant, while lifecycle
/// evolution remains inside the family's own enum.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "family", content = "event")]
pub enum ContractEventPayload {
    #[serde(rename = "root")]
    Root(DomainEvent),
    #[serde(rename = "marketplace_listing")]
    MarketplaceListing(MarketplaceListingEvent),
}

impl ContractEventPayload {
    fn event_type(&self) -> &'static str {
        match self {
            Self::Root(event) => event.event_type(),
            Self::MarketplaceListing(event) => event.event_type(),
        }
    }

    fn schema_version(&self) -> u16 {
        match self {
            Self::Root(event) => event.schema_version(),
            Self::MarketplaceListing(event) => event.schema_version(),
        }
    }
}

impl ValidateEvent for ContractEventPayload {
    fn validate(&self) -> Result<(), EventValidationError> {
        match self {
            Self::Root(event) => event.validate(),
            Self::MarketplaceListing(event) => event.validate(),
        }
    }
}

impl sealed::Sealed for DomainEvent {}

impl EventContract for DomainEvent {
    fn event_type(&self) -> &'static str {
        DomainEvent::event_type(self)
    }

    fn schema_version(&self) -> u16 {
        DomainEvent::schema_version(self)
    }

    fn into_contract_payload(self) -> ContractEventPayload {
        ContractEventPayload::Root(self)
    }
}

/// Transport-neutral envelope for any sealed typed platform event contract.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
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
        self.event_type.as_str()
    }

    pub fn schema_version(&self) -> u16 {
        self.schema_version
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn payload(&self) -> Result<&ContractEventPayload, EventContractEnvelopeError> {
        self.validate_registered_schema()?;
        Ok(&self.event)
    }

    pub fn into_payload(self) -> Result<ContractEventPayload, EventContractEnvelopeError> {
        self.validate_registered_schema()?;
        Ok(self.event)
    }

    pub fn validate_registered_schema(&self) -> Result<(), EventContractEnvelopeError> {
        if self.id.is_nil() {
            return Err(EventValidationError::NilUuid("id").into());
        }
        if self.correlation_id.is_nil() {
            return Err(EventValidationError::NilUuid("correlation_id").into());
        }
        if self.tenant_id.is_nil() {
            return Err(EventValidationError::NilUuid("tenant_id").into());
        }
        if self.actor_id.is_some_and(|actor_id| actor_id.is_nil()) {
            return Err(EventValidationError::NilUuid("actor_id").into());
        }
        self.event.validate()?;
        let schema = crate::event_schema(&self.event_type).ok_or_else(|| {
            EventContractEnvelopeError::UnregisteredEventType(self.event_type.clone())
        })?;
        if self.schema_version != schema.version {
            return Err(EventContractEnvelopeError::SchemaVersionMismatch {
                event_type: self.event_type.clone(),
                envelope_version: self.schema_version,
                registered_version: schema.version,
            });
        }
        if self.event_type != self.event.event_type()
            || self.schema_version != self.event.schema_version()
        {
            return Err(EventContractEnvelopeError::PayloadMetadataMismatch {
                envelope_type: self.event_type.clone(),
                envelope_version: self.schema_version,
                payload_type: self.event.event_type().to_string(),
                payload_version: self.event.schema_version(),
            });
        }
        Ok(())
    }

    pub fn into_root_envelope(self) -> Result<EventEnvelope, EventContractEnvelopeError> {
        self.validate_registered_schema()?;
        let Self {
            id,
            event_type,
            schema_version,
            correlation_id,
            causation_id,
            tenant_id,
            trace_id,
            timestamp,
            actor_id,
            event,
            retry_count,
        } = self;
        let ContractEventPayload::Root(event) = event else {
            return Err(EventContractEnvelopeError::NotRootEvent(event_type));
        };
        Ok(EventEnvelope {
            id,
            event_type,
            schema_version,
            correlation_id,
            causation_id,
            tenant_id,
            trace_id,
            timestamp,
            actor_id,
            event,
            retry_count,
        })
    }
}

#[derive(Debug, Error)]
pub enum EventContractEnvelopeError {
    #[error("event contract validation failed: {0}")]
    Validation(#[from] EventValidationError),
    #[error("event contract type `{0}` is not registered")]
    UnregisteredEventType(String),
    #[error(
        "event contract schema version mismatch for `{event_type}`: envelope={envelope_version}, registered={registered_version}"
    )]
    SchemaVersionMismatch {
        event_type: String,
        envelope_version: u16,
        registered_version: u16,
    },
    #[error(
        "event contract payload metadata mismatch: envelope=`{envelope_type}`/{envelope_version}, payload=`{payload_type}`/{payload_version}"
    )]
    PayloadMetadataMismatch {
        envelope_type: String,
        envelope_version: u16,
        payload_type: String,
        payload_version: u16,
    },
    #[error("event contract `{0}` is not a root DomainEvent")]
    NotRootEvent(String),
}
