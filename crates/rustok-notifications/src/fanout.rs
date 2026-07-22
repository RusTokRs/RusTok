use std::sync::Arc;

use chrono::{Duration, Utc};
use rustok_core::ModuleRuntimeExtensions;
use rustok_notifications_api::{
    DescribeNotificationRequest, NotificationAudienceCursor, NotificationSemanticDescriptor,
    NotificationSourceEventRef, NotificationSourceRegistry, NotificationSourceSlug,
    NotificationTypeKey, ResolveNotificationAudienceRequest,
    notification_source_registry_from_extensions,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, DatabaseConnection, EntityTrait,
    IntoActiveModel, QueryFilter, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::{fanout_item, fanout_job, source_inbox};
use crate::error::{NotificationError, NotificationResult};
use crate::model::{FanoutItemStatus, NotificationJobStatus, NotificationSourceInboxStatus};

const DEFAULT_LEASE_SECONDS: i64 = 60;
const RETRY_DELAY_SECONDS: i64 = 30;
const MAX_WORKER_ID_BYTES: usize = 191;
const MAX_DESCRIPTOR_BYTES: usize = 16 * 1024;
const MAX_ERROR_CODE_BYTES: usize = 100;
const MAX_ERROR_MESSAGE_BYTES: usize = 2_000;
const MAX_FANOUT_PAGE_SIZE: u16 = 256;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationSourceInboxReceipt {
    pub inbox_id: Uuid,
    pub status: NotificationSourceInboxStatus,
    pub fanout_job_id: Option<Uuid>,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationFanoutPageResult {
    pub job_id: Uuid,
    pub candidates: usize,
    pub inserted_items: usize,
    pub next_cursor: Option<String>,
    pub completed: bool,
}

#[derive(Clone)]
pub struct NotificationFanoutService {
    db: DatabaseConnection,
    registry: Arc<NotificationSourceRegistry>,
}

impl NotificationFanoutService {
    pub fn new(db: DatabaseConnection, registry: Arc<NotificationSourceRegistry>) -> Self {
        Self { db, registry }
    }

    pub fn from_runtime_extensions(
        db: DatabaseConnection,
        extensions: &ModuleRuntimeExtensions,
    ) -> Self {
        let registry = notification_source_registry_from_extensions(extensions)
            .unwrap_or_else(|| Arc::new(NotificationSourceRegistry::default()));
        Self::new(db, registry)
    }

    pub async fn enqueue_source_event(
        &self,
        event: NotificationSourceEventRef,
    ) -> NotificationResult<NotificationSourceInboxReceipt> {
        validate_source_event(&event)?;
        self.provider_for_event(&event)?;

        if let Some(existing) = self.find_inbox(&event).await? {
            ensure_inbox_identity(&existing, &event)?;
            return Ok(receipt(existing, true));
        }

        let now = Utc::now().into();
        let candidate = source_inbox::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(event.tenant_id()),
            source_slug: Set(event.source().as_str().to_string()),
            source_event_id: Set(event.event_id()),
            source_revision: Set(source_revision_i64(&event)?),
            event_type: Set(event.event_type().as_str().to_string()),
            status: Set(NotificationSourceInboxStatus::Pending),
            attempt_count: Set(0),
            next_attempt_at: Set(None),
            lease_owner: Set(None),
            lease_expires_at: Set(None),
            fanout_job_id: Set(None),
            last_error_code: Set(None),
            last_error_message: Set(None),
            completed_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        };

        match candidate.insert(&self.db).await {
            Ok(inserted) => Ok(receipt(inserted, false)),
            Err(insert_error) => {
                if let Some(existing) = self.find_inbox(&event).await? {
                    ensure_inbox_identity(&existing, &event)?;
                    Ok(receipt(existing, true))
                } else {
                    Err(insert_error.into())
                }
            }
        }
    }

    pub async fn materialize_source_event(
        &self,
        inbox_id: Uuid,
        worker_id: &str,
    ) -> NotificationResult<NotificationSourceInboxReceipt> {
        validate_worker_id(worker_id)?;
        let inbox = source_inbox::Entity::find_by_id(inbox_id)
            .one(&self.db)
            .await?
            .ok_or(NotificationError::InvalidEvent)?;
        if is_terminal_inbox(inbox.status) {
            return Ok(receipt(inbox, true));
        }

        self.claim_inbox(inbox_id, worker_id).await?;
        let inbox = source_inbox::Entity::find_by_id(inbox_id)
            .one(&self.db)
            .await?
            .ok_or(NotificationError::InvalidEvent)?;
        let event = event_from_inbox(&inbox)?;
        let provider = self.provider_for_event(&event)?;

        let descriptor = match provider
            .describe_event(DescribeNotificationRequest {
                event: event.clone(),
            })
            .await
        {
            Ok(descriptor) => descriptor,
            Err(provider_error) => {
                let error = NotificationError::from(provider_error);
                self.finish_inbox_error(&inbox, worker_id, &error).await?;
                return Err(error);
            }
        };

        let Some(descriptor) = descriptor else {
            let completed = self
                .finish_inbox_terminal(
                    &inbox,
                    worker_id,
                    NotificationSourceInboxStatus::Suppressed,
                    None,
                )
                .await?;
            return Ok(receipt(completed, false));
        };
        validate_descriptor(&event, &descriptor)?;
        let descriptor_json = serde_json::to_value(&descriptor)?;
        if serde_json::to_vec(&descriptor_json)?.len() > MAX_DESCRIPTOR_BYTES {
            let error = NotificationError::InvalidDescriptor;
            self.finish_inbox_error(&inbox, worker_id, &error).await?;
            return Err(error);
        }

        let job = match self
            .find_or_create_job(&event, descriptor.notification_type.as_str(), descriptor_json)
            .await
        {
            Ok(job) => job,
            Err(error) => {
                self.finish_inbox_error(&inbox, worker_id, &error).await?;
                return Err(error);
            }
        };
        let completed = self
            .finish_inbox_terminal(
                &inbox,
                worker_id,
                NotificationSourceInboxStatus::Completed,
                Some(job.id),
            )
            .await?;
        Ok(receipt(completed, false))
    }

    pub async fn process_fanout_page(
        &self,
        job_id: Uuid,
        worker_id: &str,
        limit: u16,
    ) -> NotificationResult<NotificationFanoutPageResult> {
        validate_worker_id(worker_id)?;
        if limit == 0 || limit > MAX_FANOUT_PAGE_SIZE {
            return Err(NotificationError::Validation(format!(
                "fan-out page size must be between 1 and {MAX_FANOUT_PAGE_SIZE}"
            )));
        }

        let claimed = self.claim_job(job_id, worker_id).await?;
        if claimed.status == NotificationJobStatus::Completed {
            return Ok(NotificationFanoutPageResult {
                job_id,
                candidates: 0,
                inserted_items: 0,
                next_cursor: None,
                completed: true,
            });
        }
        let event = event_from_job(&claimed)?;
        let descriptor: NotificationSemanticDescriptor =
            serde_json::from_value(claimed.descriptor_json.clone())
                .map_err(|_| NotificationError::InvalidDescriptor)?;
        validate_descriptor(&event, &descriptor)?;
        let provider = self.provider_for_event(&event)?;
        let cursor = claimed
            .audience_cursor
            .as_ref()
            .map(|cursor| NotificationAudienceCursor::new(cursor.clone()))
            .transpose()
            .map_err(|_| NotificationError::InvalidEvent)?;

        let page = match provider
            .resolve_audience(ResolveNotificationAudienceRequest {
                event,
                descriptor,
                cursor,
                limit,
            })
            .await
        {
            Ok(page) => page,
            Err(provider_error) => {
                let error = NotificationError::from(provider_error);
                self.finish_job_error(&claimed, worker_id, &error).await?;
                return Err(error);
            }
        };
        let (recipients, next_cursor) = page.into_parts();
        let next_cursor = next_cursor.map(|cursor| cursor.as_str().to_string());
        if next_cursor.is_some() && next_cursor == claimed.audience_cursor {
            let error = NotificationError::CursorDidNotAdvance;
            self.finish_job_error(&claimed, worker_id, &error).await?;
            return Err(error);
        }

        let txn = self.db.begin().await?;
        let current = fanout_job::Entity::find_by_id(job_id)
            .one(&txn)
            .await?
            .ok_or(NotificationError::InvalidEvent)?;
        ensure_job_lease(&current, worker_id)?;

        let mut inserted_items = 0usize;
        for candidate in &recipients {
            let existing = fanout_item::Entity::find()
                .filter(fanout_item::Column::TenantId.eq(current.tenant_id))
                .filter(fanout_item::Column::FanoutJobId.eq(current.id))
                .filter(fanout_item::Column::RecipientId.eq(candidate.recipient_id))
                .one(&txn)
                .await?;
            if existing.is_some() {
                continue;
            }
            let now = Utc::now().into();
            let item = fanout_item::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(current.tenant_id),
                fanout_job_id: Set(current.id),
                recipient_id: Set(candidate.recipient_id),
                status: Set(FanoutItemStatus::Pending),
                notification_id: Set(None),
                idempotency_key: Set(format!(
                    "fanout:{}:{}",
                    current.id, candidate.recipient_id
                )),
                last_error_code: Set(None),
                created_at: Set(now),
                updated_at: Set(now),
                processed_at: Set(None),
            };
            match item.insert(&txn).await {
                Ok(_) => inserted_items += 1,
                Err(insert_error) => {
                    let duplicate = fanout_item::Entity::find()
                        .filter(fanout_item::Column::TenantId.eq(current.tenant_id))
                        .filter(fanout_item::Column::FanoutJobId.eq(current.id))
                        .filter(fanout_item::Column::RecipientId.eq(candidate.recipient_id))
                        .one(&txn)
                        .await?;
                    if duplicate.is_none() {
                        return Err(insert_error.into());
                    }
                }
            }
        }

        let complete = next_cursor.is_none();
        let now = Utc::now().into();
        let mut update = current.into_active_model();
        update.audience_cursor = Set(next_cursor.clone());
        update.status = Set(if complete {
            NotificationJobStatus::Completed
        } else {
            NotificationJobStatus::Pending
        });
        update.lease_owner = Set(None);
        update.lease_expires_at = Set(None);
        update.next_attempt_at = Set(None);
        update.last_error_code = Set(None);
        update.last_error_message = Set(None);
        update.completed_at = Set(complete.then_some(now));
        update.updated_at = Set(now);
        update.update(&txn).await?;
        txn.commit().await?;

        Ok(NotificationFanoutPageResult {
            job_id,
            candidates: recipients.len(),
            inserted_items,
            next_cursor,
            completed: complete,
        })
    }

    async fn provider_for_event(
        &self,
        event: &NotificationSourceEventRef,
    ) -> NotificationResult<Arc<dyn rustok_notifications_api::NotificationSourceProvider>> {
        let provider = self
            .registry
            .get(event.source())
            .ok_or(NotificationError::SourceUnavailable)?;
        if !provider
            .supported_types()
            .iter()
            .any(|event_type| event_type == event.event_type())
        {
            return Err(NotificationError::UnsupportedEvent);
        }
        Ok(provider)
    }

    async fn find_inbox(
        &self,
        event: &NotificationSourceEventRef,
    ) -> NotificationResult<Option<source_inbox::Model>> {
        Ok(source_inbox::Entity::find()
            .filter(source_inbox::Column::TenantId.eq(event.tenant_id()))
            .filter(source_inbox::Column::SourceSlug.eq(event.source().as_str()))
            .filter(source_inbox::Column::SourceEventId.eq(event.event_id()))
            .filter(source_inbox::Column::EventType.eq(event.event_type().as_str()))
            .one(&self.db)
            .await?)
    }

    async fn claim_inbox(&self, inbox_id: Uuid, worker_id: &str) -> NotificationResult<()> {
        let now = Utc::now();
        let lease_expires = (now + Duration::seconds(DEFAULT_LEASE_SECONDS)).into();
        let result = source_inbox::Entity::update_many()
            .set(source_inbox::ActiveModel {
                status: Set(NotificationSourceInboxStatus::Processing),
                lease_owner: Set(Some(worker_id.to_string())),
                lease_expires_at: Set(Some(lease_expires)),
                next_attempt_at: Set(None),
                completed_at: Set(None),
                updated_at: Set(now.into()),
                ..Default::default()
            })
            .filter(source_inbox::Column::Id.eq(inbox_id))
            .filter(
                Condition::any()
                    .add(source_inbox::Column::Status.eq(NotificationSourceInboxStatus::Pending))
                    .add(
                        Condition::all()
                            .add(
                                source_inbox::Column::Status
                                    .eq(NotificationSourceInboxStatus::RetryableError),
                            )
                            .add(
                                Condition::any()
                                    .add(source_inbox::Column::NextAttemptAt.is_null())
                                    .add(source_inbox::Column::NextAttemptAt.lte(now)),
                            ),
                    )
                    .add(
                        Condition::all()
                            .add(
                                source_inbox::Column::Status
                                    .eq(NotificationSourceInboxStatus::Processing),
                            )
                            .add(source_inbox::Column::LeaseExpiresAt.lt(now)),
                    ),
            )
            .exec(&self.db)
            .await?;
        if result.rows_affected == 0 {
            let current = source_inbox::Entity::find_by_id(inbox_id)
                .one(&self.db)
                .await?
                .ok_or(NotificationError::InvalidEvent)?;
            if is_terminal_inbox(current.status) {
                return Ok(());
            }
            return Err(NotificationError::LeaseUnavailable);
        }
        Ok(())
    }

    async fn finish_inbox_terminal(
        &self,
        inbox: &source_inbox::Model,
        worker_id: &str,
        status: NotificationSourceInboxStatus,
        fanout_job_id: Option<Uuid>,
    ) -> NotificationResult<source_inbox::Model> {
        let now = Utc::now().into();
        let result = source_inbox::Entity::update_many()
            .set(source_inbox::ActiveModel {
                status: Set(status),
                fanout_job_id: Set(fanout_job_id),
                lease_owner: Set(None),
                lease_expires_at: Set(None),
                next_attempt_at: Set(None),
                last_error_code: Set(None),
                last_error_message: Set(None),
                completed_at: Set(Some(now)),
                updated_at: Set(now),
                ..Default::default()
            })
            .filter(source_inbox::Column::Id.eq(inbox.id))
            .filter(source_inbox::Column::Status.eq(NotificationSourceInboxStatus::Processing))
            .filter(source_inbox::Column::LeaseOwner.eq(worker_id))
            .exec(&self.db)
            .await?;
        if result.rows_affected != 1 {
            return Err(NotificationError::LeaseUnavailable);
        }
        source_inbox::Entity::find_by_id(inbox.id)
            .one(&self.db)
            .await?
            .ok_or(NotificationError::InvalidEvent)
    }

    async fn finish_inbox_error(
        &self,
        inbox: &source_inbox::Model,
        worker_id: &str,
        error: &NotificationError,
    ) -> NotificationResult<()> {
        let retryable = error.is_retryable();
        let now = Utc::now();
        let result = source_inbox::Entity::update_many()
            .set(source_inbox::ActiveModel {
                status: Set(if retryable {
                    NotificationSourceInboxStatus::RetryableError
                } else {
                    NotificationSourceInboxStatus::Rejected
                }),
                attempt_count: Set(inbox.attempt_count.saturating_add(1)),
                next_attempt_at: Set(retryable
                    .then_some((now + Duration::seconds(RETRY_DELAY_SECONDS)).into())),
                lease_owner: Set(None),
                lease_expires_at: Set(None),
                last_error_code: Set(Some(truncate(error.stable_code(), MAX_ERROR_CODE_BYTES))),
                last_error_message: Set(Some(truncate(
                    &error.to_string(),
                    MAX_ERROR_MESSAGE_BYTES,
                ))),
                completed_at: Set((!retryable).then_some(now.into())),
                updated_at: Set(now.into()),
                ..Default::default()
            })
            .filter(source_inbox::Column::Id.eq(inbox.id))
            .filter(source_inbox::Column::Status.eq(NotificationSourceInboxStatus::Processing))
            .filter(source_inbox::Column::LeaseOwner.eq(worker_id))
            .exec(&self.db)
            .await?;
        if result.rows_affected != 1 {
            return Err(NotificationError::LeaseUnavailable);
        }
        Ok(())
    }

    async fn find_or_create_job(
        &self,
        event: &NotificationSourceEventRef,
        notification_type: &str,
        descriptor_json: serde_json::Value,
    ) -> NotificationResult<fanout_job::Model> {
        if let Some(existing) = self.find_job(event, notification_type).await? {
            ensure_job_identity(&existing, event, &descriptor_json)?;
            return Ok(existing);
        }
        let now = Utc::now().into();
        let candidate = fanout_job::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(event.tenant_id()),
            source_slug: Set(event.source().as_str().to_string()),
            source_event_id: Set(event.event_id()),
            source_revision: Set(source_revision_i64(event)?),
            notification_type: Set(notification_type.to_string()),
            descriptor_json: Set(descriptor_json.clone()),
            audience_cursor: Set(None),
            status: Set(NotificationJobStatus::Pending),
            attempt_count: Set(0),
            next_attempt_at: Set(None),
            lease_owner: Set(None),
            lease_expires_at: Set(None),
            last_error_code: Set(None),
            last_error_message: Set(None),
            completed_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        };
        match candidate.insert(&self.db).await {
            Ok(inserted) => Ok(inserted),
            Err(insert_error) => {
                if let Some(existing) = self.find_job(event, notification_type).await? {
                    ensure_job_identity(&existing, event, &descriptor_json)?;
                    Ok(existing)
                } else {
                    Err(insert_error.into())
                }
            }
        }
    }

    async fn find_job(
        &self,
        event: &NotificationSourceEventRef,
        notification_type: &str,
    ) -> NotificationResult<Option<fanout_job::Model>> {
        Ok(fanout_job::Entity::find()
            .filter(fanout_job::Column::TenantId.eq(event.tenant_id()))
            .filter(fanout_job::Column::SourceSlug.eq(event.source().as_str()))
            .filter(fanout_job::Column::SourceEventId.eq(event.event_id()))
            .filter(fanout_job::Column::NotificationType.eq(notification_type))
            .one(&self.db)
            .await?)
    }

    async fn claim_job(
        &self,
        job_id: Uuid,
        worker_id: &str,
    ) -> NotificationResult<fanout_job::Model> {
        let existing = fanout_job::Entity::find_by_id(job_id)
            .one(&self.db)
            .await?
            .ok_or(NotificationError::InvalidEvent)?;
        if existing.status == NotificationJobStatus::Completed {
            return Ok(existing);
        }
        let now = Utc::now();
        let result = fanout_job::Entity::update_many()
            .set(fanout_job::ActiveModel {
                status: Set(NotificationJobStatus::Leased),
                lease_owner: Set(Some(worker_id.to_string())),
                lease_expires_at: Set(Some(
                    (now + Duration::seconds(DEFAULT_LEASE_SECONDS)).into(),
                )),
                next_attempt_at: Set(None),
                completed_at: Set(None),
                updated_at: Set(now.into()),
                ..Default::default()
            })
            .filter(fanout_job::Column::Id.eq(job_id))
            .filter(
                Condition::any()
                    .add(fanout_job::Column::Status.eq(NotificationJobStatus::Pending))
                    .add(
                        Condition::all()
                            .add(
                                fanout_job::Column::Status
                                    .eq(NotificationJobStatus::RetryableError),
                            )
                            .add(
                                Condition::any()
                                    .add(fanout_job::Column::NextAttemptAt.is_null())
                                    .add(fanout_job::Column::NextAttemptAt.lte(now)),
                            ),
                    )
                    .add(
                        Condition::all()
                            .add(fanout_job::Column::Status.eq(NotificationJobStatus::Leased))
                            .add(fanout_job::Column::LeaseExpiresAt.lt(now)),
                    ),
            )
            .exec(&self.db)
            .await?;
        if result.rows_affected == 0 {
            let current = fanout_job::Entity::find_by_id(job_id)
                .one(&self.db)
                .await?
                .ok_or(NotificationError::InvalidEvent)?;
            if current.status == NotificationJobStatus::Completed {
                return Ok(current);
            }
            return Err(NotificationError::LeaseUnavailable);
        }
        fanout_job::Entity::find_by_id(job_id)
            .one(&self.db)
            .await?
            .ok_or(NotificationError::InvalidEvent)
    }

    async fn finish_job_error(
        &self,
        job: &fanout_job::Model,
        worker_id: &str,
        error: &NotificationError,
    ) -> NotificationResult<()> {
        let retryable = error.is_retryable();
        let now = Utc::now();
        let result = fanout_job::Entity::update_many()
            .set(fanout_job::ActiveModel {
                status: Set(if retryable {
                    NotificationJobStatus::RetryableError
                } else {
                    NotificationJobStatus::DeadLetter
                }),
                attempt_count: Set(job.attempt_count.saturating_add(1)),
                next_attempt_at: Set(retryable
                    .then_some((now + Duration::seconds(RETRY_DELAY_SECONDS)).into())),
                lease_owner: Set(None),
                lease_expires_at: Set(None),
                last_error_code: Set(Some(truncate(error.stable_code(), MAX_ERROR_CODE_BYTES))),
                last_error_message: Set(Some(truncate(
                    &error.to_string(),
                    MAX_ERROR_MESSAGE_BYTES,
                ))),
                completed_at: Set(None),
                updated_at: Set(now.into()),
                ..Default::default()
            })
            .filter(fanout_job::Column::Id.eq(job.id))
            .filter(fanout_job::Column::Status.eq(NotificationJobStatus::Leased))
            .filter(fanout_job::Column::LeaseOwner.eq(worker_id))
            .exec(&self.db)
            .await?;
        if result.rows_affected != 1 {
            return Err(NotificationError::LeaseUnavailable);
        }
        Ok(())
    }
}

