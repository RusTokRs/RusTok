use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;
use ulid::Ulid;
use uuid::Uuid;

use crate::{
    BlogCommentsDelegationScheduleAuditEvent, DomainEvent, EventEnvelope, EventValidationError,
    ForumMentionEvent, ForumSearchProjectionEvent, MarketplaceListingEvent, MarketplaceSellerEvent,
    ProductIndexRefreshEvent, RbacArtifactPermissionEvent, RbacRoleMutationEvent, ReactionsEvent,
    SocialGraphRelationEvent, TranslationWorkflowEvent, ValidateEvent,
};

pub(crate) mod sealed {
    pub trait Sealed {}
}

/// Closed platform contract for typed events accepted by durable transports.
///
/// Implementations live in `rustok-events`; domain modules cannot publish
/// arbitrary string event names or unregistered payloads.
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
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(tag = "family", content = "event")]
pub enum ContractEventPayload {
    #[serde(rename = "root")]
    Root(DomainEvent),
    #[serde(rename = "blog_comments_delegation_schedule_audit")]
    BlogCommentsDelegationScheduleAudit(BlogCommentsDelegationScheduleAuditEvent),
    #[serde(rename = "forum_mention")]
    ForumMention(ForumMentionEvent),
    #[serde(rename = "forum_search_projection")]
    ForumSearchProjection(ForumSearchProjectionEvent),
    #[serde(rename = "marketplace_listing")]
    MarketplaceListing(MarketplaceListingEvent),
    #[serde(rename = "marketplace_seller")]
    MarketplaceSeller(MarketplaceSellerEvent),
    #[serde(rename = "product_index_refresh")]
    ProductIndexRefresh(ProductIndexRefreshEvent),
    #[serde(rename = "rbac_artifact_permission")]
    RbacArtifactPermission(RbacArtifactPermissionEvent),
    #[serde(rename = "rbac_role_mutation")]
    RbacRoleMutation(RbacRoleMutationEvent),
    #[serde(rename = "reactions")]
    Reactions(ReactionsEvent),
    #[serde(rename = "social_graph_relation")]
    SocialGraphRelation(SocialGraphRelationEvent),
    #[serde(rename = "translation_workflow")]
    TranslationWorkflow(TranslationWorkflowEvent),
}

impl ContractEventPayload {
    fn event_type(&self) -> &'static str {
        match self {
            Self::Root(event) => event.event_type(),
            Self::BlogCommentsDelegationScheduleAudit(event) => event.event_type(),
            Self::ForumMention(event) => event.event_type(),
            Self::ForumSearchProjection(event) => event.event_type(),
            Self::MarketplaceListing(event) => event.event_type(),
            Self::MarketplaceSeller(event) => event.event_type(),
            Self::ProductIndexRefresh(event) => event.event_type(),
            Self::RbacArtifactPermission(event) => event.event_type(),
            Self::RbacRoleMutation(event) => event.event_type(),
            Self::Reactions(event) => event.event_type(),
            Self::SocialGraphRelation(event) => event.event_type(),
            Self::TranslationWorkflow(event) => event.event_type(),
        }
    }

    fn schema_version(&self) -> u16 {
        match self {
            Self::Root(event) => event.schema_version(),
            Self::BlogCommentsDelegationScheduleAudit(event) => event.schema_version(),
            Self::ForumMention(event) => event.schema_version(),
            Self::ForumSearchProjection(event) => event.schema_version(),
            Self::MarketplaceListing(event) => event.schema_version(),
            Self::MarketplaceSeller(event) => event.schema_version(),
            Self::ProductIndexRefresh(event) => event.schema_version(),
            Self::RbacArtifactPermission(event) => event.schema_version(),
            Self::RbacRoleMutation(event) => event.schema_version(),
            Self::Reactions(event) => event.schema_version(),
            Self::SocialGraphRelation(event) => event.schema_version(),
            Self::TranslationWorkflow(event) => event.schema_version(),
        }
    }
}

