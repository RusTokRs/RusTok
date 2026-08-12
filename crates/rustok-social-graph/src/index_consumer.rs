use std::sync::Arc;
use std::time::Duration;

use rustok_events::{ContractEventEnvelope, ContractEventPayload, EventContractEnvelopeError};
use rustok_iggy::{
    ConsumedContractDecodeFailure, ConsumedContractEvent, DlqEntry, IggyTransport,
    PersistentContractConsumerGroup, PersistentContractDelivery,
};
use rustok_index::{
    IndexSchema, MutationApplyOutcome, MutationDelivery, MutationStorageError,
    PostgresMutationStore, PostgresSchemaRegistrationStore, SchemaRegistrationError,
    SchemaRegistry, SchemaRegistryError,
};
use sea_orm::DatabaseConnection;
use thiserror::Error;
use uuid::Uuid;

use crate::index::{
    SocialGraphIndexError, social_graph_relation_index_mutation, social_graph_relation_index_schema,
};
use crate::index_dlq_message_id::social_graph_index_dlq_broker_message_id;
use crate::index_dlq_receipt::{
    SocialGraphIndexDlqIdentity, SocialGraphIndexDlqPublishClaim, SocialGraphIndexDlqReceipt,
    SocialGraphIndexDlqReceiptError, SocialGraphIndexDlqReceiptState,
    SocialGraphIndexDlqReceiptStore,
};

/// The existing shared domain topic currently carries sealed Social Graph relation facts.
pub const SOCIAL_GRAPH_INDEX_TOPIC: &str = "domain";
/// Dedicated durable cursor identity for the Social Graph relation projection.
pub const SOCIAL_GRAPH_INDEX_CONSUMER_GROUP: &str = "rustok-social-graph-index";
/// Stable Index inbox source identity for relation-event deliveries.
pub const SOCIAL_GRAPH_INDEX_SOURCE: &str = "social_graph.relation.state_changed.v1";
/// Stable bounded outcome code used when a durable DLQ receipt is recovered on redelivery.
pub const SOCIAL_GRAPH_INDEX_DLQ_RECEIPT_RECOVERED_CODE: &str =
    "social_graph.index.dlq_receipt_recovered";
/// A crashed publisher may be reclaimed after this bounded durable lease.
pub const SOCIAL_GRAPH_INDEX_DLQ_PUBLISH_LEASE: Duration = Duration::from_secs(30);

