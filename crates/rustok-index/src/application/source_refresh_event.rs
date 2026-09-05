use thiserror::Error;
use uuid::Uuid;

use crate::{EntityKey, IndexMutation, SchemaRef, SchemaRegistry};

use super::{
    IndexMutationAcknowledgeFailure, IndexMutationEventAcknowledger, IndexReplayFailure,
    IndexReplayMutationOutcome, IndexReplayMutationSink, IndexSourceError, IndexSourceLoadRequest,
    SharedIndexMutationEventRegistry, SharedIndexSourceRegistry,
};

const MAX_EVENT_DOMAIN_BYTES: usize = 128;

/// One broker delivery that requests an exact authoritative source refresh.
///
/// The event carries no copied owner payload. The registered source remains authoritative for the
/// current upsert or tombstone mutation. `minimum_source_version` fences replica lag or an owner
/// publication bug: acknowledgement is withheld until the source exposes at least the revision
/// committed by the owner event.
pub struct IndexSourceRefreshEventDelivery<T> {
    event_domain: String,
    event_id: Uuid,
    key: EntityKey,
    minimum_source_version: u64,
    acknowledgement_token: T,
}

impl<T> IndexSourceRefreshEventDelivery<T> {
    pub fn new(
        event_domain: impl Into<String>,
        event_id: Uuid,
        key: EntityKey,
        minimum_source_version: u64,
        acknowledgement_token: T,
    ) -> Result<Self, IndexSourceRefreshEventError> {
        let event_domain = event_domain.into();
        if !valid_machine_name(&event_domain, MAX_EVENT_DOMAIN_BYTES) {
            return Err(IndexSourceRefreshEventError::InvalidEventDomain(
                event_domain,
            ));
        }
        if event_id.is_nil() {
            return Err(IndexSourceRefreshEventError::NilEventId);
        }
        if key.tenant_id.is_nil() {
            return Err(IndexSourceRefreshEventError::NilTenantId);
        }
        if key.entity_id.is_nil() {
            return Err(IndexSourceRefreshEventError::NilEntityId);
        }
        if minimum_source_version == 0 {
            return Err(IndexSourceRefreshEventError::ZeroMinimumSourceVersion);
        }
        Ok(Self {
            event_domain,
            event_id,
            key,
            minimum_source_version,
            acknowledgement_token,
        })
    }

    pub fn event_domain(&self) -> &str {
        &self.event_domain
    }

    pub fn event_id(&self) -> Uuid {
        self.event_id
    }

    pub fn key(&self) -> &EntityKey {
        &self.key
    }

    pub fn minimum_source_version(&self) -> u64 {
        self.minimum_source_version
    }

    pub fn acknowledgement_token(&self) -> &T {
        &self.acknowledgement_token
    }

