use std::sync::Arc;

use chrono::{DateTime, FixedOffset, Utc};
use rustok_notifications_api::NotificationSourceRegistry;
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::{fanout_job, source_inbox};
use crate::error::{NotificationError, NotificationResult};
use crate::fanout::{
    NotificationFanoutPageResult, NotificationFanoutService, NotificationSourceInboxReceipt,
};
use crate::model::{NotificationJobStatus, NotificationSourceInboxStatus};

pub const DEFAULT_NOTIFICATION_FANOUT_BATCH_SIZE: usize = 32;
pub const MAX_NOTIFICATION_FANOUT_BATCH_SIZE: usize = 64;
pub const DEFAULT_NOTIFICATION_FANOUT_PAGE_SIZE: u16 = 256;
pub const MAX_NOTIFICATION_FANOUT_PAGE_SIZE: u16 = 256;
const MAX_WORKER_ID_BYTES: usize = 191;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationFanoutSourceWorkItem {
    pub inbox_id: Uuid,
    pub tenant_id: Uuid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationFanoutJobWorkItem {
    pub job_id: Uuid,
    pub tenant_id: Uuid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationFanoutWorkerStage {
    SourceInbox,
    FanoutJob,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationFanoutWorkerFailure {
    pub stage: NotificationFanoutWorkerStage,
    pub record_id: Uuid,
    pub error_code: String,
    pub retryable: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationFanoutWorkerBatchResult {
    pub source_selected: usize,
    pub source_completed: usize,
    pub source_replayed: usize,
    pub jobs_selected: usize,
    pub pages_processed: usize,
    pub jobs_completed: usize,
    pub lease_conflicts: usize,
    pub failures: Vec<NotificationFanoutWorkerFailure>,
}

#[derive(Clone)]
pub struct NotificationFanoutWorker {
    db: DatabaseConnection,
    service: NotificationFanoutService,
    worker_id: String,
    batch_size: usize,
    page_size: u16,
}

impl std::fmt::Debug for NotificationFanoutWorker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NotificationFanoutWorker")
            .field("worker_id", &self.worker_id)
            .field("batch_size", &self.batch_size)
            .field("page_size", &self.page_size)
            .finish_non_exhaustive()
    }
}

impl NotificationFanoutWorker {
    pub fn new(
        db: DatabaseConnection,
        registry: Arc<NotificationSourceRegistry>,
        worker_id: impl Into<String>,
        batch_size: usize,
        page_size: u16,
    ) -> NotificationResult<Self> {
        let worker_id = worker_id.into();
        validate_worker_id(&worker_id)?;
        if !(1..=MAX_NOTIFICATION_FANOUT_BATCH_SIZE).contains(&batch_size) {
            return Err(NotificationError::Validation(format!(
                "fanout worker batch size must contain between 1 and {MAX_NOTIFICATION_FANOUT_BATCH_SIZE} items"
            )));
        }
        if !(1..=MAX_NOTIFICATION_FANOUT_PAGE_SIZE).contains(&page_size) {
            return Err(NotificationError::Validation(format!(
                "fanout worker page size must contain between 1 and {MAX_NOTIFICATION_FANOUT_PAGE_SIZE} recipients"
            )));
        }
        Ok(Self {
            service: NotificationFanoutService::new(db.clone(), registry),
            db,
            worker_id,
            batch_size,
            page_size,
        })
    }

    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    pub const fn batch_size(&self) -> usize {
        self.batch_size
    }

    pub const fn page_size(&self) -> u16 {
        self.page_size
    }

    /// Selects one bounded stable page of source inbox work. Selection itself
    /// never acquires a lease; tenant identity is exposed for host policy gating.
    pub async fn claimable_source_inbox_work(
        &self,
    ) -> NotificationResult<Vec<NotificationFanoutSourceWorkItem>> {
        let timestamp = now();
        let rows = source_inbox::Entity::find()
            .filter(claimable_source_condition(timestamp))
            .order_by_asc(source_inbox::Column::CreatedAt)
            .order_by_asc(source_inbox::Column::Id)
            .limit(self.batch_size as u64)
            .all(&self.db)
            .await?;
        Ok(rows
            .into_iter()
            .map(|row| NotificationFanoutSourceWorkItem {
                inbox_id: row.id,
                tenant_id: row.tenant_id,
            })
            .collect())
    }

    /// Selects one bounded stable page of fanout-job work. Selection itself never
    /// acquires a lease; tenant identity is exposed for host policy gating.
    pub async fn claimable_fanout_job_work(
        &self,
    ) -> NotificationResult<Vec<NotificationFanoutJobWorkItem>> {
        let timestamp = now();
        let rows = fanout_job::Entity::find()
            .filter(claimable_job_condition(timestamp))
            .order_by_asc(fanout_job::Column::CreatedAt)
            .order_by_asc(fanout_job::Column::Id)
            .limit(self.batch_size as u64)
            .all(&self.db)
            .await?;
        Ok(rows
            .into_iter()
            .map(|row| NotificationFanoutJobWorkItem {
                job_id: row.id,
                tenant_id: row.tenant_id,
            })
            .collect())
    }

    pub async fn materialize_source_inbox(
        &self,
        inbox_id: Uuid,
    ) -> NotificationResult<NotificationSourceInboxReceipt> {
        self.service
            .materialize_source_event(inbox_id, self.worker_id.as_str())
            .await
    }

    pub async fn process_fanout_job(
        &self,
        job_id: Uuid,
    ) -> NotificationResult<NotificationFanoutPageResult> {
        self.service
            .process_fanout_page(job_id, self.worker_id.as_str(), self.page_size)
            .await
    }

    /// Convenience bounded path for trusted callers that have already established
    /// tenant capability. Executable hosts must gate each work item before calling
    /// the canonical processing methods.
    pub async fn process_next_batch(
        &self,
    ) -> NotificationResult<NotificationFanoutWorkerBatchResult> {
        let source_work = self.claimable_source_inbox_work().await?;
        let mut result = NotificationFanoutWorkerBatchResult {
            source_selected: source_work.len(),
            ..NotificationFanoutWorkerBatchResult::default()
        };
        for work in source_work {
            match self.materialize_source_inbox(work.inbox_id).await {
                Ok(receipt) => {
                    result.source_completed += 1;
                    if receipt.replayed {
                        result.source_replayed += 1;
                    }
                }
                Err(NotificationError::LeaseUnavailable) => result.lease_conflicts += 1,
                Err(error) => result.failures.push(NotificationFanoutWorkerFailure {
                    stage: NotificationFanoutWorkerStage::SourceInbox,
                    record_id: work.inbox_id,
                    error_code: error.stable_code().to_string(),
                    retryable: error.is_retryable(),
                }),
            }
        }

        let job_work = self.claimable_fanout_job_work().await?;
        result.jobs_selected = job_work.len();
        for work in job_work {
            match self.process_fanout_job(work.job_id).await {
                Ok(page) => {
                    result.pages_processed += 1;
                    if page.completed {
                        result.jobs_completed += 1;
                    }
                }
                Err(NotificationError::LeaseUnavailable) => result.lease_conflicts += 1,
                Err(error) => result.failures.push(NotificationFanoutWorkerFailure {
                    stage: NotificationFanoutWorkerStage::FanoutJob,
                    record_id: work.job_id,
                    error_code: error.stable_code().to_string(),
                    retryable: error.is_retryable(),
                }),
            }
        }
        Ok(result)
    }
}

fn claimable_source_condition(timestamp: DateTime<FixedOffset>) -> Condition {
    Condition::any()
        .add(source_inbox::Column::Status.eq(NotificationSourceInboxStatus::Pending))
        .add(
            Condition::all()
                .add(
                    source_inbox::Column::Status.eq(NotificationSourceInboxStatus::RetryableError),
                )
                .add(
                    Condition::any()
                        .add(source_inbox::Column::NextAttemptAt.is_null())
                        .add(source_inbox::Column::NextAttemptAt.lte(timestamp.clone())),
                ),
        )
        .add(
            Condition::all()
                .add(source_inbox::Column::Status.eq(NotificationSourceInboxStatus::Processing))
                .add(source_inbox::Column::LeaseExpiresAt.lt(timestamp)),
        )
}

fn claimable_job_condition(timestamp: DateTime<FixedOffset>) -> Condition {
    Condition::any()
        .add(fanout_job::Column::Status.eq(NotificationJobStatus::Pending))
        .add(
            Condition::all()
                .add(fanout_job::Column::Status.eq(NotificationJobStatus::RetryableError))
                .add(
                    Condition::any()
                        .add(fanout_job::Column::NextAttemptAt.is_null())
                        .add(fanout_job::Column::NextAttemptAt.lte(timestamp.clone())),
                ),
        )
        .add(
            Condition::all()
                .add(fanout_job::Column::Status.eq(NotificationJobStatus::Leased))
                .add(fanout_job::Column::LeaseExpiresAt.lt(timestamp)),
        )
}

fn validate_worker_id(worker_id: &str) -> NotificationResult<()> {
    if worker_id.trim().is_empty() || worker_id.len() > MAX_WORKER_ID_BYTES {
        return Err(NotificationError::Validation(format!(
            "worker id must contain between 1 and {MAX_WORKER_ID_BYTES} bytes"
        )));
    }
    Ok(())
}

fn now() -> DateTime<FixedOffset> {
    Utc::now().fixed_offset()
}
