use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Duration, FixedOffset, Utc};
use rustok_notifications_api::{
    AuthorizeNotificationTargetRequest, NotificationOpenAuthorization, NotificationPriority,
    NotificationSemanticDescriptor, NotificationSourceRegistry, NotificationSourceSlug,
    NotificationTargetRef,
};
use sea_orm::{
    ActiveValue::Set, ColumnTrait, Condition, ConnectionTrait, DatabaseConnection,
    DatabaseTransaction, EntityTrait, QueryFilter, TransactionTrait, sea_query::OnConflict,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::{fanout_job, notification, preference};
use crate::error::{NotificationError, NotificationResult};
use crate::model::{
    FanoutItemStatus, NotificationDeliveryMode, NotificationPriorityValue, NotificationState,
};

const DEFAULT_LEASE_SECONDS: i64 = 60;
const RETRY_DELAY_SECONDS: i64 = 30;
const MAX_WORKER_ID_BYTES: usize = 191;
const MAX_ERROR_CODE_BYTES: usize = 100;
const MAX_POLICY_REVISION_BYTES: usize = 128;
const MAX_DEFAULT_ENABLED_MODULES: usize = 512;
const MAX_MODULE_SLUG_BYTES: usize = 191;
const DEFAULT_SOURCE_SCOPE: &str = "*";
const DEFAULT_TYPE_SCOPE: &str = "*";
const NOTIFICATIONS_MODULE_SLUG: &str = "notifications";
const PREFERENCE_DISABLED_CODE: &str = "NOTIFICATION_PREFERENCE_DISABLED";
const SOURCE_TARGET_UNAVAILABLE_CODE: &str = "NOTIFICATION_SOURCE_TARGET_UNAVAILABLE";

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
#[serde(rename_all = "snake_case")]
pub enum NotificationRecipientSuppression {
    RecipientUnavailable,
    ProfileRestricted,
    Blocked,
    Muted,
    TenantRestricted,
}

impl NotificationRecipientSuppression {
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::RecipientUnavailable => "NOTIFICATION_RECIPIENT_UNAVAILABLE",
            Self::ProfileRestricted => "NOTIFICATION_PROFILE_RESTRICTED",
            Self::Blocked => "NOTIFICATION_RECIPIENT_BLOCKED",
            Self::Muted => "NOTIFICATION_RECIPIENT_MUTED",
            Self::TenantRestricted => "NOTIFICATION_TENANT_RESTRICTED",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum NotificationRecipientPolicyDecision {
    Allow,
    Suppress {
        reason: NotificationRecipientSuppression,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationRecipientPolicyError {
    pub retryable: bool,
}

impl NotificationRecipientPolicyError {
    pub const fn retryable() -> Self {
        Self { retryable: true }
    }

    pub const fn permanent() -> Self {
        Self { retryable: false }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationRecipientPolicyRequest {
    pub tenant_id: Uuid,
    pub recipient_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub source_slug: String,
    pub source_event_id: Uuid,
    pub source_revision: i64,
    pub notification_type: String,
    pub target: NotificationTargetRef,
}

#[async_trait]
pub trait NotificationRecipientPolicy: Send + Sync {
    async fn evaluate(
        &self,
        request: NotificationRecipientPolicyRequest,
    ) -> Result<NotificationRecipientPolicyDecision, NotificationRecipientPolicyError>;
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationTenantCapabilityCommitRequest {
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub observed_policy_revision: String,
    pub observed_default_enabled_modules: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationTenantCapabilityCommitDecision {
    Allow,
    Disabled,
    RevisionChanged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationTenantCapabilityCommitError {
    pub retryable: bool,
}

impl NotificationTenantCapabilityCommitError {
    pub const fn retryable() -> Self {
        Self { retryable: true }
    }

    pub const fn permanent() -> Self {
        Self { retryable: false }
    }
}

#[async_trait]
pub trait NotificationTenantCapabilityCommitGuard: Send + Sync {
    async fn evaluate(
        &self,
        transaction: &DatabaseTransaction,
        request: NotificationTenantCapabilityCommitRequest,
    ) -> Result<NotificationTenantCapabilityCommitDecision, NotificationTenantCapabilityCommitError>;
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationCandidateProcessResult {
    pub item_id: Uuid,
    pub status: FanoutItemStatus,
    pub notification_id: Option<Uuid>,
    pub replayed: bool,
}

#[derive(Clone)]
pub struct NotificationCandidateService {
    db: DatabaseConnection,
    registry: Arc<NotificationSourceRegistry>,
    policy: Arc<dyn NotificationRecipientPolicy>,
    commit_guard: Option<Arc<dyn NotificationTenantCapabilityCommitGuard>>,
}

impl NotificationCandidateService {
    pub fn new(
        db: DatabaseConnection,
        registry: Arc<NotificationSourceRegistry>,
        policy: Arc<dyn NotificationRecipientPolicy>,
    ) -> Self {
        Self {
            db,
            registry,
            policy,
            commit_guard: None,
        }
    }

    pub fn new_with_commit_guard(
        db: DatabaseConnection,
        registry: Arc<NotificationSourceRegistry>,
        policy: Arc<dyn NotificationRecipientPolicy>,
        commit_guard: Arc<dyn NotificationTenantCapabilityCommitGuard>,
    ) -> Self {
        Self {
            db,
            registry,
            policy,
            commit_guard: Some(commit_guard),
        }
    }

    /// Trusted compatibility path for callers that establish tenant capability
    /// outside this command. Production workers should use
    /// `process_candidate_with_policy_revision`.
    pub async fn process_candidate(
        &self,
        item_id: Uuid,
        worker_id: &str,
    ) -> NotificationResult<NotificationCandidateProcessResult> {
        self.process_candidate_inner(item_id, worker_id, None, None)
            .await
    }

    /// Processes one candidate with the exact effective-policy revision and
    /// manifest defaults observed before claim. The final transaction revalidates
    /// tenant overrides under the lifecycle cursor without opening another pool
    /// connection while the candidate transaction is active.
    pub async fn process_candidate_with_policy_revision(
        &self,
        item_id: Uuid,
        worker_id: &str,
        observed_policy_revision: &str,
        observed_default_enabled_modules: &[String],
    ) -> NotificationResult<NotificationCandidateProcessResult> {
        validate_policy_revision(observed_policy_revision)?;
        validate_default_enabled_modules(observed_default_enabled_modules)?;
        self.process_candidate_inner(
            item_id,
            worker_id,
            Some(observed_policy_revision),
            Some(observed_default_enabled_modules),
        )
        .await
    }

    async fn process_candidate_inner(
        &self,
        item_id: Uuid,
        worker_id: &str,
        observed_policy_revision: Option<&str>,
        observed_default_enabled_modules: Option<&[String]>,
    ) -> NotificationResult<NotificationCandidateProcessResult> {
        validate_worker_id(worker_id)?;
        let initial = candidate_item::Entity::find_by_id(item_id)
            .one(&self.db)
            .await?
            .ok_or(NotificationError::InvalidEvent)?;
        if is_terminal_candidate(initial.status) {
            return Ok(candidate_result(initial, true));
        }

        let item = self.claim_candidate(item_id, worker_id).await?;
        if is_terminal_candidate(item.status) {
            return Ok(candidate_result(item, true));
        }

        let job = match self.load_job(&item).await {
            Ok(job) => job,
            Err(error) => return self.fail_candidate(&item, worker_id, error).await,
        };
        let descriptor = match deserialize_descriptor(&job) {
            Ok(descriptor) => descriptor,
            Err(error) => return self.fail_candidate(&item, worker_id, error).await,
        };

        let preference_allows = match self.preference_allows_in_app(&self.db, &item, &job).await {
            Ok(allows) => allows,
            Err(error) => return self.fail_candidate(&item, worker_id, error).await,
        };
        if !preference_allows {
            return self
                .finish_candidate_skipped(&item, worker_id, PREFERENCE_DISABLED_CODE)
                .await;
        }

        let policy_request = NotificationRecipientPolicyRequest {
            tenant_id: item.tenant_id,
            recipient_id: item.recipient_id,
            actor_id: descriptor.actor_id,
            source_slug: job.source_slug.clone(),
            source_event_id: job.source_event_id,
            source_revision: job.source_revision,
            notification_type: job.notification_type.clone(),
            target: descriptor.target.clone(),
        };
        match self.policy.evaluate(policy_request).await {
            Ok(NotificationRecipientPolicyDecision::Allow) => {}
            Ok(NotificationRecipientPolicyDecision::Suppress { reason }) => {
                return self
                    .finish_candidate_skipped(&item, worker_id, reason.stable_code())
                    .await;
            }
            Err(error) => {
                return self
                    .fail_candidate(
                        &item,
                        worker_id,
                        NotificationError::RecipientPolicyFailure {
                            retryable: error.retryable,
                        },
                    )
                    .await;
            }
        }

        let source = match NotificationSourceSlug::new(job.source_slug.clone()) {
            Ok(source) => source,
            Err(_) => {
                return self
                    .fail_candidate(&item, worker_id, NotificationError::InvalidDescriptor)
                    .await;
            }
        };
        let provider = match self.registry.get(&source) {
            Some(provider) => provider,
            None => {
                return self
                    .fail_candidate(&item, worker_id, NotificationError::SourceUnavailable)
                    .await;
            }
        };
        let authorization = match provider
            .authorize_target_open(AuthorizeNotificationTargetRequest {
                tenant_id: item.tenant_id,
                recipient_id: item.recipient_id,
                target: descriptor.target.clone(),
            })
            .await
        {
            Ok(authorization) => authorization,
            Err(error) => {
                return self
                    .fail_candidate(&item, worker_id, NotificationError::from(error))
                    .await;
            }
        };
        if authorization == NotificationOpenAuthorization::Unavailable {
            return self
                .finish_candidate_skipped(&item, worker_id, SOURCE_TARGET_UNAVAILABLE_CODE)
                .await;
        }

        match self
            .persist_final_notification(
                &item,
                &job,
                descriptor,
                worker_id,
                observed_policy_revision,
                observed_default_enabled_modules,
            )
            .await
        {
            Ok(result) => Ok(result),
            Err(NotificationError::LeaseUnavailable) => Err(NotificationError::LeaseUnavailable),
            Err(error) => self.fail_candidate(&item, worker_id, error).await,
        }
    }

    async fn load_job(
        &self,
        item: &candidate_item::Model,
    ) -> NotificationResult<fanout_job::Model> {
        fanout_job::Entity::find_by_id(item.fanout_job_id)
            .filter(fanout_job::Column::TenantId.eq(item.tenant_id))
            .one(&self.db)
            .await?
            .ok_or(NotificationError::InvalidEvent)
    }

    async fn preference_allows_in_app<C>(
        &self,
        connection: &C,
        item: &candidate_item::Model,
        job: &fanout_job::Model,
    ) -> NotificationResult<bool>
    where
        C: ConnectionTrait,
    {
        let rows = preference::Entity::find()
            .filter(preference::Column::TenantId.eq(item.tenant_id))
            .filter(preference::Column::UserId.eq(item.recipient_id))
            .filter(
                Condition::any()
                    .add(preference::Column::SourceScope.eq(job.source_slug.as_str()))
                    .add(preference::Column::SourceScope.eq(DEFAULT_SOURCE_SCOPE)),
            )
            .filter(
                Condition::any()
                    .add(preference::Column::TypeScope.eq(job.notification_type.as_str()))
                    .add(preference::Column::TypeScope.eq(DEFAULT_TYPE_SCOPE)),
            )
            .all(connection)
            .await?;

        let selected = rows.into_iter().max_by_key(|row| {
            preference_specificity(
                row.source_scope.as_str(),
                row.type_scope.as_str(),
                job.source_slug.as_str(),
                job.notification_type.as_str(),
            )
        });
        Ok(selected.is_none_or(|row| {
            row.delivery_mode != NotificationDeliveryMode::Off && row.in_app_enabled
        }))
    }

    async fn claim_candidate(
        &self,
        item_id: Uuid,
        worker_id: &str,
    ) -> NotificationResult<candidate_item::Model> {
        let current = candidate_item::Entity::find_by_id(item_id)
            .one(&self.db)
            .await?
            .ok_or(NotificationError::InvalidEvent)?;
        if is_terminal_candidate(current.status) {
            return Ok(current);
        }

        let timestamp = now();
        let result = candidate_item::Entity::update_many()
            .set(candidate_item::ActiveModel {
                status: Set(FanoutItemStatus::Processing),
                lease_owner: Set(Some(worker_id.to_string())),
                lease_expires_at: Set(Some(timestamp + Duration::seconds(DEFAULT_LEASE_SECONDS))),
                next_attempt_at: Set(None),
                processed_at: Set(None),
                notification_id: Set(None),
                updated_at: Set(timestamp),
                ..Default::default()
            })
            .filter(candidate_item::Column::Id.eq(item_id))
            .filter(
                Condition::any()
                    .add(candidate_item::Column::Status.eq(FanoutItemStatus::Pending))
                    .add(
                        Condition::all()
                            .add(
                                candidate_item::Column::Status.eq(FanoutItemStatus::RetryableError),
                            )
                            .add(
                                Condition::any()
                                    .add(candidate_item::Column::NextAttemptAt.is_null())
                                    .add(candidate_item::Column::NextAttemptAt.lte(timestamp)),
                            ),
                    )
                    .add(
                        Condition::all()
                            .add(candidate_item::Column::Status.eq(FanoutItemStatus::Processing))
                            .add(candidate_item::Column::LeaseExpiresAt.lt(timestamp)),
                    ),
            )
            .exec(&self.db)
            .await?;
        if result.rows_affected == 0 {
            let current = candidate_item::Entity::find_by_id(item_id)
                .one(&self.db)
                .await?
                .ok_or(NotificationError::InvalidEvent)?;
            if is_terminal_candidate(current.status) {
                return Ok(current);
            }
            return Err(NotificationError::LeaseUnavailable);
        }
        candidate_item::Entity::find_by_id(item_id)
            .one(&self.db)
            .await?
            .ok_or(NotificationError::InvalidEvent)
    }

    async fn finish_candidate_skipped(
        &self,
        item: &candidate_item::Model,
        worker_id: &str,
        reason_code: &str,
    ) -> NotificationResult<NotificationCandidateProcessResult> {
        let timestamp = now();
        let result = candidate_item::Entity::update_many()
            .set(candidate_item::ActiveModel {
                status: Set(FanoutItemStatus::Skipped),
                notification_id: Set(None),
                last_error_code: Set(Some(truncate(reason_code, MAX_ERROR_CODE_BYTES))),
                next_attempt_at: Set(None),
                lease_owner: Set(None),
                lease_expires_at: Set(None),
                processed_at: Set(Some(timestamp)),
                updated_at: Set(timestamp),
                ..Default::default()
            })
            .filter(candidate_item::Column::Id.eq(item.id))
            .filter(candidate_item::Column::Status.eq(FanoutItemStatus::Processing))
            .filter(candidate_item::Column::LeaseOwner.eq(worker_id))
            .filter(candidate_item::Column::LeaseExpiresAt.gt(timestamp))
            .exec(&self.db)
            .await?;
        if result.rows_affected != 1 {
            return Err(NotificationError::LeaseUnavailable);
        }
        let completed = candidate_item::Entity::find_by_id(item.id)
            .one(&self.db)
            .await?
            .ok_or(NotificationError::InvalidEvent)?;
        Ok(candidate_result(completed, false))
    }

    async fn fail_candidate<T>(
        &self,
        item: &candidate_item::Model,
        worker_id: &str,
        error: NotificationError,
    ) -> NotificationResult<T> {
        self.finish_candidate_error(item, worker_id, &error).await?;
        Err(error)
    }

    async fn finish_candidate_error(
        &self,
        item: &candidate_item::Model,
        worker_id: &str,
        error: &NotificationError,
    ) -> NotificationResult<()> {
        let retryable = error.is_retryable();
        let timestamp = now();
        let result = candidate_item::Entity::update_many()
            .set(candidate_item::ActiveModel {
                status: Set(if retryable {
                    FanoutItemStatus::RetryableError
                } else {
                    FanoutItemStatus::Failed
                }),
                notification_id: Set(None),
                attempt_count: Set(item.attempt_count.saturating_add(1)),
                next_attempt_at: Set(
                    retryable.then_some(timestamp + Duration::seconds(RETRY_DELAY_SECONDS))
                ),
                lease_owner: Set(None),
                lease_expires_at: Set(None),
                last_error_code: Set(Some(truncate(error.stable_code(), MAX_ERROR_CODE_BYTES))),
                processed_at: Set((!retryable).then_some(timestamp)),
                updated_at: Set(timestamp),
                ..Default::default()
            })
            .filter(candidate_item::Column::Id.eq(item.id))
            .filter(candidate_item::Column::Status.eq(FanoutItemStatus::Processing))
            .filter(candidate_item::Column::LeaseOwner.eq(worker_id))
            .filter(candidate_item::Column::LeaseExpiresAt.gt(timestamp))
            .exec(&self.db)
            .await?;
        if result.rows_affected != 1 {
            return Err(NotificationError::LeaseUnavailable);
        }
        Ok(())
    }

    async fn persist_final_notification(
        &self,
        item: &candidate_item::Model,
        job: &fanout_job::Model,
        descriptor: NotificationSemanticDescriptor,
        worker_id: &str,
        observed_policy_revision: Option<&str>,
        observed_default_enabled_modules: Option<&[String]>,
    ) -> NotificationResult<NotificationCandidateProcessResult> {
        let txn = self.db.begin().await?;
        let current = candidate_item::Entity::find_by_id(item.id)
            .one(&txn)
            .await?
            .ok_or(NotificationError::InvalidEvent)?;
        ensure_candidate_lease(&current, worker_id)?;

        if let Some(observed_policy_revision) = observed_policy_revision {
            let Some(commit_guard) = self.commit_guard.as_ref() else {
                txn.rollback().await?;
                return Err(NotificationError::TenantPolicyCommitFailure { retryable: true });
            };
            let Some(observed_default_enabled_modules) = observed_default_enabled_modules else {
                txn.rollback().await?;
                return Err(NotificationError::TenantPolicyCommitFailure { retryable: false });
            };
            let request = NotificationTenantCapabilityCommitRequest {
                tenant_id: current.tenant_id,
                module_slug: NOTIFICATIONS_MODULE_SLUG.to_string(),
                observed_policy_revision: observed_policy_revision.to_string(),
                observed_default_enabled_modules: observed_default_enabled_modules.to_vec(),
            };
            match commit_guard.evaluate(&txn, request).await {
                Ok(NotificationTenantCapabilityCommitDecision::Allow) => {}
                Ok(NotificationTenantCapabilityCommitDecision::Disabled) => {
                    txn.rollback().await?;
                    return Err(NotificationError::TenantCapabilityDisabled);
                }
                Ok(NotificationTenantCapabilityCommitDecision::RevisionChanged) => {
                    txn.rollback().await?;
                    return Err(NotificationError::TenantPolicyRevisionChanged);
                }
                Err(error) => {
                    txn.rollback().await?;
                    return Err(NotificationError::TenantPolicyCommitFailure {
                        retryable: error.retryable,
                    });
                }
            }
        }

        if !self.preference_allows_in_app(&txn, &current, job).await? {
            let completion_time = now();
            let result = candidate_item::Entity::update_many()
                .set(candidate_item::ActiveModel {
                    status: Set(FanoutItemStatus::Skipped),
                    notification_id: Set(None),
                    last_error_code: Set(Some(PREFERENCE_DISABLED_CODE.to_string())),
                    next_attempt_at: Set(None),
                    lease_owner: Set(None),
                    lease_expires_at: Set(None),
                    processed_at: Set(Some(completion_time)),
                    updated_at: Set(completion_time),
                    ..Default::default()
                })
                .filter(candidate_item::Column::Id.eq(current.id))
                .filter(candidate_item::Column::Status.eq(FanoutItemStatus::Processing))
                .filter(candidate_item::Column::LeaseOwner.eq(worker_id))
                .filter(candidate_item::Column::LeaseExpiresAt.gt(completion_time))
                .exec(&txn)
                .await?;
            if result.rows_affected != 1 {
                txn.rollback().await?;
                return Err(NotificationError::LeaseUnavailable);
            }
            txn.commit().await?;
            return Ok(NotificationCandidateProcessResult {
                item_id: current.id,
                status: FanoutItemStatus::Skipped,
                notification_id: None,
                replayed: false,
            });
        }

        let timestamp = now();
        let template_data_json = serde_json::to_value(&descriptor.template_data)?;
        let active = notification::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(current.tenant_id),
            recipient_id: Set(current.recipient_id),
            source_slug: Set(job.source_slug.clone()),
            source_event_id: Set(job.source_event_id),
            source_revision: Set(job.source_revision),
            notification_type: Set(job.notification_type.clone()),
            template_key: Set(descriptor.template_key.as_str().to_string()),
            target_owner: Set(descriptor.target.owner.as_str().to_string()),
            target_kind: Set(descriptor.target.kind.as_str().to_string()),
            target_id: Set(descriptor.target.id),
            actor_id: Set(descriptor.actor_id),
            priority: Set(priority_value(descriptor.priority)),
            state: Set(NotificationState::Unread),
            template_data_json: Set(template_data_json.clone()),
            group_key: Set(None),
            idempotency_key: Set(format!("candidate:{}", current.id)),
            seen_at: Set(None),
            read_at: Set(None),
            archived_at: Set(None),
            created_at: Set(timestamp),
            updated_at: Set(timestamp),
        };
        notification::Entity::insert(active)
            .on_conflict(
                OnConflict::columns([
                    notification::Column::TenantId,
                    notification::Column::RecipientId,
                    notification::Column::SourceSlug,
                    notification::Column::SourceEventId,
                    notification::Column::NotificationType,
                ])
                .do_nothing()
                .to_owned(),
            )
            .exec_without_returning(&txn)
            .await?;

        let persisted = notification::Entity::find()
            .filter(notification::Column::TenantId.eq(current.tenant_id))
            .filter(notification::Column::RecipientId.eq(current.recipient_id))
            .filter(notification::Column::SourceSlug.eq(job.source_slug.as_str()))
            .filter(notification::Column::SourceEventId.eq(job.source_event_id))
            .filter(notification::Column::NotificationType.eq(job.notification_type.as_str()))
            .one(&txn)
            .await?
            .ok_or(NotificationError::SourceIdentityConflict)?;
        ensure_notification_identity(
            &persisted,
            job,
            &descriptor,
            &template_data_json,
            current.recipient_id,
        )?;

        let completion_time = now();
        let result = candidate_item::Entity::update_many()
            .set(candidate_item::ActiveModel {
                status: Set(FanoutItemStatus::Processed),
                notification_id: Set(Some(persisted.id)),
                last_error_code: Set(None),
                next_attempt_at: Set(None),
                lease_owner: Set(None),
                lease_expires_at: Set(None),
                processed_at: Set(Some(completion_time)),
                updated_at: Set(completion_time),
                ..Default::default()
            })
            .filter(candidate_item::Column::Id.eq(current.id))
            .filter(candidate_item::Column::Status.eq(FanoutItemStatus::Processing))
            .filter(candidate_item::Column::LeaseOwner.eq(worker_id))
            .filter(candidate_item::Column::LeaseExpiresAt.gt(completion_time))
            .exec(&txn)
            .await?;
        if result.rows_affected != 1 {
            txn.rollback().await?;
            return Err(NotificationError::LeaseUnavailable);
        }
        txn.commit().await?;

        Ok(NotificationCandidateProcessResult {
            item_id: current.id,
            status: FanoutItemStatus::Processed,
            notification_id: Some(persisted.id),
            replayed: false,
        })
    }
}

fn deserialize_descriptor(
    job: &fanout_job::Model,
) -> NotificationResult<NotificationSemanticDescriptor> {
    let descriptor =
        serde_json::from_value::<NotificationSemanticDescriptor>(job.descriptor_json.clone())
            .map_err(|_| NotificationError::InvalidDescriptor)?;
    if descriptor.notification_type.as_str() != job.notification_type
        || descriptor.target.owner.as_str() != job.source_slug
        || descriptor.target.id.is_nil()
    {
        return Err(NotificationError::InvalidDescriptor);
    }
    Ok(descriptor)
}

fn preference_specificity(
    source_scope: &str,
    type_scope: &str,
    source_slug: &str,
    notification_type: &str,
) -> u8 {
    let source_score = u8::from(source_scope == source_slug) * 2;
    let type_score = u8::from(type_scope == notification_type);
    source_score + type_score
}

fn ensure_candidate_lease(item: &candidate_item::Model, worker_id: &str) -> NotificationResult<()> {
    if item.status != FanoutItemStatus::Processing
        || item.lease_owner.as_deref() != Some(worker_id)
        || item
            .lease_expires_at
            .as_ref()
            .is_none_or(|expires_at| expires_at <= &now())
    {
        return Err(NotificationError::LeaseUnavailable);
    }
    Ok(())
}

fn ensure_notification_identity(
    existing: &notification::Model,
    job: &fanout_job::Model,
    descriptor: &NotificationSemanticDescriptor,
    template_data_json: &serde_json::Value,
    recipient_id: Uuid,
) -> NotificationResult<()> {
    if existing.tenant_id != job.tenant_id
        || existing.recipient_id != recipient_id
        || existing.source_slug != job.source_slug
        || existing.source_event_id != job.source_event_id
        || existing.source_revision != job.source_revision
        || existing.notification_type != job.notification_type
        || existing.template_key != descriptor.template_key.as_str()
        || existing.target_owner != descriptor.target.owner.as_str()
        || existing.target_kind != descriptor.target.kind.as_str()
        || existing.target_id != descriptor.target.id
        || existing.actor_id != descriptor.actor_id
        || existing.priority != priority_value(descriptor.priority)
        || &existing.template_data_json != template_data_json
    {
        return Err(NotificationError::SourceIdentityConflict);
    }
    Ok(())
}

fn priority_value(priority: NotificationPriority) -> NotificationPriorityValue {
    match priority {
        NotificationPriority::Low => NotificationPriorityValue::Low,
        NotificationPriority::Normal => NotificationPriorityValue::Normal,
        NotificationPriority::High => NotificationPriorityValue::High,
        NotificationPriority::Urgent => NotificationPriorityValue::Urgent,
    }
}

fn validate_worker_id(worker_id: &str) -> NotificationResult<()> {
    if worker_id.trim().is_empty() || worker_id.len() > MAX_WORKER_ID_BYTES {
        return Err(NotificationError::Validation(format!(
            "worker id must contain between 1 and {MAX_WORKER_ID_BYTES} bytes"
        )));
    }
    Ok(())
}

fn validate_policy_revision(policy_revision: &str) -> NotificationResult<()> {
    if policy_revision.trim().is_empty()
        || policy_revision != policy_revision.trim()
        || policy_revision.len() > MAX_POLICY_REVISION_BYTES
        || policy_revision.chars().any(char::is_control)
    {
        return Err(NotificationError::Validation(
            "observed tenant policy revision is invalid".to_string(),
        ));
    }
    Ok(())
}

fn validate_default_enabled_modules(default_enabled_modules: &[String]) -> NotificationResult<()> {
    if default_enabled_modules.len() > MAX_DEFAULT_ENABLED_MODULES
        || default_enabled_modules.iter().any(|module_slug| {
            module_slug.trim().is_empty()
                || module_slug != module_slug.trim()
                || module_slug.len() > MAX_MODULE_SLUG_BYTES
                || module_slug.chars().any(char::is_control)
        })
    {
        return Err(NotificationError::Validation(
            "observed default-enabled module set is invalid".to_string(),
        ));
    }
    Ok(())
}

fn is_terminal_candidate(status: FanoutItemStatus) -> bool {
    matches!(
        status,
        FanoutItemStatus::Processed | FanoutItemStatus::Skipped | FanoutItemStatus::Failed
    )
}

fn candidate_result(
    item: candidate_item::Model,
    replayed: bool,
) -> NotificationCandidateProcessResult {
    NotificationCandidateProcessResult {
        item_id: item.id,
        status: item.status,
        notification_id: item.notification_id,
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
