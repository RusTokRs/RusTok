use std::sync::Arc;

use chrono::{DateTime, FixedOffset, Utc};
use rustok_notifications_api::NotificationSourceEventRef;
use sea_orm::{
    ActiveValue::Set, ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, QueryTrait, TransactionTrait, sea_query::OnConflict,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::source_inbox;
use crate::error::{NotificationError, NotificationResult};
use crate::model::NotificationSourceInboxStatus;

pub const DEFAULT_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE: usize = 32;
pub const MAX_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE: usize = 64;
const FORUM_TOPIC_CREATED: &str = "forum.topic.created";
const FORUM_USER_MENTION_ADDED: &str = "forum.mention.user_added";

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

#[derive(Clone)]
pub struct NotificationOutboxIntakeWorker {
    db: DatabaseConnection,
    decoder: Arc<dyn NotificationOutboxEnvelopeDecoder>,
    batch_size: usize,
}

impl std::fmt::Debug for NotificationOutboxIntakeWorker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NotificationOutboxIntakeWorker")
            .field("batch_size", &self.batch_size)
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
        })
    }

    pub fn batch_size(&self) -> usize {
        self.batch_size
    }

    /// Selects one stable bounded page of committed supported outbox envelopes
    /// that have neither an accepted receipt nor a permanent rejection record.
    pub async fn pending_outbox_event_ids(&self) -> NotificationResult<Vec<Uuid>> {
        let receipts = intake_receipt::Entity::find()
            .select_only()
            .column(intake_receipt::Column::OutboxEventId)
            .into_query();
        let rejections = intake_rejection::Entity::find()
            .select_only()
            .column(intake_rejection::Column::OutboxEventId)
            .into_query();
        let rows = outbox_event::Entity::find()
            .filter(
                Condition::any()
                    .add(outbox_event::Column::EventType.eq(FORUM_TOPIC_CREATED))
                    .add(outbox_event::Column::EventType.eq(FORUM_USER_MENTION_ADDED)),
            )
            .filter(outbox_event::Column::Id.not_in_subquery(receipts))
            .filter(outbox_event::Column::Id.not_in_subquery(rejections))
            .order_by_asc(outbox_event::Column::CreatedAt)
            .order_by_asc(outbox_event::Column::Id)
            .limit(self.batch_size as u64)
            .all(&self.db)
            .await?;
        Ok(rows.into_iter().map(|row| row.id).collect())
    }

    pub async fn process_outbox_event(
        &self,
        outbox_event_id: Uuid,
    ) -> NotificationResult<NotificationOutboxIntakeOutcome> {
        let row = outbox_event::Entity::find_by_id(outbox_event_id)
            .one(&self.db)
            .await?
            .ok_or(NotificationError::InvalidEvent)?;
        let envelope = envelope_record(&row);

        if let Some(existing) = intake_receipt::Entity::find_by_id(outbox_event_id)
            .one(&self.db)
            .await?
        {
            let source_event = self
                .decoder
                .decode(&envelope)
                .map_err(|_| NotificationError::SourceIdentityConflict)?;
            ensure_receipt_identity(&existing, outbox_event_id, &source_event)?;
            return Ok(NotificationOutboxIntakeOutcome::Accepted(intake_result(
                existing, true,
            )));
        }
        if let Some(existing) = intake_rejection::Entity::find_by_id(outbox_event_id)
            .one(&self.db)
            .await?
        {
            ensure_rejection_matches_outbox(&existing, &row)?;
            return Ok(NotificationOutboxIntakeOutcome::Rejected(rejection_result(
                existing, true,
            )));
        }

        let source_event = match self.decoder.decode(&envelope) {
            Ok(source_event) => source_event,
            Err(error) if error.is_retryable() => return Err(error),
            Err(error) => return self.persist_rejection(&row, &error).await,
        };

        match self.accept_decoded(outbox_event_id, source_event).await {
            Ok(result) => Ok(NotificationOutboxIntakeOutcome::Accepted(result)),
            Err(error) if error.is_retryable() => Err(error),
            Err(error) => self.persist_rejection(&row, &error).await,
        }
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

    async fn accept_decoded(
        &self,
        outbox_event_id: Uuid,
        source_event: NotificationSourceEventRef,
    ) -> NotificationResult<NotificationOutboxIntakeResult> {
        let source_revision = source_revision_i64(&source_event)?;
        let txn = self.db.begin().await?;
        if let Some(existing) = intake_receipt::Entity::find_by_id(outbox_event_id)
            .one(&txn)
            .await?
        {
            ensure_receipt_identity(&existing, outbox_event_id, &source_event)?;
            txn.commit().await?;
            return Ok(intake_result(existing, true));
        }
        if intake_rejection::Entity::find_by_id(outbox_event_id)
            .one(&txn)
            .await?
            .is_some()
        {
            return Err(NotificationError::SourceIdentityConflict);
        }

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
        .exec_without_returning(&txn)
        .await?;

        let inbox = source_inbox::Entity::find()
            .filter(source_inbox::Column::TenantId.eq(source_event.tenant_id()))
            .filter(source_inbox::Column::SourceSlug.eq(source_event.source().as_str()))
            .filter(source_inbox::Column::SourceEventId.eq(source_event.event_id()))
            .one(&txn)
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
        .exec_without_returning(&txn)
        .await?;

        let receipt = intake_receipt::Entity::find_by_id(outbox_event_id)
            .one(&txn)
            .await?
            .ok_or(NotificationError::InvalidEvent)?;
        ensure_receipt_identity(&receipt, outbox_event_id, &source_event)?;
        txn.commit().await?;
        Ok(intake_result(receipt, false))
    }

    async fn persist_rejection(
        &self,
        row: &outbox_event::Model,
        error: &NotificationError,
    ) -> NotificationResult<NotificationOutboxIntakeOutcome> {
        debug_assert!(!error.is_retryable());
        let txn = self.db.begin().await?;
        if let Some(existing) = intake_receipt::Entity::find_by_id(row.id).one(&txn).await? {
            let source_event = self
                .decoder
                .decode(&envelope_record(row))
                .map_err(|_| NotificationError::SourceIdentityConflict)?;
            ensure_receipt_identity(&existing, row.id, &source_event)?;
            txn.commit().await?;
            return Ok(NotificationOutboxIntakeOutcome::Accepted(intake_result(
                existing, true,
            )));
        }
        if let Some(existing) = intake_rejection::Entity::find_by_id(row.id)
            .one(&txn)
            .await?
        {
            ensure_rejection_matches_outbox(&existing, row)?;
            txn.commit().await?;
            return Ok(NotificationOutboxIntakeOutcome::Rejected(rejection_result(
                existing, true,
            )));
        }

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
        .exec_without_returning(&txn)
        .await?;

        let rejection = intake_rejection::Entity::find_by_id(row.id)
            .one(&txn)
            .await?
            .ok_or(NotificationError::InvalidEvent)?;
        ensure_rejection_matches_outbox(&rejection, row)?;
        txn.commit().await?;
        Ok(NotificationOutboxIntakeOutcome::Rejected(rejection_result(
            rejection, false,
        )))
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