fn validate_source_event(event: &NotificationSourceEventRef) -> NotificationResult<()> {
    if event.tenant_id().is_nil() || event.event_id().is_nil() {
        return Err(NotificationError::InvalidEvent);
    }
    let _ = source_revision_i64(event)?;
    Ok(())
}

fn validate_worker_id(worker_id: &str) -> NotificationResult<()> {
    if worker_id.trim().is_empty() || worker_id.len() > MAX_WORKER_ID_BYTES {
        return Err(NotificationError::Validation(format!(
            "worker id must contain between 1 and {MAX_WORKER_ID_BYTES} bytes"
        )));
    }
    Ok(())
}

fn validate_descriptor(
    event: &NotificationSourceEventRef,
    descriptor: &NotificationSemanticDescriptor,
) -> NotificationResult<()> {
    if &descriptor.notification_type != event.event_type()
        || descriptor.target.id.is_nil()
        || descriptor.target.owner != *event.source()
    {
        return Err(NotificationError::InvalidDescriptor);
    }
    Ok(())
}

fn source_revision_i64(event: &NotificationSourceEventRef) -> NotificationResult<i64> {
    i64::try_from(event.source_revision()).map_err(|_| NotificationError::InvalidEvent)
}

fn event_from_inbox(inbox: &source_inbox::Model) -> NotificationResult<NotificationSourceEventRef> {
    NotificationSourceEventRef::new(
        inbox.tenant_id,
        inbox.source_event_id,
        NotificationSourceSlug::new(inbox.source_slug.clone())
            .map_err(|_| NotificationError::InvalidEvent)?,
        NotificationTypeKey::new(inbox.event_type.clone())
            .map_err(|_| NotificationError::InvalidEvent)?,
        u64::try_from(inbox.source_revision).map_err(|_| NotificationError::InvalidEvent)?,
    )
    .map_err(|_| NotificationError::InvalidEvent)
}

