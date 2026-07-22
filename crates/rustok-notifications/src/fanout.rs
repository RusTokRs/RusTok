use std::sync::Arc;

use chrono::{DateTime, Duration, FixedOffset, Utc};
use rustok_core::ModuleRuntimeExtensions;
use rustok_notifications_api::{
    DescribeNotificationRequest, NotificationAudienceCursor, NotificationSemanticDescriptor,
    NotificationSourceEventRef, NotificationSourceProvider, NotificationSourceRegistry,
    NotificationSourceSlug, NotificationTypeKey, ResolveNotificationAudienceRequest,
    notification_source_registry_from_extensions,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, DatabaseConnection, EntityTrait,
    IntoActiveModel, QueryFilter, TransactionTrait,
    sea_query::OnConflict,
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

    /// Durably accepts one source event independently of provider availability.
    ///
    /// A later materialization pass performs provider discovery. This preserves
    /// the event while an optional source factory is temporarily unavailable.
    pub async fn enqueue_source_event(
        &self,
        event: NotificationSourceEventRef,
    ) -> NotificationResult<NotificationSourceInboxReceipt> {
        validate_source_event(&event)?;

        if let Some(existing) = self.find_inbox(&event).await? {
            ensure_inbox_identity(&existing, &event)?;
            return Ok(receipt(existing, true));
        }

        let timestamp = now();
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
            created_at: Set(timestamp),
            updated_at: Set(timestamp),
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

    /// Describes a durably accepted source event and creates its fan-out job.
    pub async fn materialize_source_event(
        &self,
        inbox_id: Uuid,
        worker_id: &str,
    ) -> NotificationResult<NotificationSourceInboxReceipt> {
        validate_worker_id(worker_id)?;
        let initial = source_inbox::Entity::find_by_id(inbox_id)
            .one(&self.db)
            .await?
            .ok_or(NotificationError::InvalidEvent)?;
        if is_terminal_inbox(initial.status) {
            return Ok(receipt(initial, true));
        }

        let inbox = self.claim_inbox(inbox_id, worker_id).await?;
        if is_terminal_inbox(inbox.status) {
            return Ok(receipt(inbox, true));
        }

        let event = match event_from_inbox(&inbox) {
            Ok(event) => event,
            Err(error) => return self.fail_inbox(&inbox, worker_id, error).await,
        };
        let provider = match self.provider_for_event(&event) {
            Ok(provider) => provider,
            Err(error) => return self.fail_inbox(&inbox, worker_id, error).await,
        };
        let descriptor = match provider
            .describe_event(DescribeNotificationRequest {
                event: event.clone(),
            })
            .await
        {
            Ok(descriptor) => descriptor,
            Err(provider_error) => {
                return self
                    .fail_inbox(&inbox, worker_id, NotificationError::from(provider_error))
                    .await;
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
        if let Err(error) = validate_descriptor(&event, &descriptor) {
            return self.fail_inbox(&inbox, worker_id, error).await;
        }
        let descriptor_json = match serde_json::to_value(&descriptor) {
            Ok(value) => value,
            Err(error) => {
                return self
                    .fail_inbox(&inbox, worker_id, NotificationError::Serialization(error))
                    .await;
            }
        };
        match serde_json::to_vec(&descriptor_json) {
            Ok(bytes) if bytes.len() <= MAX_DESCRIPTOR_BYTES => {}
            Ok(_) => {
                return self
                    .fail_inbox(&inbox, worker_id, NotificationError::InvalidDescriptor)
                    .await;
            }
            Err(error) => {
                return self
                    .fail_inbox(&inbox, worker_id, NotificationError::Serialization(error))
                    .await;
            }
        }

        let job = match self
            .find_or_create_job(&event, descriptor.notification_type.as_str(), descriptor_json)
            .await
        {
            Ok(job) => job,
            Err(error) => return self.fail_inbox(&inbox, worker_id, error).await,
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

    /// Resolves and persists one bounded page of candidate recipients.
    ///
    /// Candidate items deliberately remain `pending`. Preference, privacy,
    /// blocking and channel policy must run before a final notification row is
    /// created by a later owner command.
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

        let event = match self.load_event_for_job(&claimed).await {
            Ok(event) => event,
            Err(error) => return self.fail_job(&claimed, worker_id, error).await,
        };
        let descriptor: NotificationSemanticDescriptor =
            match serde_json::from_value(claimed.descriptor_json.clone()) {
                Ok(descriptor) => descriptor,
                Err(_) => {
                    return self
                        .fail_job(&claimed, worker_id, NotificationError::InvalidDescriptor)
                        .await;
                }
            };
        if let Err(error) = validate_descriptor(&event, &descriptor) {
            return self.fail_job(&claimed, worker_id, error).await;
        }
        let provider = match self.provider_for_event(&event) {
            Ok(provider) => provider,
            Err(error) => return self.fail_job(&claimed, worker_id, error).await,
        };
        let cursor = match claimed
            .audience_cursor
            .as_ref()
            .map(|cursor| NotificationAudienceCursor::new(cursor.clone()))
            .transpose()
        {
            Ok(cursor) => cursor,
            Err(_) => {
                return self
                    .fail_job(&claimed, worker_id, NotificationError::InvalidEvent)
                    .await;
            }
        };

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
                return self
                    .fail_job(&claimed, worker_id, NotificationError::from(provider_error))
                    .await;
            }
        };
        let (recipients, next_cursor) = page.into_parts();
        let next_cursor = next_cursor.map(|cursor| cursor.as_str().to_string());
        if recipients.len() > usize::from(limit)
            || (recipients.is_empty() && next_cursor.is_some())
        {
            return self
                .fail_job(&claimed, worker_id, NotificationError::ProviderRejected)
                .await;
        }
        if next_cursor.is_some() && next_cursor == claimed.audience_cursor {
            return self
                .fail_job(&claimed, worker_id, NotificationError::CursorDidNotAdvance)
                .await;
        }

        match self
            .persist_fanout_page(&claimed, worker_id, &recipients, next_cursor.clone())
            .await
        {
            Ok(inserted_items) => Ok(NotificationFanoutPageResult {
                job_id,
                candidates: recipients.len(),
                inserted_items,
                completed: next_cursor.is_none(),
                next_cursor,
            }),
            Err(NotificationError::LeaseUnavailable) => Err(NotificationError::LeaseUnavailable),
            Err(error) => self.fail_job(&claimed, worker_id, error).await,
        }
    }

    fn provider_for_event(
        &self,
        event: &NotificationSourceEventRef,
    ) -> NotificationResult<Arc<dyn NotificationSourceProvider>> {
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
            .one(&self.db)
            .await?)
    }

    async fn claim_inbox(
        &self,
        inbox_id: Uuid,
        worker_id: &str,
    ) -> NotificationResult<source_inbox::Model> {
        let current = source_inbox::Entity::find_by_id(inbox_id)
            .one(&self.db)
            .await?
            .ok_or(NotificationError::InvalidEvent)?;
        if is_terminal_inbox(current.status) {
            return Ok(current);
        }

        let timestamp = now();
        let result = source_inbox::Entity::update_many()
            .set(source_inbox::ActiveModel {
                status: Set(NotificationSourceInboxStatus::Processing),
                lease_owner: Set(Some(worker_id.to_string())),
                lease_expires_at: Set(Some(
                    timestamp + Duration::seconds(DEFAULT_LEASE_SECONDS),
                )),
                next_attempt_at: Set(None),
                completed_at: Set(None),
                updated_at: Set(timestamp),
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
                                    .add(source_inbox::Column::NextAttemptAt.lte(timestamp)),
                            ),
                    )
                    .add(
                        Condition::all()
                            .add(
                                source_inbox::Column::Status
                                    .eq(NotificationSourceInboxStatus::Processing),
                            )
                            .add(source_inbox::Column::LeaseExpiresAt.lt(timestamp)),
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
                return Ok(current);
            }
            return Err(NotificationError::LeaseUnavailable);
        }
        source_inbox::Entity::find_by_id(inbox_id)
            .one(&self.db)
            .await?
            .ok_or(NotificationError::InvalidEvent)
    }

    async fn fail_inbox<T>(
        &self,
        inbox: &source_inbox::Model,
        worker_id: &str,
        error: NotificationError,
    ) -> NotificationResult<T> {
        self.finish_inbox_error(inbox, worker_id, &error).await?;
        Err(error)
    }

    async fn finish_inbox_terminal(
        &self,
        inbox: &source_inbox::Model,
        worker_id: &str,
        status: NotificationSourceInboxStatus,
        fanout_job_id: Option<Uuid>,
    ) -> NotificationResult<source_inbox::Model> {
        let timestamp = now();
        let result = source_inbox::Entity::update_many()
            .set(source_inbox::ActiveModel {
                status: Set(status),
                fanout_job_id: Set(fanout_job_id),
                lease_owner: Set(None),
                lease_expires_at: Set(None),
                next_attempt_at: Set(None),
                last_error_code: Set(None),
                last_error_message: Set(None),
                completed_at: Set(Some(timestamp)),
                updated_at: Set(timestamp),
                ..Default::default()
            })
            .filter(source_inbox::Column::Id.eq(inbox.id))
            .filter(source_inbox::Column::Status.eq(NotificationSourceInboxStatus::Processing))
            .filter(source_inbox::Column::LeaseOwner.eq(worker_id))
            .filter(source_inbox::Column::LeaseExpiresAt.gt(timestamp))
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
        let timestamp = now();
        let result = source_inbox::Entity::update_many()
            .set(source_inbox::ActiveModel {
                status: Set(if retryable {
                    NotificationSourceInboxStatus::RetryableError
                } else {
                    NotificationSourceInboxStatus::Rejected
                }),
                attempt_count: Set(inbox.attempt_count.saturating_add(1)),
                next_attempt_at: Set(
                    retryable.then_some(timestamp + Duration::seconds(RETRY_DELAY_SECONDS)),
                ),
                lease_owner: Set(None),
                lease_expires_at: Set(None),
                last_error_code: Set(Some(truncate(error.stable_code(), MAX_ERROR_CODE_BYTES))),
                last_error_message: Set(Some(truncate(
                    &error.to_string(),
                    MAX_ERROR_MESSAGE_BYTES,
                ))),
                completed_at: Set((!retryable).then_some(timestamp)),
                updated_at: Set(timestamp),
                ..Default::default()
            })
            .filter(source_inbox::Column::Id.eq(inbox.id))
            .filter(source_inbox::Column::Status.eq(NotificationSourceInboxStatus::Processing))
            .filter(source_inbox::Column::LeaseOwner.eq(worker_id))
            .filter(source_inbox::Column::LeaseExpiresAt.gt(timestamp))
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
        if let Some(existing) = self.find_job(event).await? {
            ensure_job_identity(
                &existing,
                event,
                notification_type,
                &descriptor_json,
            )?;
            return Ok(existing);
        }
        let timestamp = now();
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
            created_at: Set(timestamp),
            updated_at: Set(timestamp),
        };
        match candidate.insert(&self.db).await {
            Ok(inserted) => Ok(inserted),
            Err(insert_error) => {
                if let Some(existing) = self.find_job(event).await? {
                    ensure_job_identity(
                        &existing,
                        event,
                        notification_type,
                        &descriptor_json,
                    )?;
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
    ) -> NotificationResult<Option<fanout_job::Model>> {
        Ok(fanout_job::Entity::find()
            .filter(fanout_job::Column::TenantId.eq(event.tenant_id()))
            .filter(fanout_job::Column::SourceSlug.eq(event.source().as_str()))
            .filter(fanout_job::Column::SourceEventId.eq(event.event_id()))
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
        let timestamp = now();
        let result = fanout_job::Entity::update_many()
            .set(fanout_job::ActiveModel {
                status: Set(NotificationJobStatus::Leased),
                lease_owner: Set(Some(worker_id.to_string())),
                lease_expires_at: Set(Some(
                    timestamp + Duration::seconds(DEFAULT_LEASE_SECONDS),
                )),
                next_attempt_at: Set(None),
                completed_at: Set(None),
                updated_at: Set(timestamp),
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
                                    .add(fanout_job::Column::NextAttemptAt.lte(timestamp)),
                            ),
                    )
                    .add(
                        Condition::all()
                            .add(fanout_job::Column::Status.eq(NotificationJobStatus::Leased))
                            .add(fanout_job::Column::LeaseExpiresAt.lt(timestamp)),
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

    async fn load_event_for_job(
        &self,
        job: &fanout_job::Model,
    ) -> NotificationResult<NotificationSourceEventRef> {
        let inbox = source_inbox::Entity::find()
            .filter(source_inbox::Column::TenantId.eq(job.tenant_id))
            .filter(source_inbox::Column::FanoutJobId.eq(job.id))
            .one(&self.db)
            .await?
            .ok_or(NotificationError::InvalidEvent)?;
        event_from_inbox(&inbox)
    }

    async fn persist_fanout_page(
        &self,
        claimed: &fanout_job::Model,
        worker_id: &str,
        recipients: &[rustok_notifications_api::NotificationAudienceCandidate],
        next_cursor: Option<String>,
    ) -> NotificationResult<usize> {
        let txn = self.db.begin().await?;
        let current = fanout_job::Entity::find_by_id(claimed.id)
            .one(&txn)
            .await?
            .ok_or(NotificationError::InvalidEvent)?;
        ensure_job_lease(&current, worker_id)?;

        let mut inserted_items = 0usize;
        for candidate in recipients {
            let timestamp = now();
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
                created_at: Set(timestamp),
                updated_at: Set(timestamp),
                processed_at: Set(None),
            };
            let inserted = fanout_item::Entity::insert(item)
                .on_conflict(
                    OnConflict::columns([
                        fanout_item::Column::TenantId,
                        fanout_item::Column::FanoutJobId,
                        fanout_item::Column::RecipientId,
                    ])
                    .do_nothing()
                    .to_owned(),
                )
                .exec_without_returning(&txn)
                .await?;
            inserted_items = inserted_items.saturating_add(inserted as usize);
        }

        let complete = next_cursor.is_none();
        let timestamp = now();
        let mut update = current.into_active_model();
        update.audience_cursor = Set(next_cursor);
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
        update.completed_at = Set(complete.then_some(timestamp));
        update.updated_at = Set(timestamp);
        update.update(&txn).await?;
        txn.commit().await?;
        Ok(inserted_items)
    }

    async fn fail_job<T>(
        &self,
        job: &fanout_job::Model,
        worker_id: &str,
        error: NotificationError,
    ) -> NotificationResult<T> {
        self.finish_job_error(job, worker_id, &error).await?;
        Err(error)
    }

    async fn finish_job_error(
        &self,
        job: &fanout_job::Model,
        worker_id: &str,
        error: &NotificationError,
    ) -> NotificationResult<()> {
        let retryable = error.is_retryable();
        let timestamp = now();
        let result = fanout_job::Entity::update_many()
            .set(fanout_job::ActiveModel {
                status: Set(if retryable {
                    NotificationJobStatus::RetryableError
                } else {
                    NotificationJobStatus::DeadLetter
                }),
                attempt_count: Set(job.attempt_count.saturating_add(1)),
                next_attempt_at: Set(
                    retryable.then_some(timestamp + Duration::seconds(RETRY_DELAY_SECONDS)),
                ),
                lease_owner: Set(None),
                lease_expires_at: Set(None),
                last_error_code: Set(Some(truncate(error.stable_code(), MAX_ERROR_CODE_BYTES))),
                last_error_message: Set(Some(truncate(
                    &error.to_string(),
                    MAX_ERROR_MESSAGE_BYTES,
                ))),
                completed_at: Set(None),
                updated_at: Set(timestamp),
                ..Default::default()
            })
            .filter(fanout_job::Column::Id.eq(job.id))
            .filter(fanout_job::Column::Status.eq(NotificationJobStatus::Leased))
            .filter(fanout_job::Column::LeaseOwner.eq(worker_id))
            .filter(fanout_job::Column::LeaseExpiresAt.gt(timestamp))
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
    if descriptor.target.id.is_nil()
        || descriptor.target.owner.as_str() != event.source().as_str()
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

fn ensure_inbox_identity(
    inbox: &source_inbox::Model,
    event: &NotificationSourceEventRef,
) -> NotificationResult<()> {
    if inbox.source_revision != source_revision_i64(event)?
        || inbox.event_type != event.event_type().as_str()
    {
        return Err(NotificationError::SourceIdentityConflict);
    }
    Ok(())
}

fn ensure_job_identity(
    job: &fanout_job::Model,
    event: &NotificationSourceEventRef,
    notification_type: &str,
    descriptor_json: &serde_json::Value,
) -> NotificationResult<()> {
    if job.source_revision != source_revision_i64(event)?
        || job.notification_type != notification_type
        || &job.descriptor_json != descriptor_json
    {
        return Err(NotificationError::SourceIdentityConflict);
    }
    Ok(())
}

fn ensure_job_lease(job: &fanout_job::Model, worker_id: &str) -> NotificationResult<()> {
    if job.status != NotificationJobStatus::Leased
        || job.lease_owner.as_deref() != Some(worker_id)
        || job
            .lease_expires_at
            .as_ref()
            .is_none_or(|expires_at| expires_at <= &now())
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

fn now() -> DateTime<FixedOffset> {
    Utc::now().fixed_offset()
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
