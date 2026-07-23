use std::sync::Arc;

use chrono::{DateTime, Duration, FixedOffset, Utc};
use rustok_notifications_api::NotificationSourceRegistry;
use sea_orm::{
    ActiveValue::Set, ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::candidate::{
    NotificationCandidateProcessResult, NotificationCandidateService, NotificationRecipientPolicy,
};
use crate::error::{NotificationError, NotificationResult};
use crate::model::FanoutItemStatus;

pub const DEFAULT_NOTIFICATION_CANDIDATE_BATCH_SIZE: usize = 32;
pub const MAX_NOTIFICATION_CANDIDATE_BATCH_SIZE: usize = 64;
const MAX_WORKER_ID_BYTES: usize = 191;
const TENANT_DISABLED_RETRY_SECONDS: i64 = 300;
const TENANT_POLICY_UNAVAILABLE_RETRY_SECONDS: i64 = 30;

mod candidate_item {
    use sea_orm::entity::prelude::*;

    use crate::model::FanoutItemStatus;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "notification_fanout_items")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub fanout_job_id: Uuid,
        pub recipient_id: Uuid,
        pub status: FanoutItemStatus,
        pub notification_id: Option<Uuid>,
        pub idempotency_key: String,
        pub last_error_code: Option<String>,
        pub attempt_count: i32,
        pub next_attempt_at: Option<DateTimeWithTimeZone>,
        pub lease_owner: Option<String>,
        pub lease_expires_at: Option<DateTimeWithTimeZone>,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
        pub processed_at: Option<DateTimeWithTimeZone>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationCandidateWorkItem {
    pub item_id: Uuid,
    pub tenant_id: Uuid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationCandidatePolicyDeferral {
    TenantDisabled,
    PolicyUnavailable,
}

impl NotificationCandidatePolicyDeferral {
    const fn retry_seconds(self) -> i64 {
        match self {
            Self::TenantDisabled => TENANT_DISABLED_RETRY_SECONDS,
            Self::PolicyUnavailable => TENANT_POLICY_UNAVAILABLE_RETRY_SECONDS,
        }
    }

    const fn error_code(self) -> &'static str {
        match self {
            Self::TenantDisabled => "NOTIFICATION_TENANT_CAPABILITY_DISABLED",
            Self::PolicyUnavailable => "NOTIFICATION_TENANT_POLICY_UNAVAILABLE",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationCandidateWorkerFailure {
    pub item_id: Uuid,
    pub error_code: String,
    pub retryable: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationCandidateBatchResult {
    pub selected: usize,
    pub completed: usize,
    pub replayed: usize,
    pub lease_conflicts: usize,
    pub failures: Vec<NotificationCandidateWorkerFailure>,
}

#[derive(Clone)]
pub struct NotificationCandidateWorker {
    db: DatabaseConnection,
    service: NotificationCandidateService,
    worker_id: String,
    batch_size: usize,
}

impl std::fmt::Debug for NotificationCandidateWorker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NotificationCandidateWorker")
            .field("worker_id", &self.worker_id)
            .field("batch_size", &self.batch_size)
            .finish_non_exhaustive()
    }
}

impl NotificationCandidateWorker {
    pub fn new(
        db: DatabaseConnection,
        registry: Arc<NotificationSourceRegistry>,
        policy: Arc<dyn NotificationRecipientPolicy>,
        worker_id: impl Into<String>,
        batch_size: usize,
    ) -> NotificationResult<Self> {
        let worker_id = worker_id.into();
        validate_worker_id(&worker_id)?;
        if !(1..=MAX_NOTIFICATION_CANDIDATE_BATCH_SIZE).contains(&batch_size) {
            return Err(NotificationError::Validation(format!(
                "candidate worker batch size must contain between 1 and {MAX_NOTIFICATION_CANDIDATE_BATCH_SIZE} items"
            )));
        }
        Ok(Self {
            service: NotificationCandidateService::new(db.clone(), registry, policy),
            db,
            worker_id,
            batch_size,
        })
    }

    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    pub const fn batch_size(&self) -> usize {
        self.batch_size
    }

    /// Selects at most one bounded page of currently claimable candidate work.
    /// Selection itself never acquires a lease; tenant identity is exposed so the
    /// executable host can enforce current capability before recipient/source calls.
    pub async fn claimable_candidate_work(
        &self,
    ) -> NotificationResult<Vec<NotificationCandidateWorkItem>> {
        let timestamp = now();
        let rows = candidate_item::Entity::find()
            .filter(claimable_condition(timestamp))
            .order_by_asc(candidate_item::Column::CreatedAt)
            .order_by_asc(candidate_item::Column::Id)
            .limit(self.batch_size as u64)
            .all(&self.db)
            .await?;
        Ok(rows
            .into_iter()
            .map(|row| NotificationCandidateWorkItem {
                item_id: row.id,
                tenant_id: row.tenant_id,
            })
            .collect())
    }

    /// Compatibility projection for trusted callers that already enforce tenant
    /// capability. Executable hosts should use `claimable_candidate_work`.
    pub async fn claimable_candidate_ids(&self) -> NotificationResult<Vec<Uuid>> {
        Ok(self
            .claimable_candidate_work()
            .await?
            .into_iter()
            .map(|work| work.item_id)
            .collect())
    }

    /// Defers claimable candidate work before canonical processing when current
    /// tenant capability is disabled or temporarily unresolved. The update is a
    /// compare-and-set against tenant, attempt count, and claimable state, so a
    /// concurrent canonical claim remains authoritative.
    pub async fn defer_candidate(
        &self,
        work: NotificationCandidateWorkItem,
        reason: NotificationCandidatePolicyDeferral,
    ) -> NotificationResult<()> {
        let current = candidate_item::Entity::find_by_id(work.item_id)
            .one(&self.db)
            .await?
            .ok_or(NotificationError::InvalidEvent)?;
        if current.tenant_id != work.tenant_id {
            return Err(NotificationError::SourceIdentityConflict);
        }

        let timestamp = now();
        let result = candidate_item::Entity::update_many()
            .set(candidate_item::ActiveModel {
                status: Set(FanoutItemStatus::RetryableError),
                notification_id: Set(None),
                attempt_count: Set(current.attempt_count.saturating_add(1)),
                next_attempt_at: Set(Some(
                    timestamp.clone() + Duration::seconds(reason.retry_seconds()),
                )),
                lease_owner: Set(None),
                lease_expires_at: Set(None),
                last_error_code: Set(Some(reason.error_code().to_string())),
                processed_at: Set(None),
                updated_at: Set(timestamp.clone()),
                ..Default::default()
            })
            .filter(candidate_item::Column::Id.eq(work.item_id))
            .filter(candidate_item::Column::TenantId.eq(work.tenant_id))
            .filter(candidate_item::Column::AttemptCount.eq(current.attempt_count))
            .filter(claimable_condition(timestamp))
            .exec(&self.db)
            .await?;
        if result.rows_affected != 1 {
            return Err(NotificationError::LeaseUnavailable);
        }
        Ok(())
    }

    /// Claims and processes one candidate through the canonical service lease CAS.
    pub async fn process_candidate(
        &self,
        item_id: Uuid,
    ) -> NotificationResult<NotificationCandidateProcessResult> {
        self.service
            .process_candidate(item_id, self.worker_id.as_str())
            .await
    }

    /// Convenience bounded batch path for trusted callers that have already
    /// established tenant capability. Deployment-owned loops should use
    /// `claimable_candidate_work`, check shutdown and tenant capability between
    /// items, then call `process_candidate`.
    pub async fn process_next_batch(&self) -> NotificationResult<NotificationCandidateBatchResult> {
        let work_items = self.claimable_candidate_work().await?;
        let mut result = NotificationCandidateBatchResult {
            selected: work_items.len(),
            ..NotificationCandidateBatchResult::default()
        };

        for work in work_items {
            match self.process_candidate(work.item_id).await {
                Ok(processed) => {
                    result.completed += 1;
                    if processed.replayed {
                        result.replayed += 1;
                    }
                }
                Err(NotificationError::LeaseUnavailable) => {
                    result.lease_conflicts += 1;
                }
                Err(error) => result.failures.push(NotificationCandidateWorkerFailure {
                    item_id: work.item_id,
                    error_code: error.stable_code().to_string(),
                    retryable: error.is_retryable(),
                }),
            }
        }

        Ok(result)
    }
}

fn claimable_condition(timestamp: DateTime<FixedOffset>) -> Condition {
    Condition::any()
        .add(candidate_item::Column::Status.eq(FanoutItemStatus::Pending))
        .add(
            Condition::all()
                .add(candidate_item::Column::Status.eq(FanoutItemStatus::RetryableError))
                .add(
                    Condition::any()
                        .add(candidate_item::Column::NextAttemptAt.is_null())
                        .add(candidate_item::Column::NextAttemptAt.lte(timestamp.clone())),
                ),
        )
        .add(
            Condition::all()
                .add(candidate_item::Column::Status.eq(FanoutItemStatus::Processing))
                .add(candidate_item::Column::LeaseExpiresAt.lt(timestamp)),
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