fn event_from_job(job: &fanout_job::Model) -> NotificationResult<NotificationSourceEventRef> {
    NotificationSourceEventRef::new(
        job.tenant_id,
        job.source_event_id,
        NotificationSourceSlug::new(job.source_slug.clone())
            .map_err(|_| NotificationError::InvalidEvent)?,
        NotificationTypeKey::new(job.notification_type.clone())
            .map_err(|_| NotificationError::InvalidEvent)?,
        u64::try_from(job.source_revision).map_err(|_| NotificationError::InvalidEvent)?,
    )
    .map_err(|_| NotificationError::InvalidEvent)
}

fn ensure_inbox_identity(
    inbox: &source_inbox::Model,
    event: &NotificationSourceEventRef,
) -> NotificationResult<()> {
    if inbox.source_revision != source_revision_i64(event)? {
        return Err(NotificationError::SourceIdentityConflict);
    }
    Ok(())
}

fn ensure_job_identity(
    job: &fanout_job::Model,
    event: &NotificationSourceEventRef,
    descriptor_json: &serde_json::Value,
) -> NotificationResult<()> {
    if job.source_revision != source_revision_i64(event)? || &job.descriptor_json != descriptor_json {
        return Err(NotificationError::SourceIdentityConflict);
    }
    Ok(())
}

fn ensure_job_lease(job: &fanout_job::Model, worker_id: &str) -> NotificationResult<()> {
    if job.status != NotificationJobStatus::Leased
        || job.lease_owner.as_deref() != Some(worker_id)
    {
        return Err(NotificationError::LeaseUnavailable);
    }
    Ok(())
}

fn is_terminal_inbox(status: NotificationSourceInboxStatus) -> bool {
    matches!(
        status,
        NotificationSourceInboxStatus::Completed
            | NotificationSourceInboxStatus::Suppressed
            | NotificationSourceInboxStatus::Rejected
    )
}

fn receipt(inbox: source_inbox::Model, replayed: bool) -> NotificationSourceInboxReceipt {
    NotificationSourceInboxReceipt {
        inbox_id: inbox.id,
        status: inbox.status,
        fanout_job_id: inbox.fanout_job_id,
        replayed,
    }
}

fn truncate(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
}