impl ValidateEvent for ContractEventPayload {
    fn validate(&self) -> Result<(), EventValidationError> {
        match self {
            Self::Root(event) => event.validate(),
            Self::BlogCommentsDelegationScheduleAudit(event) => event.validate(),
            Self::ForumMention(event) => event.validate(),
            Self::ForumSearchProjection(event) => event.validate(),
            Self::MarketplaceListing(event) => event.validate(),
            Self::MarketplaceSeller(event) => event.validate(),
            Self::ProductIndexRefresh(event) => event.validate(),
            Self::RbacArtifactPermission(event) => event.validate(),
            Self::RbacRoleMutation(event) => event.validate(),
            Self::Reactions(event) => event.validate(),
            Self::SocialGraphRelation(event) => event.validate(),
            Self::TranslationWorkflow(event) => event.validate(),
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
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct ContractEventEnvelope {
    id: Uuid,
    event_type: String,
    schema_version: u16,
    correlation_id: Uuid,
    causation_id: Option<Uuid>,
    tenant_id: Uuid,
    trace_id: Option<String>,
    #[serde(with = "crate::types::timestamp_serde")]
    #[schemars(with = "DateTime<Utc>")]
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
        let id = Uuid::from_bytes(Ulid::generate().to_bytes());
        Self::new_with_identity(id, tenant_id, actor_id, None, event)
    }

    /// Creates a typed envelope with one exact caller-owned durable identity.
    ///
    /// This constructor is reserved for transactional write-once boundaries
    /// whose existing owner idempotency identity must also be the canonical
    /// envelope UUID. The identity must be non-nil; correlation identity is set
    /// to the same UUID and the ordinary registered-envelope validation remains
    /// authoritative.
    pub fn new_with_envelope_id<E>(
        envelope_id: Uuid,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        event: E,
    ) -> Result<Self, EventContractEnvelopeError>
    where
        E: EventContract,
    {
        Self::new_with_identity(envelope_id, tenant_id, actor_id, None, event)
    }

    /// Creates a typed envelope with one exact caller-owned durable identity
    /// and one exact causal predecessor envelope.
    ///
    /// The supplied envelope UUID is also the correlation UUID. Causation stays
    /// in envelope metadata rather than the typed payload, so this constructor
    /// does not change any event-family schema or contract digest.
    pub fn new_with_envelope_id_and_causation<E>(
        envelope_id: Uuid,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        causation_id: Uuid,
        event: E,
    ) -> Result<Self, EventContractEnvelopeError>
    where
        E: EventContract,
    {
        Self::new_with_identity(envelope_id, tenant_id, actor_id, Some(causation_id), event)
    }

    /// Creates a typed envelope that is causally linked to one exact durable
    /// predecessor envelope.
    ///
    /// The causation identity is transport metadata, not payload data. A nil
    /// identity is rejected by the same registered-envelope validation used for
    /// decoded and relayed events.
    pub fn new_caused_by<E>(
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        causation_id: Uuid,
        event: E,
    ) -> Result<Self, EventContractEnvelopeError>
    where
        E: EventContract,
    {
        let id = Uuid::from_bytes(Ulid::generate().to_bytes());
        Self::new_with_identity(id, tenant_id, actor_id, Some(causation_id), event)
    }

    fn new_with_identity<E>(
        id: Uuid,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        causation_id: Option<Uuid>,
        event: E,
    ) -> Result<Self, EventContractEnvelopeError>
    where
        E: EventContract,
    {
        event.validate()?;
        let event_type = event.event_type().to_string();
        let schema_version = event.schema_version();
        let envelope = Self {
            id,
            event_type,
            schema_version,
            correlation_id: id,
            causation_id,
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

    pub fn correlation_id(&self) -> Uuid {
        self.correlation_id
    }

    pub fn causation_id(&self) -> Option<Uuid> {
        self.causation_id
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn actor_id(&self) -> Option<Uuid> {
        self.actor_id
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
        if self.tenant_id.is_nil()
            && !matches!(
                &self.event,
                ContractEventPayload::Root(event) if event.allows_platform_scope()
            )
        {
            return Err(EventValidationError::NilUuid("tenant_id").into());
        }
        if self
            .causation_id
            .is_some_and(|causation_id| causation_id.is_nil())
        {
            return Err(EventValidationError::NilUuid("causation_id").into());
        }
        if self.actor_id.is_some_and(|actor_id| actor_id.is_nil()) {
            return Err(EventValidationError::NilUuid("actor_id").into());
        }
        if let Some(trace_id) = &self.trace_id {
            if trace_id.trim().is_empty() {
                return Err(EventValidationError::EmptyField("trace_id").into());
            }
            if trace_id.len() > 512 {
                return Err(EventValidationError::FieldTooLong("trace_id", 512).into());
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn mention_event() -> ForumMentionEvent {
        ForumMentionEvent::UserMentionAdded {
            source_kind: "topic".to_string(),
            source_id: Uuid::new_v4(),
            source_revision_id: 1,
            source_locale: "en".to_string(),
            mentioned_user_id: Uuid::new_v4(),
        }
    }

    #[test]
    fn caused_contract_envelope_retains_exact_causation_identity() {
        let causation_id = Uuid::new_v4();
        let envelope = ContractEventEnvelope::new_caused_by(
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
            causation_id,
            mention_event(),
        )
        .expect("caused contract envelope should validate");

        assert_eq!(envelope.causation_id(), Some(causation_id));
        envelope
            .validate_registered_schema()
            .expect("caused contract envelope should remain registered");
    }

    #[test]
    fn caused_contract_envelope_rejects_nil_causation_identity() {
        let error = ContractEventEnvelope::new_caused_by(
            Uuid::new_v4(),
            None,
            Uuid::nil(),
            mention_event(),
        )
        .expect_err("nil causation identity must fail closed");

        assert!(matches!(
            error,
            EventContractEnvelopeError::Validation(EventValidationError::NilUuid("causation_id"))
        ));
    }

    #[test]
    fn explicit_contract_envelope_identity_is_exact_and_correlated() {
        let envelope_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let envelope = ContractEventEnvelope::new_with_envelope_id(
            envelope_id,
            Uuid::new_v4(),
            Some(actor_id),
            mention_event(),
        )
        .expect("explicit envelope identity should validate");

        assert_eq!(envelope.id(), envelope_id);
        assert_eq!(envelope.correlation_id(), envelope_id);
        assert_eq!(envelope.actor_id(), Some(actor_id));
        assert_eq!(envelope.causation_id(), None);
    }

    #[test]
    fn explicit_contract_envelope_identity_and_causation_are_exact() {
        let envelope_id = Uuid::new_v4();
        let causation_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let envelope = ContractEventEnvelope::new_with_envelope_id_and_causation(
            envelope_id,
            Uuid::new_v4(),
            Some(actor_id),
            causation_id,
            mention_event(),
        )
        .expect("explicit caused envelope should validate");

        assert_eq!(envelope.id(), envelope_id);
        assert_eq!(envelope.correlation_id(), envelope_id);
        assert_eq!(envelope.actor_id(), Some(actor_id));
        assert_eq!(envelope.causation_id(), Some(causation_id));
    }

    #[test]
    fn explicit_contract_envelope_identity_rejects_nil_uuid() {
        let error = ContractEventEnvelope::new_with_envelope_id(
            Uuid::nil(),
            Uuid::new_v4(),
            None,
            mention_event(),
        )
        .expect_err("nil explicit envelope identity must fail closed");

        assert!(matches!(
            error,
            EventContractEnvelopeError::Validation(EventValidationError::NilUuid("id"))
        ));
    }

    #[test]
    fn platform_scoped_root_event_accepts_the_root_tenant_sentinel() {
        let envelope = ContractEventEnvelope::new(
            Uuid::nil(),
            None,
            DomainEvent::ModuleArtifactAdmitted {
                installation_id: Uuid::new_v4(),
                artifact_digest: "sha256:artifact".to_string(),
                media_type: "application/vnd.rustok.wasm.component.v1+wasm".to_string(),
                size_bytes: 1,
            },
        )
        .expect("platform-scoped root event should validate");

        envelope
            .validate_registered_schema()
            .expect("platform-scoped root event should remain valid");
    }

    #[test]
    fn tenant_scoped_root_event_rejects_the_root_tenant_sentinel() {
        let error = ContractEventEnvelope::new(
            Uuid::nil(),
            None,
            DomainEvent::ModuleBuildQueued {
                request_id: Uuid::new_v4(),
                tenant_id: Uuid::new_v4(),
                project_id: "module-build-test".to_string(),
                attempt: 1,
            },
        )
        .expect_err("tenant-scoped root event must fail closed");

        assert!(matches!(
            error,
            EventContractEnvelopeError::Validation(EventValidationError::NilUuid("tenant_id"))
        ));
    }

    #[test]
    fn explicit_contract_envelope_identity_rejects_nil_causation_uuid() {
        let error = ContractEventEnvelope::new_with_envelope_id_and_causation(
            Uuid::new_v4(),
            Uuid::new_v4(),
            None,
            Uuid::nil(),
            mention_event(),
        )
        .expect_err("nil explicit causation identity must fail closed");

        assert!(matches!(
            error,
            EventContractEnvelopeError::Validation(EventValidationError::NilUuid("causation_id"))
        ));
    }
}
