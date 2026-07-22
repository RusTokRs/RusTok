use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rustok_api::{normalize_locale_tag, PortActorKind, PortCallPolicy, PortContext, PortError};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, DbBackend, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::application_entities::{
    membership_application, membership_policy, membership_policy_translation,
};
use crate::domain::{
    GroupJoinPolicy, GroupMembershipStatus, GroupRole, GroupStatus, GroupVisibility,
};
use crate::dto::GroupMembership;
use crate::entities::{group, membership};
use crate::error::{GroupsError, GroupsResult};
use crate::governance_entities::{audit_entry, command_receipt};

const UPSERT_POLICY_COMMAND: &str = "groups.upsert_membership_application_policy.v1";
const SUBMIT_APPLICATION_COMMAND: &str = "groups.submit_membership_application.v1";
const REVIEW_APPLICATION_COMMAND: &str = "groups.review_membership_application.v1";
const MAX_POLICY_QUESTIONS: usize = 20;
const MAX_POLICY_RULES: usize = 20;
const MAX_REVIEW_NOTE_CHARS: usize = 2_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupApplicationStatus {
    Pending,
    Approved,
    Rejected,
    Cancelled,
}

impl GroupApplicationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Cancelled => "cancelled",
        }
    }
}

impl FromStr for GroupApplicationStatus {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "pending" => Ok(Self::Pending),
            "approved" => Ok(Self::Approved),
            "rejected" => Ok(Self::Rejected),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(format!("unsupported GroupApplicationStatus value: {value}")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupApplicationReviewDecision {
    Approve,
    Reject,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupApplicationQuestion {
    pub key: String,
    pub prompt: String,
    pub help_text: Option<String>,
    pub required: bool,
    pub max_answer_chars: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupApplicationRule {
    pub key: String,
    pub title: String,
    pub body: String,
    pub required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupApplicationPolicy {
    pub id: Uuid,
    pub group_id: Uuid,
    pub revision: u64,
    pub enabled: bool,
    pub locale: String,
    pub questions: Vec<GroupApplicationQuestion>,
    pub rules: Vec<GroupApplicationRule>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpsertGroupApplicationPolicyRequest {
    pub group_id: Uuid,
    pub locale: String,
    pub enabled: bool,
    pub questions: Vec<GroupApplicationQuestion>,
    pub rules: Vec<GroupApplicationRule>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpsertGroupApplicationPolicyResult {
    pub policy: GroupApplicationPolicy,
    pub group_version: u64,
    pub created: bool,
    pub replayed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadGroupApplicationPolicyRequest {
    pub group_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitGroupMembershipApplicationRequest {
    pub group_id: Uuid,
    pub answers: BTreeMap<String, String>,
    pub acknowledged_rule_keys: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupMembershipApplication {
    pub id: Uuid,
    pub group_id: Uuid,
    pub user_id: Uuid,
    pub policy_id: Uuid,
    pub policy_revision: u64,
    pub policy_locale: String,
    pub questions: Vec<GroupApplicationQuestion>,
    pub rules: Vec<GroupApplicationRule>,
    pub answers: BTreeMap<String, String>,
    pub acknowledged_rule_keys: Vec<String>,
    pub status: GroupApplicationStatus,
    pub submitted_at: DateTime<Utc>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub reviewed_by_user_id: Option<Uuid>,
    pub review_note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitGroupMembershipApplicationResult {
    pub application: GroupMembershipApplication,
    pub membership: GroupMembership,
    pub group_version: u64,
    pub replayed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListGroupMembershipApplicationsRequest {
    pub group_id: Uuid,
    pub status: Option<GroupApplicationStatus>,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupMembershipApplicationConnection {
    pub items: Vec<GroupMembershipApplication>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewGroupMembershipApplicationRequest {
    pub application_id: Uuid,
    pub decision: GroupApplicationReviewDecision,
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewGroupMembershipApplicationResult {
    pub application: GroupMembershipApplication,
    pub membership: GroupMembership,
    pub group_version: u64,
    pub replayed: bool,
}

#[async_trait]
pub trait GroupApplicationReadPort: Send + Sync {
    async fn read_group_application_policy(
        &self,
        context: PortContext,
        request: ReadGroupApplicationPolicyRequest,
    ) -> Result<GroupApplicationPolicy, PortError>;

    async fn list_group_membership_applications(
        &self,
        context: PortContext,
        request: ListGroupMembershipApplicationsRequest,
    ) -> Result<GroupMembershipApplicationConnection, PortError>;
}

#[async_trait]
pub trait GroupApplicationCommandPort: Send + Sync {
    async fn upsert_group_application_policy(
        &self,
        context: PortContext,
        request: UpsertGroupApplicationPolicyRequest,
    ) -> Result<UpsertGroupApplicationPolicyResult, PortError>;

    async fn submit_group_membership_application(
        &self,
        context: PortContext,
        request: SubmitGroupMembershipApplicationRequest,
    ) -> Result<SubmitGroupMembershipApplicationResult, PortError>;

    async fn review_group_membership_application(
        &self,
        context: PortContext,
        request: ReviewGroupMembershipApplicationRequest,
    ) -> Result<ReviewGroupMembershipApplicationResult, PortError>;
}

#[derive(Clone)]
pub struct GroupApplicationService {
    db: DatabaseConnection,
}

impl GroupApplicationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn read_policy_owned(
        &self,
        context: &PortContext,
        request: ReadGroupApplicationPolicyRequest,
    ) -> GroupsResult<GroupApplicationPolicy> {
        require_read(context)?;
        let tenant_id = context_tenant_id(context)?;
        let user_id = actor_user_id(context)?;
        let group_model = find_group(&self.db, tenant_id, request.group_id).await?;
        require_application_group(&group_model)?;
        ensure_not_banned(&self.db, tenant_id, request.group_id, user_id).await?;
        load_policy_for_locale(&self.db, tenant_id, request.group_id, &context.locale).await
    }

    async fn upsert_policy_owned(
        &self,
        context: &PortContext,
        mut request: UpsertGroupApplicationPolicyRequest,
    ) -> GroupsResult<UpsertGroupApplicationPolicyResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        request.locale = normalize_locale_tag(&request.locale)
            .ok_or_else(|| GroupsError::Validation("invalid application policy locale".to_string()))?;
        normalize_policy(&mut request.questions, &mut request.rules)?;
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;

        if let Some(mut replayed) = replay_receipt::<UpsertGroupApplicationPolicyResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            UPSERT_POLICY_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        let group_model = find_group_for_update(&transaction, tenant_id, request.group_id).await?;
        require_active_group(&group_model)?;
        authorize_policy_management(
            &transaction,
            context,
            tenant_id,
            request.group_id,
            actor_user_id,
        )
        .await?;

        let now = Utc::now();
        let existing_policy = membership_policy::Entity::find()
            .filter(membership_policy::Column::TenantId.eq(tenant_id))
            .filter(membership_policy::Column::GroupId.eq(request.group_id))
            .one(&transaction)
            .await?;
        let created = existing_policy.is_none();
        let policy_model = if let Some(existing) = existing_policy {
            let next_revision = existing.revision.saturating_add(1).max(1);
            let mut active: membership_policy::ActiveModel = existing.into();
            active.revision = Set(next_revision);
            active.enabled = Set(request.enabled);
            active.updated_by_user_id = Set(actor_user_id);
            active.updated_at = Set(now.fixed_offset());
            active.update(&transaction).await?
        } else {
            membership_policy::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                group_id: Set(request.group_id),
                revision: Set(1),
                enabled: Set(request.enabled),
                created_by_user_id: Set(actor_user_id),
                updated_by_user_id: Set(actor_user_id),
                created_at: Set(now.fixed_offset()),
                updated_at: Set(now.fixed_offset()),
            }
            .insert(&transaction)
            .await?
        };

        let existing_translation = membership_policy_translation::Entity::find()
            .filter(membership_policy_translation::Column::TenantId.eq(tenant_id))
            .filter(membership_policy_translation::Column::PolicyId.eq(policy_model.id))
            .filter(membership_policy_translation::Column::Locale.eq(request.locale.clone()))
            .one(&transaction)
            .await?;
        let questions_value = serde_json::to_value(&request.questions)
            .map_err(|error| GroupsError::Invariant(format!("application questions are not serializable: {error}")))?;
        let rules_value = serde_json::to_value(&request.rules)
            .map_err(|error| GroupsError::Invariant(format!("application rules are not serializable: {error}")))?;
        if let Some(existing) = existing_translation {
            let mut active: membership_policy_translation::ActiveModel = existing.into();
            active.questions = Set(questions_value);
            active.rules = Set(rules_value);
            active.updated_at = Set(now.fixed_offset());
            active.update(&transaction).await?;
        } else {
            membership_policy_translation::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                policy_id: Set(policy_model.id),
                locale: Set(request.locale.clone()),
                questions: Set(questions_value),
                rules: Set(rules_value),
                created_at: Set(now.fixed_offset()),
                updated_at: Set(now.fixed_offset()),
            }
            .insert(&transaction)
            .await?;
        }

        let group_version = increment_group_version(&transaction, group_model, now).await?;
        let result = UpsertGroupApplicationPolicyResult {
            policy: GroupApplicationPolicy {
                id: policy_model.id,
                group_id: request.group_id,
                revision: policy_model.revision.max(1) as u64,
                enabled: policy_model.enabled,
                locale: request.locale,
                questions: request.questions,
                rules: request.rules,
            },
            group_version,
            created,
            replayed: false,
        };
        append_audit(
            &transaction,
            context,
            tenant_id,
            request.group_id,
            actor_user_id,
            "group.membership_application_policy_upserted",
            None,
            json!({
                "policy_id": result.policy.id,
                "policy_revision": result.policy.revision,
                "locale": result.policy.locale,
                "enabled": result.policy.enabled,
                "question_count": result.policy.questions.len(),
                "rule_count": result.policy.rules.len(),
                "group_version": group_version
            }),
        )
        .await?;
        store_receipt(
            &transaction,
            tenant_id,
            request.group_id,
            actor_user_id,
            idempotency_key,
            UPSERT_POLICY_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }

    async fn submit_application_owned(
        &self,
        context: &PortContext,
        mut request: SubmitGroupMembershipApplicationRequest,
    ) -> GroupsResult<SubmitGroupMembershipApplicationResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        normalize_application_submission(&mut request)?;
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;

        if let Some(mut replayed) = replay_receipt::<SubmitGroupMembershipApplicationResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            SUBMIT_APPLICATION_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        let group_model = find_group_for_update(&transaction, tenant_id, request.group_id).await?;
        require_application_group(&group_model)?;
        let policy = load_policy_for_locale(
            &transaction,
            tenant_id,
            request.group_id,
            &context.locale,
        )
        .await?;
        if !policy.enabled {
            return Err(GroupsError::Conflict(
                "group membership applications are disabled".to_string(),
            ));
        }
        validate_submission(&policy, &request)?;

        let existing_membership = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(request.group_id))
            .filter(membership::Column::UserId.eq(actor_user_id))
            .one(&transaction)
            .await?;
        if existing_membership
            .as_ref()
            .is_some_and(|row| row.status == GroupMembershipStatus::Banned.as_str())
        {
            return Err(GroupsError::Forbidden(
                "group membership is banned".to_string(),
            ));
        }
        if existing_membership
            .as_ref()
            .is_some_and(|row| row.status == GroupMembershipStatus::Active.as_str())
        {
            return Err(GroupsError::Conflict(
                "user is already an active group member".to_string(),
            ));
        }

        let existing_application = membership_application::Entity::find()
            .filter(membership_application::Column::TenantId.eq(tenant_id))
            .filter(membership_application::Column::GroupId.eq(request.group_id))
            .filter(membership_application::Column::UserId.eq(actor_user_id))
            .one(&transaction)
            .await?;
        if existing_application
            .as_ref()
            .is_some_and(|row| row.status == GroupApplicationStatus::Pending.as_str())
        {
            return Err(GroupsError::Conflict(
                "a membership application is already pending".to_string(),
            ));
        }
        if existing_application
            .as_ref()
            .is_some_and(|row| row.status == GroupApplicationStatus::Approved.as_str())
        {
            return Err(GroupsError::Conflict(
                "membership application is already approved".to_string(),
            ));
        }

        let now = Utc::now();
        let membership_model = if let Some(existing) = existing_membership {
            let mut active: membership::ActiveModel = existing.into();
            active.role = Set(GroupRole::Member.as_str().to_string());
            active.status = Set(GroupMembershipStatus::Pending.as_str().to_string());
            active.joined_at = Set(None);
            active.left_at = Set(None);
            active.updated_at = Set(now.fixed_offset());
            active.update(&transaction).await?
        } else {
            membership::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                group_id: Set(request.group_id),
                user_id: Set(actor_user_id),
                role: Set(GroupRole::Member.as_str().to_string()),
                status: Set(GroupMembershipStatus::Pending.as_str().to_string()),
                invited_by_user_id: Set(None),
                joined_at: Set(None),
                left_at: Set(None),
                metadata: Set(json!({})),
                created_at: Set(now.fixed_offset()),
                updated_at: Set(now.fixed_offset()),
            }
            .insert(&transaction)
            .await?
        };

        let policy_snapshot = json!({
            "questions": policy.questions,
            "rules": policy.rules
        });
        let answers_value = serde_json::to_value(&request.answers)
            .map_err(|error| GroupsError::Invariant(format!("application answers are not serializable: {error}")))?;
        let acknowledged_value = serde_json::to_value(&request.acknowledged_rule_keys)
            .map_err(|error| GroupsError::Invariant(format!("application acknowledgements are not serializable: {error}")))?;
        let application_model = if let Some(existing) = existing_application {
            let mut active: membership_application::ActiveModel = existing.into();
            active.policy_id = Set(policy.id);
            active.policy_revision = Set(policy.revision as i64);
            active.policy_locale = Set(policy.locale.clone());
            active.policy_snapshot = Set(policy_snapshot);
            active.answers = Set(answers_value);
            active.acknowledged_rule_keys = Set(acknowledged_value);
            active.status = Set(GroupApplicationStatus::Pending.as_str().to_string());
            active.submitted_at = Set(now.fixed_offset());
            active.reviewed_at = Set(None);
            active.reviewed_by_user_id = Set(None);
            active.review_note = Set(None);
            active.updated_at = Set(now.fixed_offset());
            active.update(&transaction).await?
        } else {
            membership_application::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                group_id: Set(request.group_id),
                user_id: Set(actor_user_id),
                policy_id: Set(policy.id),
                policy_revision: Set(policy.revision as i64),
                policy_locale: Set(policy.locale.clone()),
                policy_snapshot: Set(policy_snapshot),
                answers: Set(answers_value),
                acknowledged_rule_keys: Set(acknowledged_value),
                status: Set(GroupApplicationStatus::Pending.as_str().to_string()),
                submitted_at: Set(now.fixed_offset()),
                reviewed_at: Set(None),
                reviewed_by_user_id: Set(None),
                review_note: Set(None),
                created_at: Set(now.fixed_offset()),
                updated_at: Set(now.fixed_offset()),
            }
            .insert(&transaction)
            .await?
        };

        let group_version = increment_group_version(&transaction, group_model, now).await?;
        let result = SubmitGroupMembershipApplicationResult {
            application: map_application(application_model)?,
            membership: map_membership(membership_model)?,
            group_version,
            replayed: false,
        };
        append_audit(
            &transaction,
            context,
            tenant_id,
            request.group_id,
            actor_user_id,
            "group.membership_application_submitted",
            Some(actor_user_id),
            json!({
                "application_id": result.application.id,
                "policy_id": result.application.policy_id,
                "policy_revision": result.application.policy_revision,
                "policy_locale": result.application.policy_locale,
                "group_version": group_version
            }),
        )
        .await?;
        store_receipt(
            &transaction,
            tenant_id,
            request.group_id,
            actor_user_id,
            idempotency_key,
            SUBMIT_APPLICATION_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }

    async fn list_applications_owned(
        &self,
        context: &PortContext,
        request: ListGroupMembershipApplicationsRequest,
    ) -> GroupsResult<GroupMembershipApplicationConnection> {
        require_read(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        authorize_application_review(
            &self.db,
            context,
            tenant_id,
            request.group_id,
            actor_user_id,
        )
        .await?;
        let page = request.page.max(1);
        let per_page = request.per_page.clamp(1, 100);
        let mut query = membership_application::Entity::find()
            .filter(membership_application::Column::TenantId.eq(tenant_id))
            .filter(membership_application::Column::GroupId.eq(request.group_id));
        if let Some(status) = request.status {
            query = query.filter(membership_application::Column::Status.eq(status.as_str()));
        }
        let paginator = query
            .order_by_desc(membership_application::Column::SubmittedAt)
            .paginate(&self.db, per_page);
        let total = paginator.num_items().await?;
        let items = paginator
            .fetch_page(page.saturating_sub(1))
            .await?
            .into_iter()
            .map(map_application)
            .collect::<GroupsResult<Vec<_>>>()?;
        Ok(GroupMembershipApplicationConnection {
            items,
            total,
            page,
            per_page,
        })
    }

    async fn review_application_owned(
        &self,
        context: &PortContext,
        mut request: ReviewGroupMembershipApplicationRequest,
    ) -> GroupsResult<ReviewGroupMembershipApplicationResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        request.note = normalize_optional_note(request.note)?;
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;

        if let Some(mut replayed) = replay_receipt::<ReviewGroupMembershipApplicationResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            REVIEW_APPLICATION_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        let application_model = find_application_for_update(
            &transaction,
            tenant_id,
            request.application_id,
        )
        .await?;
        if application_model.status != GroupApplicationStatus::Pending.as_str() {
            return Err(GroupsError::Conflict(
                "membership application is no longer pending".to_string(),
            ));
        }
        let group_model =
            find_group_for_update(&transaction, tenant_id, application_model.group_id).await?;
        require_active_group(&group_model)?;
        authorize_application_review(
            &transaction,
            context,
            tenant_id,
            application_model.group_id,
            actor_user_id,
        )
        .await?;
        let membership_model = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(application_model.group_id))
            .filter(membership::Column::UserId.eq(application_model.user_id))
            .one(&transaction)
            .await?
            .ok_or_else(|| GroupsError::Invariant("pending application membership is missing".to_string()))?;
        if membership_model.status == GroupMembershipStatus::Banned.as_str() {
            return Err(GroupsError::Forbidden(
                "group membership is banned".to_string(),
            ));
        }

        let now = Utc::now();
        let approved = request.decision == GroupApplicationReviewDecision::Approve;
        let mut membership_active: membership::ActiveModel = membership_model.into();
        membership_active.role = Set(GroupRole::Member.as_str().to_string());
        membership_active.status = Set(if approved {
            GroupMembershipStatus::Active.as_str().to_string()
        } else {
            GroupMembershipStatus::Left.as_str().to_string()
        });
        membership_active.joined_at = Set(approved.then_some(now.fixed_offset()));
        membership_active.left_at = Set((!approved).then_some(now.fixed_offset()));
        membership_active.updated_at = Set(now.fixed_offset());
        let membership_model = membership_active.update(&transaction).await?;

        let mut application_active: membership_application::ActiveModel = application_model.into();
        application_active.status = Set(if approved {
            GroupApplicationStatus::Approved.as_str().to_string()
        } else {
            GroupApplicationStatus::Rejected.as_str().to_string()
        });
        application_active.reviewed_at = Set(Some(now.fixed_offset()));
        application_active.reviewed_by_user_id = Set(Some(actor_user_id));
        application_active.review_note = Set(request.note.clone());
        application_active.updated_at = Set(now.fixed_offset());
        let application_model = application_active.update(&transaction).await?;

        let group_version = if approved {
            increment_group_membership_version(&transaction, group_model, now).await?
        } else {
            increment_group_version(&transaction, group_model, now).await?
        };
        let result = ReviewGroupMembershipApplicationResult {
            application: map_application(application_model)?,
            membership: map_membership(membership_model)?,
            group_version,
            replayed: false,
        };
        append_audit(
            &transaction,
            context,
            tenant_id,
            result.application.group_id,
            actor_user_id,
            if approved {
                "group.membership_application_approved"
            } else {
                "group.membership_application_rejected"
            },
            Some(result.application.user_id),
            json!({
                "application_id": result.application.id,
                "decision": if approved { "approve" } else { "reject" },
                "review_note_present": result.application.review_note.is_some(),
                "group_version": group_version
            }),
        )
        .await?;
        store_receipt(
            &transaction,
            tenant_id,
            result.application.group_id,
            actor_user_id,
            idempotency_key,
            REVIEW_APPLICATION_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }
}

#[async_trait]
impl GroupApplicationReadPort for GroupApplicationService {
    async fn read_group_application_policy(
        &self,
        context: PortContext,
        request: ReadGroupApplicationPolicyRequest,
    ) -> Result<GroupApplicationPolicy, PortError> {
        self.read_policy_owned(&context, request).await.map_err(Into::into)
    }

    async fn list_group_membership_applications(
        &self,
        context: PortContext,
        request: ListGroupMembershipApplicationsRequest,
    ) -> Result<GroupMembershipApplicationConnection, PortError> {
        self.list_applications_owned(&context, request)
            .await
            .map_err(Into::into)
    }
}

#[async_trait]
impl GroupApplicationCommandPort for GroupApplicationService {
    async fn upsert_group_application_policy(
        &self,
        context: PortContext,
        request: UpsertGroupApplicationPolicyRequest,
    ) -> Result<UpsertGroupApplicationPolicyResult, PortError> {
        self.upsert_policy_owned(&context, request)
            .await
            .map_err(Into::into)
    }

    async fn submit_group_membership_application(
        &self,
        context: PortContext,
        request: SubmitGroupMembershipApplicationRequest,
    ) -> Result<SubmitGroupMembershipApplicationResult, PortError> {
        self.submit_application_owned(&context, request)
            .await
            .map_err(Into::into)
    }

    async fn review_group_membership_application(
        &self,
        context: PortContext,
        request: ReviewGroupMembershipApplicationRequest,
    ) -> Result<ReviewGroupMembershipApplicationResult, PortError> {
        self.review_application_owned(&context, request)
            .await
            .map_err(Into::into)
    }
}

async fn load_policy_for_locale<C: sea_orm::ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    group_id: Uuid,
    locale: &str,
) -> GroupsResult<GroupApplicationPolicy> {
    let locale = normalize_locale_tag(locale)
        .ok_or_else(|| GroupsError::Validation("invalid effective locale".to_string()))?;
    let policy = membership_policy::Entity::find()
        .filter(membership_policy::Column::TenantId.eq(tenant_id))
        .filter(membership_policy::Column::GroupId.eq(group_id))
        .one(connection)
        .await?
        .ok_or_else(|| GroupsError::Conflict("group membership application policy is not configured".to_string()))?;
    let translation = membership_policy_translation::Entity::find()
        .filter(membership_policy_translation::Column::TenantId.eq(tenant_id))
        .filter(membership_policy_translation::Column::PolicyId.eq(policy.id))
        .filter(membership_policy_translation::Column::Locale.eq(locale.clone()))
        .one(connection)
        .await?
        .ok_or_else(|| GroupsError::Conflict("group membership application policy is unavailable for the effective locale".to_string()))?;
    let questions = serde_json::from_value::<Vec<GroupApplicationQuestion>>(translation.questions)
        .map_err(|error| GroupsError::Invariant(format!("invalid application question contract: {error}")))?;
    let rules = serde_json::from_value::<Vec<GroupApplicationRule>>(translation.rules)
        .map_err(|error| GroupsError::Invariant(format!("invalid application rule contract: {error}")))?;
    Ok(GroupApplicationPolicy {
        id: policy.id,
        group_id,
        revision: policy.revision.max(1) as u64,
        enabled: policy.enabled,
        locale,
        questions,
        rules,
    })
}

fn normalize_policy(
    questions: &mut Vec<GroupApplicationQuestion>,
    rules: &mut Vec<GroupApplicationRule>,
) -> GroupsResult<()> {
    if questions.len() > MAX_POLICY_QUESTIONS {
        return Err(GroupsError::Validation(format!(
            "membership policy may contain at most {MAX_POLICY_QUESTIONS} questions"
        )));
    }
    if rules.len() > MAX_POLICY_RULES {
        return Err(GroupsError::Validation(format!(
            "membership policy may contain at most {MAX_POLICY_RULES} rules"
        )));
    }
    let mut question_keys = BTreeSet::new();
    for question in questions {
        question.key = normalize_policy_key(&question.key)?;
        if !question_keys.insert(question.key.clone()) {
            return Err(GroupsError::Validation(
                "membership policy question keys must be unique".to_string(),
            ));
        }
        question.prompt = normalize_required_text(&question.prompt, "question prompt", 500)?;
        question.help_text = normalize_optional_bounded_text(question.help_text.take(), "question help text", 1_000)?;
        if !(1..=4_000).contains(&question.max_answer_chars) {
            return Err(GroupsError::Validation(
                "question max_answer_chars must be between 1 and 4000".to_string(),
            ));
        }
    }
    let mut rule_keys = BTreeSet::new();
    for rule in rules {
        rule.key = normalize_policy_key(&rule.key)?;
        if !rule_keys.insert(rule.key.clone()) {
            return Err(GroupsError::Validation(
                "membership policy rule keys must be unique".to_string(),
            ));
        }
        rule.title = normalize_required_text(&rule.title, "rule title", 240)?;
        rule.body = normalize_required_text(&rule.body, "rule body", 10_000)?;
    }
    Ok(())
}

fn normalize_application_submission(
    request: &mut SubmitGroupMembershipApplicationRequest,
) -> GroupsResult<()> {
    let mut answers = BTreeMap::new();
    for (key, value) in std::mem::take(&mut request.answers) {
        let key = normalize_policy_key(&key)?;
        if answers.insert(key, value.trim().to_string()).is_some() {
            return Err(GroupsError::Validation(
                "application answer keys must be unique".to_string(),
            ));
        }
    }
    request.answers = answers;
    let mut acknowledged = BTreeSet::new();
    for key in std::mem::take(&mut request.acknowledged_rule_keys) {
        acknowledged.insert(normalize_policy_key(&key)?);
    }
    request.acknowledged_rule_keys = acknowledged.into_iter().collect();
    Ok(())
}

fn validate_submission(
    policy: &GroupApplicationPolicy,
    request: &SubmitGroupMembershipApplicationRequest,
) -> GroupsResult<()> {
    let question_map = policy
        .questions
        .iter()
        .map(|question| (question.key.as_str(), question))
        .collect::<BTreeMap<_, _>>();
    for key in request.answers.keys() {
        if !question_map.contains_key(key.as_str()) {
            return Err(GroupsError::Validation(format!(
                "application answer references an unknown question: {key}"
            )));
        }
    }
    for question in &policy.questions {
        let answer = request.answers.get(&question.key).map(String::as_str).unwrap_or("");
        if question.required && answer.trim().is_empty() {
            return Err(GroupsError::Validation(format!(
                "an answer is required for question {}",
                question.key
            )));
        }
        if answer.chars().count() > question.max_answer_chars as usize {
            return Err(GroupsError::Validation(format!(
                "answer for question {} exceeds {} characters",
                question.key, question.max_answer_chars
            )));
        }
    }
    let acknowledged = request
        .acknowledged_rule_keys
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let rule_keys = policy
        .rules
        .iter()
        .map(|rule| rule.key.as_str())
        .collect::<BTreeSet<_>>();
    if acknowledged.iter().any(|key| !rule_keys.contains(key)) {
        return Err(GroupsError::Validation(
            "application acknowledgement references an unknown rule".to_string(),
        ));
    }
    for rule in &policy.rules {
        if rule.required && !acknowledged.contains(rule.key.as_str()) {
            return Err(GroupsError::Validation(format!(
                "rule acknowledgement is required: {}",
                rule.key
            )));
        }
    }
    Ok(())
}

fn map_application(model: membership_application::Model) -> GroupsResult<GroupMembershipApplication> {
    #[derive(Deserialize)]
    struct Snapshot {
        questions: Vec<GroupApplicationQuestion>,
        rules: Vec<GroupApplicationRule>,
    }
    let snapshot = serde_json::from_value::<Snapshot>(model.policy_snapshot)
        .map_err(|error| GroupsError::Invariant(format!("invalid application policy snapshot: {error}")))?;
    let answers = serde_json::from_value::<BTreeMap<String, String>>(model.answers)
        .map_err(|error| GroupsError::Invariant(format!("invalid application answers: {error}")))?;
    let acknowledged_rule_keys = serde_json::from_value::<Vec<String>>(model.acknowledged_rule_keys)
        .map_err(|error| GroupsError::Invariant(format!("invalid application acknowledgements: {error}")))?;
    Ok(GroupMembershipApplication {
        id: model.id,
        group_id: model.group_id,
        user_id: model.user_id,
        policy_id: model.policy_id,
        policy_revision: model.policy_revision.max(1) as u64,
        policy_locale: model.policy_locale,
        questions: snapshot.questions,
        rules: snapshot.rules,
        answers,
        acknowledged_rule_keys,
        status: GroupApplicationStatus::from_str(&model.status).map_err(GroupsError::Invariant)?,
        submitted_at: model.submitted_at.with_timezone(&Utc),
        reviewed_at: model.reviewed_at.map(|value| value.with_timezone(&Utc)),
        reviewed_by_user_id: model.reviewed_by_user_id,
        review_note: model.review_note,
    })
}

fn map_membership(model: membership::Model) -> GroupsResult<GroupMembership> {
    Ok(GroupMembership {
        id: model.id,
        group_id: model.group_id,
        user_id: model.user_id,
        role: GroupRole::from_str(&model.role).map_err(GroupsError::Invariant)?,
        status: GroupMembershipStatus::from_str(&model.status).map_err(GroupsError::Invariant)?,
    })
}

async fn find_group<C: sea_orm::ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    group_id: Uuid,
) -> GroupsResult<group::Model> {
    group::Entity::find()
        .filter(group::Column::TenantId.eq(tenant_id))
        .filter(group::Column::Id.eq(group_id))
        .one(connection)
        .await?
        .ok_or(GroupsError::NotFound)
}

async fn find_group_for_update(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    group_id: Uuid,
) -> GroupsResult<group::Model> {
    let query = || {
        group::Entity::find()
            .filter(group::Column::TenantId.eq(tenant_id))
            .filter(group::Column::Id.eq(group_id))
    };
    match transaction.get_database_backend() {
        DbBackend::Sqlite => query().one(transaction).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().one(transaction).await?,
    }
    .ok_or(GroupsError::NotFound)
}

async fn find_application_for_update(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    application_id: Uuid,
) -> GroupsResult<membership_application::Model> {
    let query = || {
        membership_application::Entity::find()
            .filter(membership_application::Column::TenantId.eq(tenant_id))
            .filter(membership_application::Column::Id.eq(application_id))
    };
    match transaction.get_database_backend() {
        DbBackend::Sqlite => query().one(transaction).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().one(transaction).await?,
    }
    .ok_or(GroupsError::NotFound)
}

fn require_application_group(model: &group::Model) -> GroupsResult<()> {
    require_active_group(model)?;
    if GroupJoinPolicy::from_str(&model.join_policy).map_err(GroupsError::Invariant)?
        != GroupJoinPolicy::Request
    {
        return Err(GroupsError::Conflict(
            "group does not accept membership applications".to_string(),
        ));
    }
    if GroupVisibility::from_str(&model.visibility).map_err(GroupsError::Invariant)?
        == GroupVisibility::Secret
    {
        return Err(GroupsError::NotFound);
    }
    Ok(())
}

fn require_active_group(model: &group::Model) -> GroupsResult<()> {
    if model.status == GroupStatus::Active.as_str() {
        Ok(())
    } else {
        Err(GroupsError::Conflict("group is not active".to_string()))
    }
}

async fn ensure_not_banned<C: sea_orm::ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    group_id: Uuid,
    user_id: Uuid,
) -> GroupsResult<()> {
    let banned = membership::Entity::find()
        .filter(membership::Column::TenantId.eq(tenant_id))
        .filter(membership::Column::GroupId.eq(group_id))
        .filter(membership::Column::UserId.eq(user_id))
        .filter(membership::Column::Status.eq(GroupMembershipStatus::Banned.as_str()))
        .one(connection)
        .await?
        .is_some();
    if banned {
        Err(GroupsError::Forbidden(
            "group membership is banned".to_string(),
        ))
    } else {
        Ok(())
    }
}

async fn authorize_policy_management<C: sea_orm::ConnectionTrait>(
    connection: &C,
    context: &PortContext,
    tenant_id: Uuid,
    group_id: Uuid,
    actor_user_id: Uuid,
) -> GroupsResult<()> {
    if has_platform_manage(context) {
        return Ok(());
    }
    let allowed = membership::Entity::find()
        .filter(membership::Column::TenantId.eq(tenant_id))
        .filter(membership::Column::GroupId.eq(group_id))
        .filter(membership::Column::UserId.eq(actor_user_id))
        .one(connection)
        .await?
        .filter(|row| row.status == GroupMembershipStatus::Active.as_str())
        .and_then(|row| GroupRole::from_str(&row.role).ok())
        .is_some_and(GroupRole::can_manage_settings);
    if allowed {
        Ok(())
    } else {
        Err(GroupsError::Forbidden(
            "group owner or administrator role is required".to_string(),
        ))
    }
}

async fn authorize_application_review<C: sea_orm::ConnectionTrait>(
    connection: &C,
    context: &PortContext,
    tenant_id: Uuid,
    group_id: Uuid,
    actor_user_id: Uuid,
) -> GroupsResult<()> {
    if has_platform_manage(context) {
        return Ok(());
    }
    let allowed = membership::Entity::find()
        .filter(membership::Column::TenantId.eq(tenant_id))
        .filter(membership::Column::GroupId.eq(group_id))
        .filter(membership::Column::UserId.eq(actor_user_id))
        .one(connection)
        .await?
        .filter(|row| row.status == GroupMembershipStatus::Active.as_str())
        .and_then(|row| GroupRole::from_str(&row.role).ok())
        .is_some_and(GroupRole::can_moderate);
    if allowed {
        Ok(())
    } else {
        Err(GroupsError::Forbidden(
            "group owner, administrator, or moderator role is required".to_string(),
        ))
    }
}

async fn increment_group_version(
    transaction: &DatabaseTransaction,
    group_model: group::Model,
    now: DateTime<Utc>,
) -> GroupsResult<u64> {
    let group_version = group_model.version.saturating_add(1).max(1) as u64;
    let mut active: group::ActiveModel = group_model.into();
    active.version = Set(group_version as i64);
    active.updated_at = Set(now.fixed_offset());
    active.update(transaction).await?;
    Ok(group_version)
}

async fn increment_group_membership_version(
    transaction: &DatabaseTransaction,
    group_model: group::Model,
    now: DateTime<Utc>,
) -> GroupsResult<u64> {
    let group_version = group_model.version.saturating_add(1).max(1) as u64;
    let member_count = group_model.member_count.saturating_add(1);
    let mut active: group::ActiveModel = group_model.into();
    active.member_count = Set(member_count);
    active.version = Set(group_version as i64);
    active.updated_at = Set(now.fixed_offset());
    active.update(transaction).await?;
    Ok(group_version)
}

async fn replay_receipt<T: DeserializeOwned>(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_user_id: Uuid,
    idempotency_key: &str,
    command_type: &str,
    request_hash: &str,
) -> GroupsResult<Option<T>> {
    let Some(receipt) = command_receipt::Entity::find()
        .filter(command_receipt::Column::TenantId.eq(tenant_id))
        .filter(command_receipt::Column::IdempotencyKey.eq(idempotency_key))
        .one(transaction)
        .await?
    else {
        return Ok(None);
    };
    if receipt.actor_user_id != actor_user_id
        || receipt.command_type != command_type
        || receipt.request_hash != request_hash
    {
        return Err(GroupsError::Conflict(
            "idempotency key was already used for another group command".to_string(),
        ));
    }
    serde_json::from_value(receipt.response)
        .map(Some)
        .map_err(|error| GroupsError::Invariant(format!("invalid group command receipt: {error}")))
}

async fn store_receipt<T: Serialize>(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    group_id: Uuid,
    actor_user_id: Uuid,
    idempotency_key: String,
    command_type: &str,
    request_hash: String,
    response: &T,
) -> GroupsResult<()> {
    command_receipt::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        group_id: Set(group_id),
        actor_user_id: Set(actor_user_id),
        idempotency_key: Set(idempotency_key),
        command_type: Set(command_type.to_string()),
        request_hash: Set(request_hash),
        response: Set(serde_json::to_value(response).map_err(|error| {
            GroupsError::Invariant(format!("group command response is not serializable: {error}"))
        })?),
        created_at: Set(Utc::now().fixed_offset()),
    }
    .insert(transaction)
    .await?;
    Ok(())
}

async fn append_audit(
    transaction: &DatabaseTransaction,
    context: &PortContext,
    tenant_id: Uuid,
    group_id: Uuid,
    actor_user_id: Uuid,
    action: &str,
    target_user_id: Option<Uuid>,
    details: Value,
) -> GroupsResult<()> {
    audit_entry::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        group_id: Set(group_id),
        actor_user_id: Set(Some(actor_user_id)),
        action: Set(action.to_string()),
        target_user_id: Set(target_user_id),
        details: Set(details),
        correlation_id: Set(context.correlation_id.clone()),
        created_at: Set(Utc::now().fixed_offset()),
    }
    .insert(transaction)
    .await?;
    Ok(())
}

fn request_hash<T: Serialize>(request: &T) -> GroupsResult<String> {
    let bytes = serde_json::to_vec(request).map_err(|error| {
        GroupsError::Validation(format!("group command request is not serializable: {error}"))
    })?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn normalize_policy_key(value: &str) -> GroupsResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty()
        || normalized.len() > 64
        || !normalized.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '-' | '_')
        })
    {
        return Err(GroupsError::Validation(
            "policy keys must contain 1 to 64 lowercase ASCII letters, digits, hyphens, or underscores"
                .to_string(),
        ));
    }
    Ok(normalized)
}

fn normalize_required_text(value: &str, field: &str, max_chars: usize) -> GroupsResult<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > max_chars {
        return Err(GroupsError::Validation(format!(
            "{field} must contain between 1 and {max_chars} characters"
        )));
    }
    Ok(value.to_string())
}

