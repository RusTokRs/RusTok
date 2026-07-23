use chrono::{DateTime, FixedOffset, Utc};
use rustok_events::{
    ContractEventEnvelope, ContractEventPayload, DomainEvent, EventEnvelope, ForumMentionEvent,
};
use rustok_notifications_api::{
    NotificationSourceEventRef, NotificationSourceSlug, NotificationTypeKey,
};
use rustok_outbox::entity::{self as outbox_event, SysEventStatus};
use sea_orm::{
    ActiveValue::Set, ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, TransactionTrait, sea_query::OnConflict,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::source_inbox;
use crate::error::{NotificationError, NotificationResult};
use crate::model::NotificationSourceInboxStatus;

pub const DEFAULT_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE: usize = 32;
pub const MAX_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE: usize = 64;
const FORUM_SOURCE: &str = "forum";
const FORUM_TOPIC_CREATED: &str = "forum.topic.created";
const FORUM_USER_MENTION_ADDED: &str = "forum.mention.user_added";

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
pub struct NotificationOutboxIntakeFailure {
    pub outbox_event_id: Uuid,
    pub error_code: String,
    pub retryable: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationOutboxIntakeBatchResult {
    pub selected: usize,
    pub accepted: usize,
    pub replayed: usize,
    pub failures: Vec<NotificationOutboxIntakeFailure>,
}

#[derive(Clone)]
pub struct NotificationOutboxIntakeWorker {
    db: DatabaseConnection,
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
    pub fn new(db: DatabaseConnection, batch_size: usize) -> NotificationResult<Self> {
        if !(1..=MAX_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE).contains(&batch_size) {
            return Err(NotificationError::Validation(format!(
                "outbox intake batch size must contain between 1 and {MAX_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE} items"
            )));
        }
        Ok(Self { db, batch_size })
    }

    pub fn batch_size(&self) -> usize {
        self.batch_size
    }

    /// Selects one stable bounded page of dispatched, supported outbox envelopes
    /// that do not yet have a durable Notifications intake receipt.
    pub async fn pending_outbox_event_ids(&self) -> NotificationResult<Vec<Uuid>> {
        let receipts = intake_receipt::Entity::find()
            .select_only()
            .column(intake_receipt::Column::OutboxEventId)
            .into_query();
        let rows = outbox_event::Entity::find()
            .filter(outbox_event::Column::Status.eq(SysEventStatus::Dispatched))
            .filter(
                Condition::any()
                    .add(outbox_event::Column::EventType.eq(FORUM_TOPIC_CREATED))
                    .add(outbox_event::Column::EventType.eq(FORUM_USER_MENTION_ADDED)),
            )
            .filter(outbox_event::Column::Id.not_in_subquery(receipts))
            .order_by_asc(outbox_event::Column::DispatchedAt)
            .order_by_asc(outbox_event::Column::CreatedAt)
            .order_by_asc(outbox_event::Column::Id)
            .limit(self.batch_size as u64)
            .all(&self.db)
            .await?;
        Ok(rows.into_iter().map(|row| row.id).collect())
    }

    /// Accepts one dispatched source envelope into the durable source inbox.
    ///
    /// The producer payload is decoded from `sys_events`; no producer service or
    /// producer-owned table is called. Source inbox creation and the outbox intake
    /// receipt commit in one transaction.
    pub async fn process_outbox_event(
        &self,
        outbox_event_id: Uuid,
    ) -> NotificationResult<NotificationOutboxIntakeResult> {
        let row = outbox_event::Entity::find_by_id(outbox_event_id)
            .filter(outbox_event::Column::Status.eq(SysEventStatus::Dispatched))
            .one(&self.db)
            .await?
            .ok_or(NotificationError::InvalidEvent)?;
        let source_event = decode_source_event(&row)?;
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

    pub async fn process_next_batch(&self) -> NotificationResult<NotificationOutboxIntakeBatchResult> {
        let event_ids = self.pending_outbox_event_ids().await?;
        let mut result = NotificationOutboxIntakeBatchResult {
            selected: event_ids.len(),
            ..NotificationOutboxIntakeBatchResult::default()
        };
        for outbox_event_id in event_ids {
            match self.process_outbox_event(outbox_event_id).await {
                Ok(receipt) => {
                    result.accepted += 1;
                    if receipt.replayed {
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
}

fn decode_source_event(row: &outbox_event::Model) -> NotificationResult<NotificationSourceEventRef> {
    if let Ok(envelope) = serde_json::from_value::<ContractEventEnvelope>(row.payload.clone()) {
        envelope
            .validate_registered_schema()
            .map_err(|_| NotificationError::InvalidEvent)?;
        if envelope.id() != row.id
            || envelope.event_type() != row.event_type
            || i16::try_from(envelope.schema_version()).ok() != Some(row.schema_version)
        {
            return Err(NotificationError::InvalidEvent);
        }
        let tenant_id = envelope.tenant_id();
        let event_id = envelope.id();
        return match envelope
            .into_payload()
            .map_err(|_| NotificationError::InvalidEvent)?
        {
            ContractEventPayload::ForumMention(ForumMentionEvent::UserMentionAdded {
                source_revision_id,
                ..
            }) if row.event_type == FORUM_USER_MENTION_ADDED => source_event_ref(
                tenant_id,
                event_id,
                FORUM_USER_MENTION_ADDED,
                u64::try_from(source_revision_id).map_err(|_| NotificationError::InvalidEvent)?,
            ),
            _ => Err(NotificationError::InvalidEvent),
        };
    }

    let envelope = serde_json::from_value::<EventEnvelope>(row.payload.clone())?;
    envelope
        .validate_registered_schema()
        .map_err(|_| NotificationError::InvalidEvent)?;
    if envelope.id != row.id
        || envelope.event_type != row.event_type
        || i16::try_from(envelope.schema_version).ok() != Some(row.schema_version)
    {
        return Err(NotificationError::InvalidEvent);
    }
    match envelope.event {
        DomainEvent::ForumTopicCreated { topic_id, .. }
            if row.event_type == FORUM_TOPIC_CREATED =>
        {
            source_event_ref(envelope.tenant_id, topic_id, FORUM_TOPIC_CREATED, 1)
        }
        _ => Err(NotificationError::InvalidEvent),
    }
}

fn source_event_ref(
    tenant_id: Uuid,
    event_id: Uuid,
    event_type: &str,
    source_revision: u64,
) -> NotificationResult<NotificationSourceEventRef> {
    NotificationSourceEventRef::new(
        tenant_id,
        event_id,
        NotificationSourceSlug::new(FORUM_SOURCE).map_err(|_| NotificationError::InvalidEvent)?,
        NotificationTypeKey::new(event_type).map_err(|_| NotificationError::InvalidEvent)?,
        source_revision,
    )
    .map_err(|_| NotificationError::InvalidEvent)
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

fn intake_result(
    receipt: intake_receipt::Model,
    replayed: bool,
) -> NotificationOutboxIntakeResult {
    NotificationOutboxIntakeResult {
        outbox_event_id: receipt.outbox_event_id,
        source_inbox_id: receipt.source_inbox_id,
        source_slug: receipt.source_slug,
        event_type: receipt.event_type,
        source_revision: receipt.source_revision,
        replayed,
    }
}

fn now() -> DateTime<FixedOffset> {
    Utc::now().fixed_offset()
}
