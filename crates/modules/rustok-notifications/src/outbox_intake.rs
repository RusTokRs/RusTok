use std::sync::Arc;

use crate::NotificationSourceEventRef;
use chrono::{DateTime, FixedOffset, Utc};
use sea_orm::{
    ActiveValue::Set, ColumnTrait, Condition, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder, QuerySelect, QueryTrait,
    TransactionTrait, Value, sea_query::{LockBehavior, LockType, OnConflict},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::source_inbox;
use crate::error::{NotificationError, NotificationResult};
use crate::model::NotificationSourceInboxStatus;

pub const DEFAULT_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE: usize = 32;
pub const MAX_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE: usize = 64;

mod outbox_event {
    use chrono::{DateTime, Utc};
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "sys_events")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub event_type: String,
        pub schema_version: i16,
        pub payload: Json,
        pub created_at: DateTime<Utc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

mod intake_receipt {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "notification_outbox_intake_receipts")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub outbox_event_id: Uuid,
        pub tenant_id: Uuid,
        pub event_type: String,
        pub source_slug: String,
        pub source_event_id: Uuid,
        pub source_revision: i64,
        pub source_inbox_id: Uuid,
        pub created_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

mod intake_rejection {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "notification_outbox_intake_rejections")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub outbox_event_id: Uuid,
        pub event_type: String,
        pub schema_version: i16,
        pub error_code: String,
        pub created_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NotificationOutboxEnvelopeRecord {
    pub outbox_event_id: Uuid,
    pub event_type: String,
    pub schema_version: i16,
    pub payload: serde_json::Value,
}

pub trait NotificationOutboxEnvelopeDecoder: Send + Sync {
    fn decode(
        &self,
        envelope: &NotificationOutboxEnvelopeRecord,
    ) -> NotificationResult<NotificationSourceEventRef>;

    /// Event types this decoder claims out of the shared outbox.
    ///
    /// The intake pre-filter consumes this list instead of knowing any module's
    /// event vocabulary, so a source module extends its notification types in its
    /// own owner crate plus host wiring. An empty list claims every committed
    /// outbox row: selecting nothing and then rejecting a foreign row would write
    /// a permanent rejection for another source's event.
    fn accepted_event_types(&self) -> Vec<String> {
        Vec::new()
    }
}

/// Owner port for atomically acknowledging a terminal notification intake
/// receipt in the canonical Outbox retention ledger.
#[async_trait::async_trait]
pub trait NotificationOutboxIntakeReceiptPort: Send + Sync {
    async fn acknowledge_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        consumer_key: &str,
        outbox_event_id: Uuid,
        event_type: &str,
    ) -> NotificationResult<()>;
}