fn normalize_optional_bounded_text(
    value: Option<String>,
    field: &str,
    max_chars: usize,
) -> GroupsResult<Option<String>> {
    value
        .map(|value| {
            let value = value.trim();
            if value.is_empty() {
                return Ok(None);
            }
            if value.chars().count() > max_chars {
                return Err(GroupsError::Validation(format!(
                    "{field} must not exceed {max_chars} characters"
                )));
            }
            Ok(Some(value.to_string()))
        })
        .transpose()
        .map(Option::flatten)
}

fn normalize_optional_note(value: Option<String>) -> GroupsResult<Option<String>> {
    normalize_optional_bounded_text(value, "application review note", MAX_REVIEW_NOTE_CHARS)
}

fn require_read(context: &PortContext) -> GroupsResult<()> {
    context
        .require_policy(PortCallPolicy::read())
        .map_err(|error| GroupsError::Validation(error.message))
}

fn require_write(context: &PortContext) -> GroupsResult<()> {
    context
        .require_policy(PortCallPolicy::write())
        .map_err(|error| GroupsError::Validation(error.message))
}

fn context_tenant_id(context: &PortContext) -> GroupsResult<Uuid> {
    Uuid::parse_str(&context.tenant_id)
        .map_err(|_| GroupsError::Validation("tenant_id must be a UUID".to_string()))
}

fn actor_user_id(context: &PortContext) -> GroupsResult<Uuid> {
    if context.actor.kind != PortActorKind::User {
        return Err(GroupsError::Forbidden(
            "a user actor is required for group membership applications".to_string(),
        ));
    }
    Uuid::parse_str(&context.actor.id)
        .map_err(|_| GroupsError::Validation("actor.id must be a UUID".to_string()))
}

fn idempotency_key(context: &PortContext) -> GroupsResult<String> {
    let key = context
        .idempotency_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| GroupsError::Validation("idempotency key is required".to_string()))?;
    if key.len() > 160 {
        return Err(GroupsError::Validation(
            "idempotency key must not exceed 160 bytes".to_string(),
        ));
    }
    Ok(key.to_string())
}

fn has_platform_manage(context: &PortContext) -> bool {
    context
        .claims
        .iter()
        .any(|claim| matches!(claim.as_str(), "groups:manage" | "groups:*" | "*:*"))
}