#[derive(Debug, Error)]
pub enum SocialGraphIndexConsumerError {
    #[error("Social Graph Index transport operation failed: {0}")]
    Transport(String),
    #[error("Social Graph Index DLQ publication is owned by another live publisher")]
    DlqPublishInProgress,
    #[error(transparent)]
    DlqReceipt(#[from] SocialGraphIndexDlqReceiptError),
    #[error(transparent)]
    Envelope(#[from] EventContractEnvelopeError),
    #[error(transparent)]
    Projection(#[from] SocialGraphIndexError),
    #[error(transparent)]
    Registry(#[from] SchemaRegistryError),
    #[error(transparent)]
    SchemaPersistence(#[from] SchemaRegistrationError),
    #[error(transparent)]
    Storage(#[from] MutationStorageError),
}

impl SocialGraphIndexConsumerError {
    /// Stable bounded code suitable for retry/DLQ telemetry without storage details.
    pub const fn stable_code(&self) -> &'static str {
        match self {
            Self::Transport(_) => "social_graph.index.transport_unavailable",
            Self::DlqPublishInProgress => "social_graph.index.dlq_publish_in_progress",
            Self::DlqReceipt(error) => error.stable_code(),
            Self::Envelope(_) => "social_graph.index.envelope_invalid",
            Self::Projection(_) => "social_graph.index.projection_invalid",
            Self::Registry(_) => "social_graph.index.registry_invalid",
            Self::SchemaPersistence(_) => "social_graph.index.schema_persistence_failed",
            Self::Storage(_) => "social_graph.index.mutation_persistence_failed",
        }
    }

    /// Only transient transport/storage ownership failures are retried in-process.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Transport(_) | Self::DlqPublishInProgress => true,
            Self::DlqReceipt(error) => error.is_retryable(),
            Self::Envelope(_) | Self::Projection(_) | Self::Registry(_) => false,
            Self::SchemaPersistence(error) => {
                matches!(error, SchemaRegistrationError::Storage(_))
            }
            Self::Storage(error) => matches!(
                error,
                MutationStorageError::DeliveryInProgress { .. }
                    | MutationStorageError::Storage(_)
                    | MutationStorageError::ConcurrentMutationConflict
                    | MutationStorageError::InboxCompletionLost
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocialGraphIndexProcessOutcome {
    Projected(MutationApplyOutcome),
    IgnoredUnrelated { event_type: String },
    DeadLettered { error_code: &'static str },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocialGraphIndexDlqPublishOutcome {
    Published,
    PreviouslyPublished,
}

/// Converts a validated sealed envelope into one durable Index inbox delivery.
pub fn social_graph_index_delivery_from_envelope(
    envelope: &ContractEventEnvelope,
) -> Result<Option<MutationDelivery>, SocialGraphIndexConsumerError> {
    let payload = envelope.payload()?;
    let ContractEventPayload::SocialGraphRelation(event) = payload else {
        return Ok(None);
    };
    let mutation =
        social_graph_relation_index_mutation(envelope.tenant_id(), envelope.id(), event.clone())?;
    Ok(Some(MutationDelivery::from_event(
        SOCIAL_GRAPH_INDEX_SOURCE,
        mutation,
    )?))
}

/// Transport-neutral durable projector used by live and replay-oriented consumers.
pub struct SocialGraphIndexProjector {
    schema: IndexSchema,
    schema_store: PostgresSchemaRegistrationStore,
    store: PostgresMutationStore,
    registry: SchemaRegistry,
}

impl SocialGraphIndexProjector {
    pub fn new(db: DatabaseConnection) -> Result<Self, SocialGraphIndexConsumerError> {
        let schema = social_graph_relation_index_schema()?;
        let mut registry = SchemaRegistry::new();
        registry.register(schema.clone())?;
        Ok(Self {
            schema,
            schema_store: PostgresSchemaRegistrationStore::new(db.clone()),
            store: PostgresMutationStore::new(db),
            registry,
        })
    }

    pub fn registry(&self) -> &SchemaRegistry {
        &self.registry
    }

    pub async fn apply_envelope(
        &self,
        envelope: &ContractEventEnvelope,
    ) -> Result<SocialGraphIndexProcessOutcome, SocialGraphIndexConsumerError> {
        let Some(delivery) = social_graph_index_delivery_from_envelope(envelope)? else {
            return Ok(SocialGraphIndexProcessOutcome::IgnoredUnrelated {
                event_type: envelope.event_type().to_string(),
            });
        };
        self.schema_store
            .register(envelope.tenant_id(), &self.schema)
            .await?;
        let outcome = self.store.apply(&self.registry, &delivery).await?;
        Ok(SocialGraphIndexProcessOutcome::Projected(outcome))
    }
}

/// Result-first durable consumer for `social_graph.relation.state_changed`.
///
/// The consumer owns one persistent broker cursor, one transport-neutral projector,
/// and one owner-side DLQ receipt store. A durable DLQ receipt is checked before
/// projection so a redelivery cannot cross from an already chosen DLQ terminal result
/// back into Index mutation work.
pub struct SocialGraphIndexConsumer {
    transport: Arc<IggyTransport>,
    group: PersistentContractConsumerGroup,
    projector: SocialGraphIndexProjector,
    dlq_receipts: SocialGraphIndexDlqReceiptStore,
    dlq_publisher_id: Uuid,
}

impl SocialGraphIndexConsumer {
    pub async fn open(
        transport: Arc<IggyTransport>,
        db: DatabaseConnection,
    ) -> Result<Self, SocialGraphIndexConsumerError> {
        let group = transport
            .open_persistent_contract_consumer_group(
                SOCIAL_GRAPH_INDEX_CONSUMER_GROUP,
                SOCIAL_GRAPH_INDEX_TOPIC,
            )
            .await
            .map_err(|error| SocialGraphIndexConsumerError::Transport(error.to_string()))?;
        Ok(Self {
            transport,
            group,
            projector: SocialGraphIndexProjector::new(db.clone())?,
            dlq_receipts: SocialGraphIndexDlqReceiptStore::new(db),
            dlq_publisher_id: Uuid::new_v4(),
        })
    }

    pub fn projector(&self) -> &SocialGraphIndexProjector {
        &self.projector
    }

    /// Receives either a validated contract event or an exact-byte decode failure.
    /// Neither variant is acknowledged by this operation.
    pub async fn receive_delivery(
        &self,
    ) -> Result<Option<PersistentContractDelivery>, SocialGraphIndexConsumerError> {
        self.group
            .receive_delivery()
            .await
            .map_err(|error| SocialGraphIndexConsumerError::Transport(error.to_string()))
    }

    /// Compatibility path retained for callers that only accept validated events.
    pub async fn receive_next(
        &self,
    ) -> Result<Option<ConsumedContractEvent>, SocialGraphIndexConsumerError> {
        match self.receive_delivery().await? {
            Some(PersistentContractDelivery::Event(consumed)) => Ok(Some(*consumed)),
            Some(PersistentContractDelivery::DecodeFailure(failure)) => {
                Err(SocialGraphIndexConsumerError::Transport(format!(
                    "contract delivery requires raw poison handling [{}]",
                    failure.stable_error_code()
                )))
            }
            None => Ok(None),
        }
    }

    /// Persists or recognizes the terminal owner result without acknowledging.
    ///
    /// Any existing DLQ receipt wins before projection. `published` and `acknowledged`
    /// are terminal; `reserved` and `publishing` remain retryable DLQ work and must not
    /// re-enter Index mutation processing.
    pub async fn project_consumed(
        &self,
        consumed: &ConsumedContractEvent,
    ) -> Result<SocialGraphIndexProcessOutcome, SocialGraphIndexConsumerError> {
        if let Some(receipt) = self.consumed_dlq_receipt(consumed).await? {
            return match receipt.state {
                SocialGraphIndexDlqReceiptState::Published
                | SocialGraphIndexDlqReceiptState::Acknowledged => {
                    Ok(SocialGraphIndexProcessOutcome::DeadLettered {
                        error_code: SOCIAL_GRAPH_INDEX_DLQ_RECEIPT_RECOVERED_CODE,
                    })
                }
                SocialGraphIndexDlqReceiptState::Reserved
                | SocialGraphIndexDlqReceiptState::Publishing => {
                    Err(SocialGraphIndexConsumerError::DlqPublishInProgress)
                }
            };
        }
        self.projector.apply_envelope(&consumed.envelope).await
    }

    /// Commits the exact broker offset after a durable decoded-event result exists.
    ///
    /// Receipt acknowledgement is best-effort bookkeeping after the broker commit.
    /// Failure to update it cannot turn a committed source offset back into a failure.
    pub async fn acknowledge_consumed(
        &self,
        consumed: &ConsumedContractEvent,
    ) -> Result<(), SocialGraphIndexConsumerError> {
        self.group
            .acknowledge(consumed)
            .await
            .map_err(|error| SocialGraphIndexConsumerError::Transport(error.to_string()))?;
        if let Ok(identity) = self.dlq_identity(consumed)
            && let Ok(Some(receipt)) = self.dlq_receipts.find(&identity).await
            && matches!(
                receipt.state,
                SocialGraphIndexDlqReceiptState::Published
                    | SocialGraphIndexDlqReceiptState::Acknowledged
            )
            && let Err(error) = self.dlq_receipts.mark_acknowledged(&identity).await
        {
            tracing::warn!(
                event_id = %consumed.envelope.id(),
                error_code = error.stable_code(),
                "Source offset committed but DLQ receipt acknowledgement bookkeeping failed"
            );
        }
        Ok(())
    }

    /// Commits the exact broker offset for an undecodable delivery only after the
    /// worker has durably established its neutral poison result.
    pub async fn acknowledge_decode_failure(
        &self,
        consumed: &ConsumedContractDecodeFailure,
    ) -> Result<(), SocialGraphIndexConsumerError> {
        self.group
            .acknowledge_decode_failure(consumed)
            .await
            .map_err(|error| SocialGraphIndexConsumerError::Transport(error.to_string()))
    }

    pub async fn consumed_dlq_receipt(
        &self,
        consumed: &ConsumedContractEvent,
    ) -> Result<Option<SocialGraphIndexDlqReceipt>, SocialGraphIndexConsumerError> {
        let identity = self.dlq_identity(consumed)?;
        self.dlq_receipts.find(&identity).await.map_err(Into::into)
    }

    /// Publishes exact broker bytes only after durably reserving the source identity.
    ///
    /// `Ok` is returned only after the receipt reached `published`, so a later source-ack
    /// failure or process restart recognizes the terminal DLQ result and skips publication.
    /// A crash after broker success but before `published` may retry the same deterministic
    /// broker message ID after the lease expires. Physical duplicate suppression additionally
    /// requires an enabled Iggy deduplication window that still contains that ID.
    pub async fn publish_consumed_to_dlq(
        &self,
        consumed: &ConsumedContractEvent,
        stable_error_code: &str,
        retry_count: u32,
    ) -> Result<SocialGraphIndexDlqPublishOutcome, SocialGraphIndexConsumerError> {
        let identity = self.dlq_identity(consumed)?;
        let existing = self.dlq_receipts.find(&identity).await?;
        let effective_error_code = existing.as_ref().map_or(stable_error_code, |receipt| {
            receipt.stable_error_code.as_str()
        });
        let effective_retry_count = existing
            .as_ref()
            .map_or(retry_count, |receipt| receipt.projection_attempt_count);

        match self
            .dlq_receipts
            .reserve_and_claim(
                &identity,
                effective_error_code,
                effective_retry_count,
                self.dlq_publisher_id,
                SOCIAL_GRAPH_INDEX_DLQ_PUBLISH_LEASE,
            )
            .await?
        {
            SocialGraphIndexDlqPublishClaim::AlreadyPublished
            | SocialGraphIndexDlqPublishClaim::AlreadyAcknowledged => {
                return Ok(SocialGraphIndexDlqPublishOutcome::PreviouslyPublished);
            }
            SocialGraphIndexDlqPublishClaim::Busy => {
                return Err(SocialGraphIndexConsumerError::DlqPublishInProgress);
            }
            SocialGraphIndexDlqPublishClaim::Claimed => {}
        }

        let broker_message_id = social_graph_index_dlq_broker_message_id(&identity);
        let entry = DlqEntry::new(
            consumed.envelope.id(),
            consumed.topic.clone(),
            consumed.raw_payload().to_vec(),
            effective_error_code,
            effective_retry_count,
        )
        .with_connector_metadata(consumed.connector_metadata.clone())
        .with_broker_message_id(broker_message_id);
        if let Err(error) = self.transport.move_to_dlq(entry).await {
            let _ = self
                .dlq_receipts
                .release_claim(&identity, self.dlq_publisher_id)
                .await;
            return Err(SocialGraphIndexConsumerError::Transport(error.to_string()));
        }
        self.dlq_receipts
            .mark_published(&identity, self.dlq_publisher_id)
            .await?;
        Ok(SocialGraphIndexDlqPublishOutcome::Published)
    }

    pub async fn mark_consumed_dlq_acknowledged(
        &self,
        consumed: &ConsumedContractEvent,
    ) -> Result<(), SocialGraphIndexConsumerError> {
        let identity = self.dlq_identity(consumed)?;
        self.dlq_receipts
            .mark_acknowledged(&identity)
            .await
            .map_err(Into::into)
    }

    pub async fn move_to_dlq_and_acknowledge(
        &self,
        consumed: &ConsumedContractEvent,
        stable_error_code: &str,
        retry_count: u32,
    ) -> Result<(), SocialGraphIndexConsumerError> {
        self.publish_consumed_to_dlq(consumed, stable_error_code, retry_count)
            .await?;
        self.acknowledge_consumed(consumed).await
    }

    pub async fn process_next(
        &self,
    ) -> Result<Option<SocialGraphIndexProcessOutcome>, SocialGraphIndexConsumerError> {
        let Some(consumed) = self.receive_next().await? else {
            return Ok(None);
        };
        let outcome = self.project_consumed(&consumed).await?;
        self.acknowledge_consumed(&consumed).await?;
        Ok(Some(outcome))
    }

    fn dlq_identity(
        &self,
        consumed: &ConsumedContractEvent,
    ) -> Result<SocialGraphIndexDlqIdentity, SocialGraphIndexConsumerError> {
        consumed
            .validate_connector_metadata()
            .map_err(|error| SocialGraphIndexConsumerError::Transport(error.to_string()))?;
        let source_offset =
            consumed
                .offset()
                .ok_or(SocialGraphIndexDlqReceiptError::InvalidIdentity {
                    field: "source_offset",
                    reason: "connector metadata did not provide an offset",
                })?;
        SocialGraphIndexDlqIdentity::new(
            consumed.envelope.tenant_id(),
            consumed.envelope.id(),
            SOCIAL_GRAPH_INDEX_CONSUMER_GROUP,
            consumed.stream.clone(),
            consumed.topic.clone(),
            consumed.partition,
            source_offset,
            consumed.raw_payload().to_vec(),
        )
        .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use rustok_core::MigrationSource;
    use rustok_events::{ContractEventEnvelope, DomainEvent, SocialGraphRelationEvent};
    use rustok_index::{IndexModule, IndexMutation};
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
    use sea_orm_migration::SchemaManager;
    use uuid::Uuid;

    use super::*;

    fn relation_event(
        tenant_id: Uuid,
        relation_id: Uuid,
        active: bool,
        revision: i64,
    ) -> ContractEventEnvelope {
        ContractEventEnvelope::new(
            tenant_id,
            None,
            SocialGraphRelationEvent::RelationStateChanged {
                relation_id,
                source_user_id: Uuid::from_u128(3),
                target_user_id: Uuid::from_u128(4),
                relation_kind: "follow".to_string(),
                active,
                revision,
            },
        )
        .unwrap()
    }

    async fn projector_fixture() -> (DatabaseConnection, SocialGraphIndexProjector, Uuid) {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("PRAGMA foreign_keys = ON")
            .await
            .unwrap();
        db.execute_unprepared("CREATE TABLE tenants (id TEXT PRIMARY KEY)")
            .await
            .unwrap();
        let tenant_id = Uuid::from_u128(1);
        db.execute_unprepared(&format!("INSERT INTO tenants (id) VALUES ('{tenant_id}')"))
            .await
            .unwrap();
        let manager = SchemaManager::new(&db);
        for migration in IndexModule.migrations() {
            migration.up(&manager).await.unwrap();
        }
        let projector = SocialGraphIndexProjector::new(db.clone()).unwrap();
        (db, projector, tenant_id)
    }

    async fn scalar_i64(db: &DatabaseConnection, sql: &str) -> i64 {
        db.query_one(Statement::from_string(DbBackend::Sqlite, sql.to_owned()))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "value")
            .unwrap()
    }

    #[test]
    fn relation_envelope_becomes_stable_index_delivery() {
        let tenant_id = Uuid::from_u128(1);
        let envelope = relation_event(tenant_id, Uuid::from_u128(2), true, 9);
        let delivery = social_graph_index_delivery_from_envelope(&envelope)
            .unwrap()
            .expect("relation event must produce an Index delivery");
        assert_eq!(delivery.source_name(), SOCIAL_GRAPH_INDEX_SOURCE);
        assert_eq!(delivery.delivery_id(), envelope.id().to_string());
        let IndexMutation::Upsert { record, .. } = delivery.mutation() else {
            panic!("active relation must produce an upsert");
        };
        assert_eq!(record.key.tenant_id, tenant_id);
        assert_eq!(record.key.entity_id, Uuid::from_u128(2));
        assert_eq!(record.source_version, 9);
    }

    #[test]
    fn unrelated_domain_event_is_ignored_without_projection() {
        let envelope = ContractEventEnvelope::new(
            Uuid::from_u128(1),
            None,
            DomainEvent::NodeCreated {
                node_id: Uuid::from_u128(2),
                kind: "post".to_string(),
                author_id: None,
            },
        )
        .unwrap();
        assert!(
            social_graph_index_delivery_from_envelope(&envelope)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn retry_classification_is_fail_closed() {
        assert!(SocialGraphIndexConsumerError::Transport("down".to_string()).is_retryable());
        assert!(SocialGraphIndexConsumerError::DlqPublishInProgress.is_retryable());
        assert!(
            SocialGraphIndexConsumerError::SchemaPersistence(SchemaRegistrationError::Storage(
                "down".to_string()
            ))
            .is_retryable()
        );
        assert!(
            !SocialGraphIndexConsumerError::SchemaPersistence(SchemaRegistrationError::NilTenantId)
                .is_retryable()
        );
        assert!(
            !SocialGraphIndexConsumerError::Storage(MutationStorageError::DeliveryConflict)
                .is_retryable()
        );
    }

    #[tokio::test]
    async fn projector_persists_schema_before_result_first_mutation_apply() {
        let (db, projector, tenant_id) = projector_fixture().await;
        let relation_id = Uuid::from_u128(2);
        let first = relation_event(tenant_id, relation_id, true, 1);
        assert_eq!(
            projector.apply_envelope(&first).await.unwrap(),
            SocialGraphIndexProcessOutcome::Projected(MutationApplyOutcome::Applied {
                source_version: 1
            })
        );
        assert_eq!(
            projector.apply_envelope(&first).await.unwrap(),
            SocialGraphIndexProcessOutcome::Projected(MutationApplyOutcome::Duplicate {
                source_version: 1
            })
        );
        let second = relation_event(tenant_id, relation_id, false, 2);
        assert_eq!(
            projector.apply_envelope(&second).await.unwrap(),
            SocialGraphIndexProcessOutcome::Projected(MutationApplyOutcome::Applied {
                source_version: 2
            })
        );
        assert_eq!(
            scalar_i64(&db, "SELECT COUNT(*) AS value FROM index_schemas").await,
            1
        );
        assert_eq!(
            scalar_i64(
                &db,
                "SELECT COUNT(*) AS value FROM index_entities WHERE is_deleted = TRUE AND source_version = 2"
            )
            .await,
            1
        );
    }
}