    pub fn into_parts(self) -> (String, Uuid, EntityKey, u64, T) {
        (
            self.event_domain,
            self.event_id,
            self.key,
            self.minimum_source_version,
            self.acknowledgement_token,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexSourceRefreshEventProcessOutcome {
    event_id: Uuid,
    source_name: String,
    source_version: u64,
    mutation_outcome: IndexReplayMutationOutcome,
}

impl IndexSourceRefreshEventProcessOutcome {
    pub fn event_id(&self) -> Uuid {
        self.event_id
    }

    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    pub fn source_version(&self) -> u64 {
        self.source_version
    }

    pub fn mutation_outcome(&self) -> IndexReplayMutationOutcome {
        self.mutation_outcome
    }
}

/// Source-driven commit-before-ack orchestration for owner change notifications.
///
/// The exact event route chooses one schema/source, the delivery chooses one entity key and minimum
/// owner revision, and the immutable source registry supplies the canonical mutation. The broker
/// event UUID replaces the replay-only mutation UUID so redelivery is deduplicated by the durable
/// inbox. A newer source revision is accepted; an older or missing source result is not.
pub struct IndexSourceRefreshEventWorker<M, A> {
    mutation_sink: M,
    acknowledger: A,
}

impl<M, A> IndexSourceRefreshEventWorker<M, A>
where
    M: IndexReplayMutationSink,
    A: IndexMutationEventAcknowledger,
{
    pub fn new(mutation_sink: M, acknowledger: A) -> Self {
        Self {
            mutation_sink,
            acknowledger,
        }
    }

    pub async fn process(
        &self,
        schema_registry: &SchemaRegistry,
        source_registry: &SharedIndexSourceRegistry,
        event_registry: &SharedIndexMutationEventRegistry,
        delivery: IndexSourceRefreshEventDelivery<A::Token>,
    ) -> Result<IndexSourceRefreshEventProcessOutcome, IndexSourceRefreshEventProcessError> {
        let (event_domain, event_id, key, minimum_source_version, acknowledgement_token) =
            delivery.into_parts();
        let descriptor = event_registry.get(&event_domain).ok_or_else(|| {
            IndexSourceRefreshEventProcessError::UnknownEventDomain(event_domain.clone())
        })?;
        let expected_schema = descriptor.schema().clone();
        let expected_source_name = descriptor.source_name().to_owned();
        if key.schema != expected_schema {
            return Err(IndexSourceRefreshEventProcessError::EventSchemaMismatch {
                event_domain,
                expected: expected_schema,
                actual: key.schema.clone(),
            });
        }

        let source = source_registry.source_for_schema(&key.schema).ok_or_else(|| {
            IndexSourceRefreshEventProcessError::MissingReplaySource {
                schema: key.schema.clone(),
            }
        })?;
        if source.source_name() != expected_source_name {
            return Err(IndexSourceRefreshEventProcessError::ReplaySourceMismatch {
                event_domain,
                expected: expected_source_name,
                actual: source.source_name().to_owned(),
            });
        }

        let request = IndexSourceLoadRequest::new(vec![key.clone()])
            .map_err(IndexSourceRefreshEventProcessError::Source)?;
        let mut mutations = source_registry
            .load(request)
            .await
            .map_err(IndexSourceRefreshEventProcessError::Source)?
            .into_mutations();
        let mutation = match mutations.len() {
            0 => {
                return Err(IndexSourceRefreshEventProcessError::MissingSourceMutation {
                    event_domain,
                    key,
                });
            }
            1 => mutations.pop().expect("single source mutation"),
            actual => {
                return Err(
                    IndexSourceRefreshEventProcessError::AmbiguousSourceMutation {
                        event_domain,
                        actual,
                    },
                );
            }
        };
        let source_version = mutation.source_version();
        if source_version < minimum_source_version {
            return Err(IndexSourceRefreshEventProcessError::SourceVersionBehind {
                event_domain,
                minimum: minimum_source_version,
                actual: source_version,
            });
        }

        let mutation = rebind_event_id(mutation, event_id);
        let mutation_outcome = self
            .mutation_sink
            .apply_replay_mutation(schema_registry, &expected_source_name, &mutation)
            .await
            .map_err(IndexSourceRefreshEventProcessError::Mutation)?;

        self.acknowledger
            .acknowledge(&acknowledgement_token)
            .await
            .map_err(IndexSourceRefreshEventProcessError::Acknowledge)?;

        Ok(IndexSourceRefreshEventProcessOutcome {
            event_id,
            source_name: expected_source_name,
            source_version,
            mutation_outcome,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IndexSourceRefreshEventError {
    #[error("Index source refresh event domain is invalid: {0}")]
    InvalidEventDomain(String),
    #[error("Index source refresh event UUID cannot be nil")]
    NilEventId,
    #[error("Index source refresh event tenant UUID cannot be nil")]
    NilTenantId,
    #[error("Index source refresh event entity UUID cannot be nil")]
    NilEntityId,
    #[error("Index source refresh event minimum source version must be positive")]
    ZeroMinimumSourceVersion,
}

#[derive(Debug, Error)]
pub enum IndexSourceRefreshEventProcessError {
    #[error("Unknown Index source refresh event domain: {0}")]
    UnknownEventDomain(String),
    #[error(
        "Index source refresh event {event_domain} carries schema {actual}, expected {expected}"
    )]
    EventSchemaMismatch {
        event_domain: String,
        expected: SchemaRef,
        actual: SchemaRef,
    },
    #[error("Index source refresh event has no replay source for schema {schema}")]
    MissingReplaySource { schema: SchemaRef },
    #[error(
        "Index source refresh event {event_domain} resolved replay source {actual}, expected {expected}"
    )]
    ReplaySourceMismatch {
        event_domain: String,
        expected: String,
        actual: String,
    },
    #[error("Index source refresh event source contract failed")]
    Source(#[source] IndexSourceError),
    #[error("Index source refresh event {event_domain} returned no mutation for {key:?}")]
    MissingSourceMutation {
        event_domain: String,
        key: EntityKey,
    },
    #[error("Index source refresh event {event_domain} returned {actual} mutations for one key")]
    AmbiguousSourceMutation { event_domain: String, actual: usize },
    #[error(
        "Index source refresh event {event_domain} source version is behind: minimum={minimum}, actual={actual}"
    )]
    SourceVersionBehind {
        event_domain: String,
        minimum: u64,
        actual: u64,
    },
    #[error("Index source refresh event persistence failed")]
    Mutation(#[source] IndexReplayFailure),
    #[error("Index source refresh event acknowledgement failed after durable persistence")]
    Acknowledge(#[source] IndexMutationAcknowledgeFailure),
}

fn rebind_event_id(mutation: IndexMutation, event_id: Uuid) -> IndexMutation {
    match mutation {
        IndexMutation::Upsert { record, .. } => IndexMutation::Upsert { event_id, record },
        IndexMutation::Delete {
            key,
            source_version,
            ..
        } => IndexMutation::Delete {
            event_id,
            key,
            source_version,
        },
    }
}

fn valid_machine_name(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        })
}