#[derive(Clone)]
struct ReceiptPortBinding {
    consumer_key: String,
    port: Arc<dyn NotificationOutboxIntakeReceiptPort>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationOutboxIntakeResult {
    pub outbox_event_id: Uuid,
    pub source_inbox_id: Uuid,
    pub source_slug: String,
    pub event_type: String,
    pub source_revision: i64,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationOutboxIntakeRejection {
    pub outbox_event_id: Uuid,
    pub event_type: String,
    pub schema_version: i16,
    pub error_code: String,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum NotificationOutboxIntakeOutcome {
    Accepted(NotificationOutboxIntakeResult),
    Rejected(NotificationOutboxIntakeRejection),
}

impl NotificationOutboxIntakeOutcome {
    pub const fn replayed(&self) -> bool {
        match self {
            Self::Accepted(result) => result.replayed,
            Self::Rejected(result) => result.replayed,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationOutboxIntakeFailure {
    pub outbox_event_id: Uuid,
    pub error_code: String,
    pub retryable: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationOutboxIntakeBatchResult {
    pub selected: usize,
    pub accepted: usize,
    pub rejected: usize,
    pub replayed: usize,
    pub failures: Vec<NotificationOutboxIntakeFailure>,
}

/// Exclusive, process-local scan position. Keep the exact database ordering
/// value so a terminal cursor row can disappear without losing the position.
#[derive(Clone, Debug)]
pub struct NotificationOutboxIntakeCursor {
    created_at: Value,
    id: Uuid,
}

#[derive(Clone, Debug)]
pub struct NotificationOutboxIntakePage {
    pub event_ids: Vec<Uuid>,
    /// None means the end of this pass; restart from the oldest pending row.
    pub next_cursor: Option<NotificationOutboxIntakeCursor>,
}

#[derive(Clone)]
pub struct NotificationOutboxIntakeWorker {
    db: DatabaseConnection,
    decoder: Arc<dyn NotificationOutboxEnvelopeDecoder>,
    batch_size: usize,
    receipt_port: Option<ReceiptPortBinding>,
}

impl std::fmt::Debug for NotificationOutboxIntakeWorker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NotificationOutboxIntakeWorker")
            .field("batch_size", &self.batch_size)
            .field(
                "receipt_acknowledgements_enabled",
                &self.receipt_port.is_some(),
            )
            .finish_non_exhaustive()
    }
}

impl NotificationOutboxIntakeWorker {
    pub fn new(
        db: DatabaseConnection,
        decoder: Arc<dyn NotificationOutboxEnvelopeDecoder>,
        batch_size: usize,
    ) -> NotificationResult<Self> {
        if !(1..=MAX_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE).contains(&batch_size) {
            return Err(NotificationError::Validation(format!(
                "outbox intake batch size must contain between 1 and {MAX_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE} items"
            )));
        }
        Ok(Self {
            db,
            decoder,
            batch_size,
            receipt_port: None,
        })
    }

    pub fn batch_size(&self) -> usize {
        self.batch_size
    }

    pub fn with_receipt_port(
        mut self,
        consumer_key: impl Into<String>,
        port: Arc<dyn NotificationOutboxIntakeReceiptPort>,
    ) -> NotificationResult<Self> {
        let consumer_key = consumer_key.into();
        if consumer_key.trim().is_empty()
            || consumer_key.trim() != consumer_key
            || consumer_key.len() > 128
        {
            return Err(NotificationError::Validation(
                "outbox consumer key must contain 1 to 128 unpadded bytes".to_string(),
            ));
        }
        self.receipt_port = Some(ReceiptPortBinding { consumer_key, port });
        Ok(self)
    }

    /// Selects one stable bounded page of committed supported outbox envelopes
    /// that have neither an accepted receipt nor a permanent rejection record.
    pub async fn pending_outbox_event_ids(&self) -> NotificationResult<Vec<Uuid>> {
        Ok(self.pending_outbox_page_after(None).await?.event_ids)
    }

    /// Visit bounded pages in the canonical created_at/id order. Selection
    /// does not acknowledge anything: failures remain pending on the next
    /// wrap, but a persistent failed prefix cannot monopolize the scan.
    pub async fn pending_outbox_page_after(
        &self,
        after: Option<&NotificationOutboxIntakeCursor>,
    ) -> NotificationResult<NotificationOutboxIntakePage> {
        let receipts = intake_receipt::Entity::find()
            .select_only()
            .column(intake_receipt::Column::OutboxEventId)
            .into_query();
        let rejections = intake_rejection::Entity::find()
            .select_only()
            .column(intake_rejection::Column::OutboxEventId)
            .into_query();
        let mut select = outbox_event::Entity::find();
        let accepted = self.decoder.accepted_event_types();
        if !accepted.is_empty() {
            select = select.filter(outbox_event::Column::EventType.is_in(accepted));
        }
        if let Some(cursor) = after {
            select = select.filter(
                Condition::any()
                    .add(outbox_event::Column::CreatedAt.gt(cursor.created_at.clone()))
                    .add(
                        Condition::all()
                            .add(outbox_event::Column::CreatedAt.eq(cursor.created_at.clone()))
                            .add(outbox_event::Column::Id.gt(cursor.id)),
                    ),
            );
        }
        let select = select
            .filter(outbox_event::Column::Id.not_in_subquery(receipts))
            .filter(outbox_event::Column::Id.not_in_subquery(rejections))
            .select_only()
            .column(outbox_event::Column::Id)
            .column(outbox_event::Column::CreatedAt)
            .order_by_asc(outbox_event::Column::CreatedAt)
            .order_by_asc(outbox_event::Column::Id)
            .limit(self.batch_size as u64);

        // SQLite orders its stored timestamp text. Do not round-trip it
        // through chrono, which can change the offset/format and skip ties.
        // PostgreSQL uses the native timestamp value for its comparison.
        let rows: Vec<(Uuid, Value)> = if self.db.get_database_backend() == DatabaseBackend::Sqlite
        {
            select
                .into_tuple::<(Uuid, String)>()
                .all(&self.db)
                .await?
                .into_iter()
                .map(|(id, created_at)| (id, created_at.into()))
                .collect()
        } else {
            select
                .into_tuple::<(Uuid, DateTime<Utc>)>()
                .all(&self.db)
                .await?
                .into_iter()
                .map(|(id, created_at)| (id, created_at.into()))
                .collect()
        };
        let next_cursor = if rows.len() == self.batch_size {
            rows.last()
                .map(|(id, created_at)| NotificationOutboxIntakeCursor {
                    created_at: created_at.clone(),
                    id: *id,
                })
        } else {
            None
        };
        Ok(NotificationOutboxIntakePage {
            event_ids: rows.into_iter().map(|(id, _)| id).collect(),
            next_cursor,
        })
    }

    /// Returns a stable keyset page of already-terminal owner receipts. The
    /// server uses this during cutover to backfill generic Outbox acknowledgements
    /// before the retention worker starts.
    pub async fn terminal_outbox_event_ids_after(
        &self,
        after: Option<Uuid>,
        limit: usize,
    ) -> NotificationResult<Vec<Uuid>> {
        if !(1..=MAX_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE).contains(&limit) {
            return Err(NotificationError::Validation(format!(
                "terminal receipt page size must contain between 1 and {MAX_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE} items"
            )));
        }
        let mut accepted = intake_receipt::Entity::find()
            .select_only()
            .column(intake_receipt::Column::OutboxEventId);
        let mut rejected = intake_rejection::Entity::find()
            .select_only()
            .column(intake_rejection::Column::OutboxEventId);
        if let Some(cursor) = after {
            accepted = accepted.filter(intake_receipt::Column::OutboxEventId.gt(cursor));
            rejected = rejected.filter(intake_rejection::Column::OutboxEventId.gt(cursor));
        }
        let mut ids = accepted
            .order_by_asc(intake_receipt::Column::OutboxEventId)
            .limit(limit as u64)
            .into_tuple::<Uuid>()
            .all(&self.db)
            .await?;
        ids.extend(
            rejected
                .order_by_asc(intake_rejection::Column::OutboxEventId)
                .limit(limit as u64)
                .into_tuple::<Uuid>()
                .all(&self.db)
                .await?,
        );
        ids.sort_unstable();
        ids.dedup();
        ids.truncate(limit);
        Ok(ids)
    }

    pub async fn process_outbox_event(
        &self,
        outbox_event_id: Uuid,
    ) -> NotificationResult<NotificationOutboxIntakeOutcome> {
        if outbox_event_id.is_nil() {
            return Err(NotificationError::Validation(
                "outbox event id cannot be nil".to_string(),
            ));
        }
        let txn = self.db.begin().await?;
        // Serialize the mutually exclusive terminal outcomes before either
        // receipt lookup. This is the same source row fenced by retention
        // and generic ACK. NOWAIT avoids holding owner locks on contention.
        let query = outbox_event::Entity::find_by_id(outbox_event_id);
        let row = match txn.get_database_backend() {
            DatabaseBackend::Postgres => {
                txn.execute_unprepared("SET TRANSACTION ISOLATION LEVEL READ COMMITTED").await?;
                query.lock_with_behavior(LockType::Update, LockBehavior::Nowait).one(&txn).await?
            }
            DatabaseBackend::Sqlite => {
                // Reserve SQLite's writer before establishing a read snapshot.
                txn.execute_unprepared("UPDATE notification_outbox_intake_receipts SET event_type = event_type WHERE 1 = 0").await?;
                query.one(&txn).await?
            }
            _ => return Err(NotificationError::Database(sea_orm::DbErr::Custom(
                "unsupported notification outbox intake database".into(),
            ))),
        }.ok_or(NotificationError::InvalidEvent)?;
        let claims = self.decoder.accepted_event_types();
        if !claims.is_empty() && !claims.iter().any(|event_type| event_type == &row.event_type) {
            return Err(NotificationError::SourceIdentityConflict);
        }
        let envelope = envelope_record(&row);
        let accepted = intake_receipt::Entity::find_by_id(outbox_event_id).one(&txn).await?;
        let rejected = intake_rejection::Entity::find_by_id(outbox_event_id).one(&txn).await?;
        let outcome = match (accepted, rejected) {
            (Some(_), Some(_)) => return Err(NotificationError::SourceIdentityConflict),
            (Some(existing), None) => {
                let source_event = self.decoder.decode(&envelope)
                    .map_err(|_| NotificationError::SourceIdentityConflict)?;
                if source_event.event_type().as_str() != row.event_type.as_str() {
                    return Err(NotificationError::SourceIdentityConflict);
                }
                ensure_receipt_identity(&existing, outbox_event_id, &source_event)?;
                NotificationOutboxIntakeOutcome::Accepted(intake_result(existing, true))
            }
            (None, Some(existing)) => {
                ensure_rejection_matches_outbox(&existing, &row)?;
                NotificationOutboxIntakeOutcome::Rejected(rejection_result(existing, true))
            }
            (None, None) => {
                match self.decoder.decode(&envelope) {
                    Ok(source_event) => {
                        // A decoder cannot silently acknowledge a different
                        // event type under the source UUID, even without an
                        // attached generic receipt port.
                        if source_event.event_type().as_str() != row.event_type.as_str() {
                            return Err(NotificationError::SourceIdentityConflict);
                        }
                        // Preserve the outer source fence if acceptance fails
                        // permanently after a partial owner write.
                        let acceptance = txn.begin().await?;
                        match self.accept_decoded(&acceptance, outbox_event_id, source_event).await {
                            Ok(result) => {
                                acceptance.commit().await?;
                                NotificationOutboxIntakeOutcome::Accepted(result)
                            }
                            Err(error) => {
                                acceptance.rollback().await?;
                                if error.is_retryable() { return Err(error); }
                                self.persist_rejection(&txn, &row, &error).await?
                            }
                        }
                    }
                    Err(error) if error.is_retryable() => return Err(error),
                    Err(error) => self.persist_rejection(&txn, &row, &error).await?,
                }
            }
        };
        self.acknowledge_outbox_event_in_transaction(&txn, outbox_event_id, row.event_type.as_str()).await?;
        txn.commit().await?;
        Ok(outcome)
    }

    pub async fn process_next_batch(
        &self,
    ) -> NotificationResult<NotificationOutboxIntakeBatchResult> {
        let event_ids = self.pending_outbox_event_ids().await?;
        let mut result = NotificationOutboxIntakeBatchResult {
            selected: event_ids.len(),
            ..NotificationOutboxIntakeBatchResult::default()
        };
        for outbox_event_id in event_ids {
            match self.process_outbox_event(outbox_event_id).await {
                Ok(NotificationOutboxIntakeOutcome::Accepted(outcome)) => {
                    result.accepted += 1;
                    if outcome.replayed {
                        result.replayed += 1;
                    }
                }
                Ok(NotificationOutboxIntakeOutcome::Rejected(outcome)) => {
                    result.rejected += 1;
                    if outcome.replayed {
                        result.replayed += 1;
                    }
                }
                Err(error) => result.failures.push(NotificationOutboxIntakeFailure {
                    outbox_event_id,
                    error_code: error.stable_code().to_string(),
                    retryable: error.is_retryable(),
                }),
            }
        }
        Ok(result)
    }

    // Called only under the source fence, after both terminal lookups.
    async fn accept_decoded(
        &self,
        txn: &DatabaseTransaction,
        outbox_event_id: Uuid,
        source_event: NotificationSourceEventRef,
    ) -> NotificationResult<NotificationOutboxIntakeResult> {
        let source_revision = source_revision_i64(&source_event)?;
        let timestamp = now();
        source_inbox::Entity::insert(source_inbox::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(source_event.tenant_id()),
            source_slug: Set(source_event.source().as_str().to_string()),
            source_event_id: Set(source_event.event_id()),
            source_revision: Set(source_revision),
            event_type: Set(source_event.event_type().as_str().to_string()),
            status: Set(NotificationSourceInboxStatus::Pending),
            attempt_count: Set(0),
            next_attempt_at: Set(None),
            lease_owner: Set(None),
            lease_expires_at: Set(None),
            fanout_job_id: Set(None),
            last_error_code: Set(None),
            last_error_message: Set(None),
            completed_at: Set(None),
            created_at: Set(timestamp),
            updated_at: Set(timestamp),
        })
        .on_conflict(
            OnConflict::columns([
                source_inbox::Column::TenantId,
                source_inbox::Column::SourceSlug,
                source_inbox::Column::SourceEventId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(txn)
        .await?;

        let inbox = source_inbox::Entity::find()
            .filter(source_inbox::Column::TenantId.eq(source_event.tenant_id()))
            .filter(source_inbox::Column::SourceSlug.eq(source_event.source().as_str()))
            .filter(source_inbox::Column::SourceEventId.eq(source_event.event_id()))
            .one(txn)
            .await?
            .ok_or(NotificationError::InvalidEvent)?;
        ensure_source_inbox_identity(&inbox, &source_event)?;

        intake_receipt::Entity::insert(intake_receipt::ActiveModel {
            outbox_event_id: Set(outbox_event_id),
            tenant_id: Set(source_event.tenant_id()),
            event_type: Set(source_event.event_type().as_str().to_string()),
            source_slug: Set(source_event.source().as_str().to_string()),
            source_event_id: Set(source_event.event_id()),
            source_revision: Set(source_revision),
            source_inbox_id: Set(inbox.id),
            created_at: Set(timestamp),
        })
        .on_conflict(
            OnConflict::column(intake_receipt::Column::OutboxEventId)
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(txn)
        .await?;

        let receipt = intake_receipt::Entity::find_by_id(outbox_event_id)
            .one(txn)
            .await?
            .ok_or(NotificationError::InvalidEvent)?;
        ensure_receipt_identity(&receipt, outbox_event_id, &source_event)?;
        Ok(intake_result(receipt, false))
    }

    async fn persist_rejection(
        &self,
        txn: &DatabaseTransaction,
        row: &outbox_event::Model,
        error: &NotificationError,
    ) -> NotificationResult<NotificationOutboxIntakeOutcome> {
        debug_assert!(!error.is_retryable());
        intake_rejection::Entity::insert(intake_rejection::ActiveModel {
            outbox_event_id: Set(row.id),
            event_type: Set(row.event_type.clone()),
            schema_version: Set(row.schema_version),
            error_code: Set(error.stable_code().to_string()),
            created_at: Set(now()),
        })
        .on_conflict(
            OnConflict::column(intake_rejection::Column::OutboxEventId)
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(txn)
        .await?;

        let rejection = intake_rejection::Entity::find_by_id(row.id)
            .one(txn)
            .await?
            .ok_or(NotificationError::InvalidEvent)?;
        ensure_rejection_matches_outbox(&rejection, row)?;
        Ok(NotificationOutboxIntakeOutcome::Rejected(rejection_result(
            rejection, false,
        )))
    }

    async fn acknowledge_outbox_event_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        outbox_event_id: Uuid,
        event_type: &str,
    ) -> NotificationResult<()> {
        if let Some(binding) = &self.receipt_port {
            binding
                .port
                .acknowledge_in_transaction(
                    transaction,
                    &binding.consumer_key,
                    outbox_event_id,
                    event_type,
                )
                .await?;
        }
        Ok(())
    }
}

fn envelope_record(row: &outbox_event::Model) -> NotificationOutboxEnvelopeRecord {
    NotificationOutboxEnvelopeRecord {
        outbox_event_id: row.id,
        event_type: row.event_type.clone(),
        schema_version: row.schema_version,
        payload: row.payload.clone(),
    }
}

fn source_revision_i64(event: &NotificationSourceEventRef) -> NotificationResult<i64> {
    i64::try_from(event.source_revision()).map_err(|_| NotificationError::InvalidEvent)
}

fn ensure_source_inbox_identity(
    inbox: &source_inbox::Model,
    event: &NotificationSourceEventRef,
) -> NotificationResult<()> {
    if inbox.tenant_id != event.tenant_id()
        || inbox.source_slug != event.source().as_str()
        || inbox.source_event_id != event.event_id()
        || inbox.event_type != event.event_type().as_str()
        || inbox.source_revision != source_revision_i64(event)?
    {
        return Err(NotificationError::SourceIdentityConflict);
    }
    Ok(())
}

fn ensure_receipt_identity(
    receipt: &intake_receipt::Model,
    outbox_event_id: Uuid,
    event: &NotificationSourceEventRef,
) -> NotificationResult<()> {
    if receipt.outbox_event_id != outbox_event_id
        || receipt.tenant_id != event.tenant_id()
        || receipt.event_type != event.event_type().as_str()
        || receipt.source_slug != event.source().as_str()
        || receipt.source_event_id != event.event_id()
        || receipt.source_revision != source_revision_i64(event)?
    {
        return Err(NotificationError::SourceIdentityConflict);
    }
    Ok(())
}

fn ensure_rejection_matches_outbox(
    rejection: &intake_rejection::Model,
    row: &outbox_event::Model,
) -> NotificationResult<()> {
    if rejection.outbox_event_id != row.id
        || rejection.event_type != row.event_type
        || rejection.schema_version != row.schema_version
    {
        return Err(NotificationError::SourceIdentityConflict);
    }
    Ok(())
}

fn intake_result(receipt: intake_receipt::Model, replayed: bool) -> NotificationOutboxIntakeResult {
    NotificationOutboxIntakeResult {
        outbox_event_id: receipt.outbox_event_id,
        source_inbox_id: receipt.source_inbox_id,
        source_slug: receipt.source_slug,
        event_type: receipt.event_type,
        source_revision: receipt.source_revision,
        replayed,
    }
}

fn rejection_result(
    rejection: intake_rejection::Model,
    replayed: bool,
) -> NotificationOutboxIntakeRejection {
    NotificationOutboxIntakeRejection {
        outbox_event_id: rejection.outbox_event_id,
        event_type: rejection.event_type,
        schema_version: rejection.schema_version,
        error_code: rejection.error_code,
        replayed,
    }
}

fn now() -> DateTime<FixedOffset> {
    Utc::now().fixed_offset()
}
