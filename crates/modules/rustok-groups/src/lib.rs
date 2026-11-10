//! # rustok-groups
//!
//! Social groups, memberships, local roles, privacy, and feature bindings.
//!
//! Provides social group identity, multilingual presentation, group memberships,
//! invitations, application policies, and group-level enforcement state.

pub mod application_entities;

mod applications_legacy_module {
    use sea_orm::ConnectionTrait;

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError, normalize_locale_tag};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, DbBackend, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
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

impl UpsertGroupApplicationPolicyRequest {
    pub fn validate(&self) -> GroupsResult<()> {
        if self.group_id.is_nil() {
            return Err(GroupsError::Validation(
                "group_id cannot be nil UUID".to_string(),
            ));
        }
        if self.locale.trim().is_empty() || normalize_locale_tag(&self.locale).is_none() {
            return Err(GroupsError::Validation(
                "invalid application policy locale".to_string(),
            ));
        }
        if self.questions.len() > MAX_POLICY_QUESTIONS {
            return Err(GroupsError::Validation(format!(
                "membership policy may contain at most {MAX_POLICY_QUESTIONS} questions"
            )));
        }
        if self.rules.len() > MAX_POLICY_RULES {
            return Err(GroupsError::Validation(format!(
                "membership policy may contain at most {MAX_POLICY_RULES} rules"
            )));
        }
        Ok(())
    }
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

impl ReadGroupApplicationPolicyRequest {
    pub fn validate(&self) -> GroupsResult<()> {
        if self.group_id.is_nil() {
            return Err(GroupsError::Validation(
                "group_id cannot be nil UUID".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitGroupMembershipApplicationRequest {
    pub group_id: Uuid,
    pub answers: BTreeMap<String, String>,
    pub acknowledged_rule_keys: Vec<String>,
}

impl SubmitGroupMembershipApplicationRequest {
    pub fn validate(&self) -> GroupsResult<()> {
        if self.group_id.is_nil() {
            return Err(GroupsError::Validation(
                "group_id cannot be nil UUID".to_string(),
            ));
        }
        if self.answers.len() > MAX_POLICY_QUESTIONS {
            return Err(GroupsError::Validation(format!(
                "membership application answers count must not exceed {MAX_POLICY_QUESTIONS}"
            )));
        }
        if self.acknowledged_rule_keys.len() > MAX_POLICY_RULES {
            return Err(GroupsError::Validation(format!(
                "membership application acknowledged rules count must not exceed {MAX_POLICY_RULES}"
            )));
        }
        Ok(())
    }
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

impl ListGroupMembershipApplicationsRequest {
    pub fn validate(&self) -> GroupsResult<()> {
        if self.group_id.is_nil() {
            return Err(GroupsError::Validation(
                "group_id cannot be nil UUID".to_string(),
            ));
        }
        if self.page == 0 {
            return Err(GroupsError::Validation(
                "page must be greater than zero".to_string(),
            ));
        }
        if self.per_page == 0 || self.per_page > 100 {
            return Err(GroupsError::Validation(
                "per_page must be between 1 and 100".to_string(),
            ));
        }
        Ok(())
    }
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

impl ReviewGroupMembershipApplicationRequest {
    pub fn validate(&self) -> GroupsResult<()> {
        if self.application_id.is_nil() {
            return Err(GroupsError::Validation(
                "application_id cannot be nil UUID".to_string(),
            ));
        }
        if self
            .note
            .as_ref()
            .is_some_and(|note| note.chars().count() > MAX_REVIEW_NOTE_CHARS)
        {
            return Err(GroupsError::Validation(format!(
                "application review note must not exceed {MAX_REVIEW_NOTE_CHARS} characters"
            )));
        }
        Ok(())
    }
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
        request.validate()?;
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
        request.validate()?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        request.locale = normalize_locale_tag(&request.locale).ok_or_else(|| {
            GroupsError::Validation("invalid application policy locale".to_string())
        })?;
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
        let questions_value = serde_json::to_value(&request.questions).map_err(|error| {
            GroupsError::Invariant(format!(
                "application questions are not serializable: {error}"
            ))
        })?;
        let rules_value = serde_json::to_value(&request.rules).map_err(|error| {
            GroupsError::Invariant(format!("application rules are not serializable: {error}"))
        })?;
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
        request.validate()?;
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
        let policy =
            load_policy_for_locale(&transaction, tenant_id, request.group_id, &context.locale)
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
        let answers_value = serde_json::to_value(&request.answers).map_err(|error| {
            GroupsError::Invariant(format!("application answers are not serializable: {error}"))
        })?;
        let acknowledged_value =
            serde_json::to_value(&request.acknowledged_rule_keys).map_err(|error| {
                GroupsError::Invariant(format!(
                    "application acknowledgements are not serializable: {error}"
                ))
            })?;
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
        request.validate()?;
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
        request.validate()?;
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

        let application_model =
            find_application_for_update(&transaction, tenant_id, request.application_id).await?;
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
            .ok_or_else(|| {
                GroupsError::Invariant("pending application membership is missing".to_string())
            })?;
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
        request.validate().map_err(PortError::from)?;
        self.read_policy_owned(&context, request)
            .await
            .map_err(Into::into)
    }

    async fn list_group_membership_applications(
        &self,
        context: PortContext,
        request: ListGroupMembershipApplicationsRequest,
    ) -> Result<GroupMembershipApplicationConnection, PortError> {
        request.validate().map_err(PortError::from)?;
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
        request.validate().map_err(PortError::from)?;
        self.upsert_policy_owned(&context, request)
            .await
            .map_err(Into::into)
    }

    async fn submit_group_membership_application(
        &self,
        context: PortContext,
        request: SubmitGroupMembershipApplicationRequest,
    ) -> Result<SubmitGroupMembershipApplicationResult, PortError> {
        request.validate().map_err(PortError::from)?;
        self.submit_application_owned(&context, request)
            .await
            .map_err(Into::into)
    }

    async fn review_group_membership_application(
        &self,
        context: PortContext,
        request: ReviewGroupMembershipApplicationRequest,
    ) -> Result<ReviewGroupMembershipApplicationResult, PortError> {
        request.validate().map_err(PortError::from)?;
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
        .ok_or_else(|| {
            GroupsError::Conflict(
                "group membership application policy is not configured".to_string(),
            )
        })?;
    let translation = membership_policy_translation::Entity::find()
        .filter(membership_policy_translation::Column::TenantId.eq(tenant_id))
        .filter(membership_policy_translation::Column::PolicyId.eq(policy.id))
        .filter(membership_policy_translation::Column::Locale.eq(locale.clone()))
        .one(connection)
        .await?
        .ok_or_else(|| {
            GroupsError::Conflict(
                "group membership application policy is unavailable for the effective locale"
                    .to_string(),
            )
        })?;
    let questions = serde_json::from_value::<Vec<GroupApplicationQuestion>>(translation.questions)
        .map_err(|error| {
            GroupsError::Invariant(format!("invalid application question contract: {error}"))
        })?;
    let rules = serde_json::from_value::<Vec<GroupApplicationRule>>(translation.rules).map_err(
        |error| GroupsError::Invariant(format!("invalid application rule contract: {error}")),
    )?;
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
        question.help_text = normalize_optional_bounded_text(
            question.help_text.take(),
            "question help text",
            1_000,
        )?;
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
        let answer = request
            .answers
            .get(&question.key)
            .map(String::as_str)
            .unwrap_or("");
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

fn map_application(
    model: membership_application::Model,
) -> GroupsResult<GroupMembershipApplication> {
    #[derive(Deserialize)]
    struct Snapshot {
        questions: Vec<GroupApplicationQuestion>,
        rules: Vec<GroupApplicationRule>,
    }
    let snapshot = serde_json::from_value::<Snapshot>(model.policy_snapshot).map_err(|error| {
        GroupsError::Invariant(format!("invalid application policy snapshot: {error}"))
    })?;
    let answers = serde_json::from_value::<BTreeMap<String, String>>(model.answers)
        .map_err(|error| GroupsError::Invariant(format!("invalid application answers: {error}")))?;
    let acknowledged_rule_keys =
        serde_json::from_value::<Vec<String>>(model.acknowledged_rule_keys).map_err(|error| {
            GroupsError::Invariant(format!("invalid application acknowledgements: {error}"))
        })?;
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
        _ => unreachable!("unsupported SeaORM database backend"),
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
        _ => unreachable!("unsupported SeaORM database backend"),
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
            GroupsError::Invariant(format!(
                "group command response is not serializable: {error}"
            ))
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
        GroupsError::Validation(format!(
            "group command request is not serializable: {error}"
        ))
    })?;
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
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
    let id = Uuid::parse_str(context.tenant_id.trim())
        .map_err(|_| GroupsError::Validation("tenant_id must be a UUID".to_string()))?;
    if id.is_nil() {
        return Err(GroupsError::Validation(
            "tenant_id cannot be nil UUID".to_string(),
        ));
    }
    Ok(id)
}

fn actor_user_id(context: &PortContext) -> GroupsResult<Uuid> {
    if context.actor.kind != PortActorKind::User {
        return Err(GroupsError::Forbidden(
            "a user actor is required for group membership applications".to_string(),
        ));
    }
    let id = Uuid::parse_str(context.actor.id.trim())
        .map_err(|_| GroupsError::Validation("actor.id must be a UUID".to_string()))?;
    if id.is_nil() {
        return Err(GroupsError::Validation(
            "actor.id cannot be nil UUID".to_string(),
        ));
    }
    Ok(id)
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

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::{PortActor, PortActorKind, PortContext};

    #[test]
    fn test_applications_request_validation() {
        let valid_group_id = Uuid::new_v4();
        let valid_app_id = Uuid::new_v4();

        // Nil group_id checks
        let read_req = ReadGroupApplicationPolicyRequest {
            group_id: Uuid::nil(),
        };
        assert!(read_req.validate().is_err());

        let upsert_req = UpsertGroupApplicationPolicyRequest {
            group_id: Uuid::nil(),
            locale: "en".to_string(),
            enabled: true,
            questions: vec![],
            rules: vec![],
        };
        assert!(upsert_req.validate().is_err());

        let submit_req = SubmitGroupMembershipApplicationRequest {
            group_id: Uuid::nil(),
            answers: BTreeMap::new(),
            acknowledged_rule_keys: vec![],
        };
        assert!(submit_req.validate().is_err());

        let list_req = ListGroupMembershipApplicationsRequest {
            group_id: Uuid::nil(),
            status: None,
            page: 1,
            per_page: 20,
        };
        assert!(list_req.validate().is_err());

        let review_req = ReviewGroupMembershipApplicationRequest {
            application_id: Uuid::nil(),
            decision: GroupApplicationReviewDecision::Approve,
            note: None,
        };
        assert!(review_req.validate().is_err());

        // Bounds checks
        let mut invalid_list = list_req.clone();
        invalid_list.group_id = valid_group_id;
        invalid_list.page = 0;
        assert!(invalid_list.validate().is_err());
        invalid_list.page = 1;
        invalid_list.per_page = 0;
        assert!(invalid_list.validate().is_err());
        invalid_list.per_page = 101;
        assert!(invalid_list.validate().is_err());
        invalid_list.per_page = 100;
        assert!(invalid_list.validate().is_ok());

        let mut invalid_upsert = upsert_req.clone();
        invalid_upsert.group_id = valid_group_id;
        invalid_upsert.locale = "invalid-locale-!!!".to_string();
        assert!(invalid_upsert.validate().is_err());
        invalid_upsert.locale = "en".to_string();
        assert!(invalid_upsert.validate().is_ok());

        let mut invalid_review = review_req.clone();
        invalid_review.application_id = valid_app_id;
        invalid_review.note = Some("a".repeat(MAX_REVIEW_NOTE_CHARS + 1));
        assert!(invalid_review.validate().is_err());
        invalid_review.note = Some("Valid note".to_string());
        assert!(invalid_review.validate().is_ok());
    }

    #[test]
    fn test_applications_context_validation() {
        let valid_tenant = Uuid::new_v4();
        let valid_user = Uuid::new_v4();

        // Valid context
        let context = PortContext::new(
            format!("  {}  ", valid_tenant),
            PortActor {
                id: format!("  {}  ", valid_user),
                kind: PortActorKind::User,
            },
            "en",
            "corr-1",
        );
        assert_eq!(context_tenant_id(&context).unwrap(), valid_tenant);
        assert_eq!(actor_user_id(&context).unwrap(), valid_user);

        // Nil tenant
        let nil_tenant_ctx = PortContext::new(
            Uuid::nil().to_string(),
            PortActor {
                id: valid_user.to_string(),
                kind: PortActorKind::User,
            },
            "en",
            "corr-2",
        );
        assert!(context_tenant_id(&nil_tenant_ctx).is_err());

        // Nil actor
        let nil_actor_ctx = PortContext::new(
            valid_tenant.to_string(),
            PortActor {
                id: Uuid::nil().to_string(),
                kind: PortActorKind::User,
            },
            "en",
            "corr-3",
        );
        assert!(actor_user_id(&nil_actor_ctx).is_err());

        // System actor (rejected)
        let system_actor_ctx = PortContext::new(
            valid_tenant.to_string(),
            PortActor {
                id: valid_user.to_string(),
                kind: PortActorKind::System,
            },
            "en",
            "corr-4",
        );
        assert!(actor_user_id(&system_actor_ctx).is_err());
    }
}
#[async_trait]
pub trait GroupApplicationReviewCommandPort: Send + Sync {
    async fn review_group_membership_application(
        &self,
        context: PortContext,
        request: ReviewGroupMembershipApplicationRequest,
    ) -> Result<ReviewGroupMembershipApplicationResult, PortError>;
}

#[async_trait]
impl GroupApplicationReviewCommandPort for GroupApplicationService {
    async fn review_group_membership_application(
        &self,
        context: PortContext,
        request: ReviewGroupMembershipApplicationRequest,
    ) -> Result<ReviewGroupMembershipApplicationResult, PortError> {
        self.review_application_authorized_owned(&context, request)
            .await
            .map_err(Into::into)
    }
}

impl GroupApplicationService {
    async fn review_application_authorized_owned(
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

        let application_model =
            find_application_for_update(&transaction, tenant_id, request.application_id).await?;
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
        if application_model.status != GroupApplicationStatus::Pending.as_str() {
            return Err(GroupsError::Conflict(
                "membership application is no longer pending".to_string(),
            ));
        }

        let membership_model = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(application_model.group_id))
            .filter(membership::Column::UserId.eq(application_model.user_id))
            .one(&transaction)
            .await?
            .ok_or_else(|| {
                GroupsError::Invariant("pending application membership is missing".to_string())
            })?;
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
const MAX_BULK_REVIEW_ITEMS: usize = 50;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BulkReviewGroupMembershipApplicationsRequest {
    pub application_ids: Vec<Uuid>,
    pub decision: GroupApplicationReviewDecision,
    pub note: Option<String>,
    pub confirmed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BulkReviewGroupMembershipApplicationItemResult {
    pub application_id: Uuid,
    pub result: Option<ReviewGroupMembershipApplicationResult>,
    pub error: Option<PortError>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BulkReviewGroupMembershipApplicationsResult {
    pub items: Vec<BulkReviewGroupMembershipApplicationItemResult>,
    pub succeeded: u32,
    pub failed: u32,
}

#[async_trait]
pub trait GroupApplicationBulkReviewCommandPort: Send + Sync {
    async fn bulk_review_group_membership_applications(
        &self,
        context: PortContext,
        request: BulkReviewGroupMembershipApplicationsRequest,
    ) -> Result<BulkReviewGroupMembershipApplicationsResult, PortError>;
}

#[async_trait]
impl GroupApplicationBulkReviewCommandPort for GroupApplicationService {
    async fn bulk_review_group_membership_applications(
        &self,
        context: PortContext,
        request: BulkReviewGroupMembershipApplicationsRequest,
    ) -> Result<BulkReviewGroupMembershipApplicationsResult, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        if !request.confirmed {
            return Err(PortError::validation(
                "groups.bulk_review_confirmation_required",
                "bulk membership application review requires explicit confirmation",
            ));
        }
        if request.application_ids.is_empty() {
            return Err(PortError::validation(
                "groups.bulk_review_empty",
                "bulk membership application review requires at least one application",
            ));
        }
        if request.application_ids.len() > MAX_BULK_REVIEW_ITEMS {
            return Err(PortError::validation(
                "groups.bulk_review_limit_exceeded",
                format!(
                    "bulk membership application review accepts at most {MAX_BULK_REVIEW_ITEMS} applications"
                ),
            ));
        }

        let mut unique_ids = BTreeSet::new();
        for application_id in &request.application_ids {
            if !unique_ids.insert(*application_id) {
                return Err(PortError::validation(
                    "groups.bulk_review_duplicate_application",
                    "bulk membership application review contains duplicate application IDs",
                ));
            }
        }
        let normalized_note = normalize_optional_note(request.note).map_err(PortError::from)?;

        let base_idempotency_key = context
            .idempotency_key
            .as_deref()
            .ok_or_else(|| {
                PortError::validation(
                    "port.idempotency_key_required",
                    "write port calls require a non-empty idempotency key",
                )
            })?
            .to_string();
        let mut items = Vec::with_capacity(request.application_ids.len());
        let mut succeeded = 0_u32;
        let mut failed = 0_u32;

        for application_id in request.application_ids {
            let item_context = context
                .clone()
                .with_causation_id(context.correlation_id.clone())
                .with_idempotency_key(bulk_review_item_idempotency_key(
                    &base_idempotency_key,
                    application_id,
                ));
            let item_request = ReviewGroupMembershipApplicationRequest {
                application_id,
                decision: request.decision,
                note: normalized_note.clone(),
            };
            match self
                .review_application_authorized_owned(&item_context, item_request)
                .await
            {
                Ok(result) => {
                    succeeded = succeeded.saturating_add(1);
                    items.push(BulkReviewGroupMembershipApplicationItemResult {
                        application_id,
                        result: Some(result),
                        error: None,
                    });
                }
                Err(error) => {
                    failed = failed.saturating_add(1);
                    items.push(BulkReviewGroupMembershipApplicationItemResult {
                        application_id,
                        result: None,
                        error: Some(error.into()),
                    });
                }
            }
        }

        Ok(BulkReviewGroupMembershipApplicationsResult {
            items,
            succeeded,
            failed,
        })
    }
}

fn bulk_review_item_idempotency_key(base_idempotency_key: &str, application_id: Uuid) -> String {
    let mut hasher = Sha256::new();
    hasher.update(base_idempotency_key.as_bytes());
    hasher.update(application_id.as_bytes());
    let digest = hasher.finalize();
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut encoded, "{byte:02x}");
    }
    format!("groups-bulk-review:{encoded}")
}
const UPSERT_POLICY_IF_CURRENT_COMMAND: &str =
    "groups.upsert_membership_application_policy_if_current.v1";
const SUBMIT_APPLICATION_IF_CURRENT_COMMAND: &str =
    "groups.submit_membership_application_if_current.v1";

pub const GROUP_APPLICATION_POLICY_CHANGED_CODE: &str = "groups.application_policy_changed";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupApplicationPolicyPrecondition {
    pub policy_id: Uuid,
    pub revision: u64,
    pub locale: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpsertGroupApplicationPolicyIfCurrentRequest {
    pub expected_policy: Option<GroupApplicationPolicyPrecondition>,
    pub policy: UpsertGroupApplicationPolicyRequest,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitGroupMembershipApplicationIfCurrentRequest {
    pub expected_policy: GroupApplicationPolicyPrecondition,
    pub submission: SubmitGroupMembershipApplicationRequest,
}

#[async_trait]
pub trait GroupApplicationCasCommandPort: Send + Sync {
    async fn upsert_group_application_policy_if_current(
        &self,
        context: PortContext,
        request: UpsertGroupApplicationPolicyIfCurrentRequest,
    ) -> Result<UpsertGroupApplicationPolicyResult, PortError>;

    async fn submit_group_membership_application_if_current(
        &self,
        context: PortContext,
        request: SubmitGroupMembershipApplicationIfCurrentRequest,
    ) -> Result<SubmitGroupMembershipApplicationResult, PortError>;
}

impl GroupApplicationService {
    async fn upsert_policy_if_current_owned(
        &self,
        context: &PortContext,
        mut request: UpsertGroupApplicationPolicyIfCurrentRequest,
    ) -> GroupsResult<UpsertGroupApplicationPolicyResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        request.policy.locale = normalize_locale_tag(&request.policy.locale).ok_or_else(|| {
            GroupsError::Validation("invalid application policy locale".to_string())
        })?;
        normalize_policy(&mut request.policy.questions, &mut request.policy.rules)?;
        if let Some(expected) = request.expected_policy.as_mut() {
            normalize_policy_precondition(expected)?;
            if expected.locale != request.policy.locale {
                return Err(GroupsError::Validation(
                    "expected policy locale must match the policy locale being updated".to_string(),
                ));
            }
        }
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;

        if let Some(mut replayed) = replay_receipt::<UpsertGroupApplicationPolicyResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            UPSERT_POLICY_IF_CURRENT_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        let group_model =
            find_group_for_update(&transaction, tenant_id, request.policy.group_id).await?;
        require_active_group(&group_model)?;
        authorize_policy_management(
            &transaction,
            context,
            tenant_id,
            request.policy.group_id,
            actor_user_id,
        )
        .await?;

        let existing_policy = membership_policy::Entity::find()
            .filter(membership_policy::Column::TenantId.eq(tenant_id))
            .filter(membership_policy::Column::GroupId.eq(request.policy.group_id))
            .one(&transaction)
            .await?;
        ensure_policy_update_precondition(
            request.expected_policy.as_ref(),
            existing_policy.as_ref(),
            &request.policy.locale,
        )?;

        let now = Utc::now();
        let created = existing_policy.is_none();
        let policy_model = if let Some(existing) = existing_policy {
            let next_revision = existing.revision.saturating_add(1).max(1);
            let mut active: membership_policy::ActiveModel = existing.into();
            active.revision = Set(next_revision);
            active.enabled = Set(request.policy.enabled);
            active.updated_by_user_id = Set(actor_user_id);
            active.updated_at = Set(now.fixed_offset());
            active.update(&transaction).await?
        } else {
            membership_policy::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                group_id: Set(request.policy.group_id),
                revision: Set(1),
                enabled: Set(request.policy.enabled),
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
            .filter(membership_policy_translation::Column::Locale.eq(request.policy.locale.clone()))
            .one(&transaction)
            .await?;
        let questions_value = serde_json::to_value(&request.policy.questions).map_err(|error| {
            GroupsError::Invariant(format!(
                "application questions are not serializable: {error}"
            ))
        })?;
        let rules_value = serde_json::to_value(&request.policy.rules).map_err(|error| {
            GroupsError::Invariant(format!("application rules are not serializable: {error}"))
        })?;
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
                locale: Set(request.policy.locale.clone()),
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
                group_id: request.policy.group_id,
                revision: policy_model.revision.max(1) as u64,
                enabled: policy_model.enabled,
                locale: request.policy.locale,
                questions: request.policy.questions,
                rules: request.policy.rules,
            },
            group_version,
            created,
            replayed: false,
        };
        append_audit(
            &transaction,
            context,
            tenant_id,
            result.policy.group_id,
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
                "group_version": group_version,
                "expected_revision_enforced": true
            }),
        )
        .await?;
        store_receipt(
            &transaction,
            tenant_id,
            result.policy.group_id,
            actor_user_id,
            idempotency_key,
            UPSERT_POLICY_IF_CURRENT_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }

    async fn submit_application_if_current_owned(
        &self,
        context: &PortContext,
        mut request: SubmitGroupMembershipApplicationIfCurrentRequest,
    ) -> GroupsResult<SubmitGroupMembershipApplicationResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        normalize_policy_precondition(&mut request.expected_policy)?;
        normalize_application_submission(&mut request.submission)?;
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;

        if let Some(mut replayed) = replay_receipt::<SubmitGroupMembershipApplicationResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            SUBMIT_APPLICATION_IF_CURRENT_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        let prelocked_application = find_candidate_application_for_update(
            &transaction,
            tenant_id,
            request.submission.group_id,
            actor_user_id,
        )
        .await?;
        let group_model =
            find_group_for_update(&transaction, tenant_id, request.submission.group_id).await?;
        require_application_group(&group_model)?;
        let policy = load_policy_for_locale(
            &transaction,
            tenant_id,
            request.submission.group_id,
            &context.locale,
        )
        .await?;
        ensure_loaded_policy_precondition(&request.expected_policy, &policy)?;
        if !policy.enabled {
            return Err(GroupsError::Conflict(
                "group membership applications are disabled".to_string(),
            ));
        }
        validate_submission(&policy, &request.submission)?;

        let existing_membership = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(request.submission.group_id))
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

        let existing_application = match prelocked_application {
            Some(existing) => Some(existing),
            None => {
                find_candidate_application_for_update(
                    &transaction,
                    tenant_id,
                    request.submission.group_id,
                    actor_user_id,
                )
                .await?
            }
        };
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
                group_id: Set(request.submission.group_id),
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
        let answers_value = serde_json::to_value(&request.submission.answers).map_err(|error| {
            GroupsError::Invariant(format!("application answers are not serializable: {error}"))
        })?;
        let acknowledged_value = serde_json::to_value(&request.submission.acknowledged_rule_keys)
            .map_err(|error| {
            GroupsError::Invariant(format!(
                "application acknowledgements are not serializable: {error}"
            ))
        })?;
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
                group_id: Set(request.submission.group_id),
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
            request.submission.group_id,
            actor_user_id,
            "group.membership_application_submitted",
            Some(actor_user_id),
            json!({
                "application_id": result.application.id,
                "policy_id": result.application.policy_id,
                "policy_revision": result.application.policy_revision,
                "policy_locale": result.application.policy_locale,
                "group_version": group_version,
                "expected_revision_enforced": true,
                "application_lock_order": "application_then_group_when_existing"
            }),
        )
        .await?;
        store_receipt(
            &transaction,
            tenant_id,
            request.submission.group_id,
            actor_user_id,
            idempotency_key,
            SUBMIT_APPLICATION_IF_CURRENT_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }
}

#[async_trait]
impl GroupApplicationCasCommandPort for GroupApplicationService {
    async fn upsert_group_application_policy_if_current(
        &self,
        context: PortContext,
        request: UpsertGroupApplicationPolicyIfCurrentRequest,
    ) -> Result<UpsertGroupApplicationPolicyResult, PortError> {
        self.upsert_policy_if_current_owned(&context, request)
            .await
            .map_err(map_application_cas_error)
    }

    async fn submit_group_membership_application_if_current(
        &self,
        context: PortContext,
        request: SubmitGroupMembershipApplicationIfCurrentRequest,
    ) -> Result<SubmitGroupMembershipApplicationResult, PortError> {
        self.submit_application_if_current_owned(&context, request)
            .await
            .map_err(map_application_cas_error)
    }
}

async fn find_candidate_application_for_update(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    group_id: Uuid,
    user_id: Uuid,
) -> GroupsResult<Option<membership_application::Model>> {
    let query = || {
        membership_application::Entity::find()
            .filter(membership_application::Column::TenantId.eq(tenant_id))
            .filter(membership_application::Column::GroupId.eq(group_id))
            .filter(membership_application::Column::UserId.eq(user_id))
    };
    let model = match transaction.get_database_backend() {
        DbBackend::Sqlite => query().one(transaction).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().one(transaction).await?,
        _ => unreachable!("unsupported SeaORM database backend"),
    };
    Ok(model)
}

fn normalize_policy_precondition(
    expected: &mut GroupApplicationPolicyPrecondition,
) -> GroupsResult<()> {
    if expected.revision == 0 {
        return Err(GroupsError::Validation(
            "expected policy revision must be at least 1".to_string(),
        ));
    }
    expected.locale = normalize_locale_tag(&expected.locale)
        .ok_or_else(|| GroupsError::Validation("invalid expected policy locale".to_string()))?;
    Ok(())
}

fn ensure_policy_update_precondition(
    expected: Option<&GroupApplicationPolicyPrecondition>,
    current: Option<&membership_policy::Model>,
    locale: &str,
) -> GroupsResult<()> {
    let matches = match (expected, current) {
        (None, None) => true,
        (Some(expected), Some(current)) => {
            expected.policy_id == current.id
                && expected.revision == current.revision.max(1) as u64
                && expected.locale == locale
        }
        _ => false,
    };
    if matches {
        Ok(())
    } else {
        Err(policy_changed_error(
            expected,
            current.map(|value| (value.id, value.revision.max(1) as u64, locale)),
        ))
    }
}

fn ensure_loaded_policy_precondition(
    expected: &GroupApplicationPolicyPrecondition,
    current: &GroupApplicationPolicy,
) -> GroupsResult<()> {
    if expected.policy_id == current.id
        && expected.revision == current.revision
        && expected.locale == current.locale
    {
        Ok(())
    } else {
        Err(policy_changed_error(
            Some(expected),
            Some((current.id, current.revision, current.locale.as_str())),
        ))
    }
}

fn policy_changed_error(
    expected: Option<&GroupApplicationPolicyPrecondition>,
    current: Option<(Uuid, u64, &str)>,
) -> GroupsError {
    let expected_description = expected
        .map(|value| format!("{}@{}:{}", value.policy_id, value.revision, value.locale))
        .unwrap_or_else(|| "none".to_string());
    let current_description = current
        .map(|(policy_id, revision, locale)| format!("{policy_id}@{revision}:{locale}"))
        .unwrap_or_else(|| "none".to_string());
    GroupsError::Conflict(format!(
        "{GROUP_APPLICATION_POLICY_CHANGED_CODE}: expected={expected_description}; current={current_description}"
    ))
}

fn map_application_cas_error(error: GroupsError) -> PortError {
    match error {
        GroupsError::Conflict(message)
            if message.starts_with(GROUP_APPLICATION_POLICY_CHANGED_CODE) =>
        {
            let message = message
                .strip_prefix(GROUP_APPLICATION_POLICY_CHANGED_CODE)
                .unwrap_or(&message)
                .trim_start_matches(':')
                .trim()
                .to_string();
            PortError::conflict(GROUP_APPLICATION_POLICY_CHANGED_CODE, message)
        }
        other => other.into(),
    }
}
const CANCEL_APPLICATION_COMMAND: &str = "groups.cancel_membership_application.v1";
const REOPEN_APPLICATION_COMMAND: &str = "groups.reopen_membership_application.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadMyGroupMembershipApplicationRequest {
    pub group_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancelGroupMembershipApplicationRequest {
    pub application_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReopenGroupMembershipApplicationRequest {
    pub application_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupApplicationLifecycleResult {
    pub application: GroupMembershipApplication,
    pub membership: GroupMembership,
    pub group_version: u64,
    pub replayed: bool,
}

#[async_trait]
pub trait GroupApplicationLifecycleReadPort: Send + Sync {
    async fn read_my_group_membership_application(
        &self,
        context: PortContext,
        request: ReadMyGroupMembershipApplicationRequest,
    ) -> Result<Option<GroupMembershipApplication>, PortError>;
}

#[async_trait]
pub trait GroupApplicationLifecycleCommandPort: Send + Sync {
    async fn cancel_group_membership_application(
        &self,
        context: PortContext,
        request: CancelGroupMembershipApplicationRequest,
    ) -> Result<GroupApplicationLifecycleResult, PortError>;

    async fn reopen_group_membership_application(
        &self,
        context: PortContext,
        request: ReopenGroupMembershipApplicationRequest,
    ) -> Result<GroupApplicationLifecycleResult, PortError>;
}

impl GroupApplicationService {
    async fn read_my_application_owned(
        &self,
        context: &PortContext,
        request: ReadMyGroupMembershipApplicationRequest,
    ) -> GroupsResult<Option<GroupMembershipApplication>> {
        require_read(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let group_model = find_group(&self.db, tenant_id, request.group_id).await?;
        require_active_group(&group_model)?;
        ensure_not_banned(&self.db, tenant_id, request.group_id, actor_user_id).await?;
        membership_application::Entity::find()
            .filter(membership_application::Column::TenantId.eq(tenant_id))
            .filter(membership_application::Column::GroupId.eq(request.group_id))
            .filter(membership_application::Column::UserId.eq(actor_user_id))
            .one(&self.db)
            .await?
            .map(map_application)
            .transpose()
    }

    async fn cancel_application_owned(
        &self,
        context: &PortContext,
        request: CancelGroupMembershipApplicationRequest,
    ) -> GroupsResult<GroupApplicationLifecycleResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;

        if let Some(mut replayed) = replay_receipt::<GroupApplicationLifecycleResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            CANCEL_APPLICATION_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        let application_model =
            find_application_for_update(&transaction, tenant_id, request.application_id).await?;
        if application_model.user_id != actor_user_id {
            return Err(GroupsError::NotFound);
        }
        if application_model.status != GroupApplicationStatus::Pending.as_str() {
            return Err(GroupsError::Conflict(
                "only a pending membership application can be cancelled".to_string(),
            ));
        }

        let group_model =
            find_group_for_update(&transaction, tenant_id, application_model.group_id).await?;
        require_active_group(&group_model)?;
        let membership_model = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(application_model.group_id))
            .filter(membership::Column::UserId.eq(actor_user_id))
            .one(&transaction)
            .await?
            .ok_or_else(|| {
                GroupsError::Invariant(
                    "pending application membership is missing during cancellation".to_string(),
                )
            })?;
        if membership_model.status == GroupMembershipStatus::Banned.as_str() {
            return Err(GroupsError::Forbidden(
                "group membership is banned".to_string(),
            ));
        }
        if membership_model.status != GroupMembershipStatus::Pending.as_str() {
            return Err(GroupsError::Conflict(
                "membership is no longer pending".to_string(),
            ));
        }

        let now = Utc::now();
        let mut membership_active: membership::ActiveModel = membership_model.into();
        membership_active.status = Set(GroupMembershipStatus::Left.as_str().to_string());
        membership_active.joined_at = Set(None);
        membership_active.left_at = Set(Some(now.fixed_offset()));
        membership_active.updated_at = Set(now.fixed_offset());
        let membership_model = membership_active.update(&transaction).await?;

        let mut application_active: membership_application::ActiveModel = application_model.into();
        application_active.status = Set(GroupApplicationStatus::Cancelled.as_str().to_string());
        application_active.reviewed_at = Set(None);
        application_active.reviewed_by_user_id = Set(None);
        application_active.review_note = Set(None);
        application_active.updated_at = Set(now.fixed_offset());
        let application_model = application_active.update(&transaction).await?;

        let group_version = increment_group_version(&transaction, group_model, now).await?;
        let result = GroupApplicationLifecycleResult {
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
            "group.membership_application_cancelled",
            Some(actor_user_id),
            json!({
                "application_id": result.application.id,
                "previous_status": "pending",
                "membership_status": result.membership.status,
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
            CANCEL_APPLICATION_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }

    async fn reopen_application_owned(
        &self,
        context: &PortContext,
        request: ReopenGroupMembershipApplicationRequest,
    ) -> GroupsResult<GroupApplicationLifecycleResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;

        if let Some(mut replayed) = replay_receipt::<GroupApplicationLifecycleResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            REOPEN_APPLICATION_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        let application_model =
            find_application_for_update(&transaction, tenant_id, request.application_id).await?;
        let group_model =
            find_group_for_update(&transaction, tenant_id, application_model.group_id).await?;
        require_application_group(&group_model)?;
        authorize_application_review(
            &transaction,
            context,
            tenant_id,
            application_model.group_id,
            actor_user_id,
        )
        .await?;

        let previous_status = GroupApplicationStatus::from_str(&application_model.status)
            .map_err(GroupsError::Invariant)?;
        if !matches!(
            previous_status,
            GroupApplicationStatus::Rejected | GroupApplicationStatus::Cancelled
        ) {
            return Err(GroupsError::Conflict(
                "only a rejected or cancelled membership application can be reopened".to_string(),
            ));
        }

        let membership_model = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(application_model.group_id))
            .filter(membership::Column::UserId.eq(application_model.user_id))
            .one(&transaction)
            .await?
            .ok_or_else(|| {
                GroupsError::Invariant(
                    "application membership is missing during reopen".to_string(),
                )
            })?;
        if membership_model.status == GroupMembershipStatus::Banned.as_str() {
            return Err(GroupsError::Forbidden(
                "group membership is banned".to_string(),
            ));
        }
        if membership_model.status == GroupMembershipStatus::Active.as_str() {
            return Err(GroupsError::Conflict(
                "an active member application cannot be reopened".to_string(),
            ));
        }
        if membership_model.status != GroupMembershipStatus::Left.as_str() {
            return Err(GroupsError::Conflict(
                "membership must be left before the application can be reopened".to_string(),
            ));
        }

        let now = Utc::now();
        let target_user_id = application_model.user_id;
        let mut membership_active: membership::ActiveModel = membership_model.into();
        membership_active.role = Set(GroupRole::Member.as_str().to_string());
        membership_active.status = Set(GroupMembershipStatus::Pending.as_str().to_string());
        membership_active.joined_at = Set(None);
        membership_active.left_at = Set(None);
        membership_active.updated_at = Set(now.fixed_offset());
        let membership_model = membership_active.update(&transaction).await?;

        let mut application_active: membership_application::ActiveModel = application_model.into();
        application_active.status = Set(GroupApplicationStatus::Pending.as_str().to_string());
        application_active.reviewed_at = Set(None);
        application_active.reviewed_by_user_id = Set(None);
        application_active.review_note = Set(None);
        application_active.updated_at = Set(now.fixed_offset());
        let application_model = application_active.update(&transaction).await?;

        let group_version = increment_group_version(&transaction, group_model, now).await?;
        let result = GroupApplicationLifecycleResult {
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
            "group.membership_application_reopened",
            Some(target_user_id),
            json!({
                "application_id": result.application.id,
                "previous_status": previous_status.as_str(),
                "policy_id": result.application.policy_id,
                "policy_revision": result.application.policy_revision,
                "policy_locale": result.application.policy_locale,
                "snapshot_preserved": true,
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
            REOPEN_APPLICATION_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }
}

#[async_trait]
impl GroupApplicationLifecycleReadPort for GroupApplicationService {
    async fn read_my_group_membership_application(
        &self,
        context: PortContext,
        request: ReadMyGroupMembershipApplicationRequest,
    ) -> Result<Option<GroupMembershipApplication>, PortError> {
        self.read_my_application_owned(&context, request)
            .await
            .map_err(Into::into)
    }
}

#[async_trait]
impl GroupApplicationLifecycleCommandPort for GroupApplicationService {
    async fn cancel_group_membership_application(
        &self,
        context: PortContext,
        request: CancelGroupMembershipApplicationRequest,
    ) -> Result<GroupApplicationLifecycleResult, PortError> {
        self.cancel_application_owned(&context, request)
            .await
            .map_err(Into::into)
    }

    async fn reopen_group_membership_application(
        &self,
        context: PortContext,
        request: ReopenGroupMembershipApplicationRequest,
    ) -> Result<GroupApplicationLifecycleResult, PortError> {
        self.reopen_application_owned(&context, request)
            .await
            .map_err(Into::into)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListGroupApplicationPolicyLocalesRequest {
    pub group_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadGroupApplicationPolicyForManagementRequest {
    pub group_id: Uuid,
    pub locale: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupApplicationPolicyLocaleCatalog {
    pub group_id: Uuid,
    pub policy_id: Option<Uuid>,
    pub revision: Option<u64>,
    pub enabled: bool,
    pub locales: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupApplicationPolicyManagementView {
    pub group_id: Uuid,
    pub policy_id: Option<Uuid>,
    pub revision: Option<u64>,
    pub enabled: bool,
    pub locale: String,
    pub translation_exists: bool,
    pub questions: Vec<GroupApplicationQuestion>,
    pub rules: Vec<GroupApplicationRule>,
}

#[async_trait]
pub trait GroupApplicationPolicyManagementReadPort: Send + Sync {
    async fn list_group_application_policy_locales(
        &self,
        context: PortContext,
        request: ListGroupApplicationPolicyLocalesRequest,
    ) -> Result<GroupApplicationPolicyLocaleCatalog, PortError>;

    async fn read_group_application_policy_for_management(
        &self,
        context: PortContext,
        request: ReadGroupApplicationPolicyForManagementRequest,
    ) -> Result<GroupApplicationPolicyManagementView, PortError>;
}

impl GroupApplicationService {
    async fn list_policy_locales_owned(
        &self,
        context: &PortContext,
        request: ListGroupApplicationPolicyLocalesRequest,
    ) -> GroupsResult<GroupApplicationPolicyLocaleCatalog> {
        require_read(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let group_model = find_group(&self.db, tenant_id, request.group_id).await?;
        require_active_group(&group_model)?;
        authorize_policy_management(
            &self.db,
            context,
            tenant_id,
            request.group_id,
            actor_user_id,
        )
        .await?;

        let policy = membership_policy::Entity::find()
            .filter(membership_policy::Column::TenantId.eq(tenant_id))
            .filter(membership_policy::Column::GroupId.eq(request.group_id))
            .one(&self.db)
            .await?;
        let Some(policy) = policy else {
            return Ok(GroupApplicationPolicyLocaleCatalog {
                group_id: request.group_id,
                policy_id: None,
                revision: None,
                enabled: true,
                locales: Vec::new(),
            });
        };

        let locales = membership_policy_translation::Entity::find()
            .filter(membership_policy_translation::Column::TenantId.eq(tenant_id))
            .filter(membership_policy_translation::Column::PolicyId.eq(policy.id))
            .order_by_asc(membership_policy_translation::Column::Locale)
            .all(&self.db)
            .await?
            .into_iter()
            .map(|translation| translation.locale)
            .collect();

        Ok(GroupApplicationPolicyLocaleCatalog {
            group_id: request.group_id,
            policy_id: Some(policy.id),
            revision: Some(policy.revision.max(1) as u64),
            enabled: policy.enabled,
            locales,
        })
    }

    async fn read_policy_for_management_owned(
        &self,
        context: &PortContext,
        mut request: ReadGroupApplicationPolicyForManagementRequest,
    ) -> GroupsResult<GroupApplicationPolicyManagementView> {
        require_read(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        request.locale = normalize_locale_tag(&request.locale).ok_or_else(|| {
            GroupsError::Validation("invalid application policy management locale".to_string())
        })?;
        let group_model = find_group(&self.db, tenant_id, request.group_id).await?;
        require_active_group(&group_model)?;
        authorize_policy_management(
            &self.db,
            context,
            tenant_id,
            request.group_id,
            actor_user_id,
        )
        .await?;

        let policy = membership_policy::Entity::find()
            .filter(membership_policy::Column::TenantId.eq(tenant_id))
            .filter(membership_policy::Column::GroupId.eq(request.group_id))
            .one(&self.db)
            .await?;
        let Some(policy) = policy else {
            return Ok(GroupApplicationPolicyManagementView {
                group_id: request.group_id,
                policy_id: None,
                revision: None,
                enabled: true,
                locale: request.locale,
                translation_exists: false,
                questions: Vec::new(),
                rules: Vec::new(),
            });
        };

        let translation = membership_policy_translation::Entity::find()
            .filter(membership_policy_translation::Column::TenantId.eq(tenant_id))
            .filter(membership_policy_translation::Column::PolicyId.eq(policy.id))
            .filter(membership_policy_translation::Column::Locale.eq(request.locale.clone()))
            .one(&self.db)
            .await?;

        let (translation_exists, questions, rules) = match translation {
            Some(translation) => (
                true,
                serde_json::from_value::<Vec<GroupApplicationQuestion>>(translation.questions)
                    .map_err(|error| {
                        GroupsError::Invariant(format!(
                            "invalid application policy management questions: {error}"
                        ))
                    })?,
                serde_json::from_value::<Vec<GroupApplicationRule>>(translation.rules).map_err(
                    |error| {
                        GroupsError::Invariant(format!(
                            "invalid application policy management rules: {error}"
                        ))
                    },
                )?,
            ),
            None => (false, Vec::new(), Vec::new()),
        };

        Ok(GroupApplicationPolicyManagementView {
            group_id: request.group_id,
            policy_id: Some(policy.id),
            revision: Some(policy.revision.max(1) as u64),
            enabled: policy.enabled,
            locale: request.locale,
            translation_exists,
            questions,
            rules,
        })
    }
}

#[async_trait]
impl GroupApplicationPolicyManagementReadPort for GroupApplicationService {
    async fn list_group_application_policy_locales(
        &self,
        context: PortContext,
        request: ListGroupApplicationPolicyLocalesRequest,
    ) -> Result<GroupApplicationPolicyLocaleCatalog, PortError> {
        self.list_policy_locales_owned(&context, request)
            .await
            .map_err(Into::into)
    }

    async fn read_group_application_policy_for_management(
        &self,
        context: PortContext,
        request: ReadGroupApplicationPolicyForManagementRequest,
    ) -> Result<GroupApplicationPolicyManagementView, PortError> {
        self.read_policy_for_management_owned(&context, request)
            .await
            .map_err(Into::into)
    }
}
    impl GroupApplicationService {
    pub(crate) async fn upsert_policy_effective_owned(
        &self,
        context: &PortContext,
        mut request: UpsertGroupApplicationPolicyRequest,
    ) -> GroupsResult<UpsertGroupApplicationPolicyResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        request.locale = normalize_locale_tag(&request.locale).ok_or_else(|| {
            GroupsError::Validation("invalid application policy locale".to_string())
        })?;
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
        crate::effective_membership_guard::require_effective_manager_owned(
            &transaction,
            context,
            tenant_id,
            request.group_id,
            actor_user_id,
            crate::effective_membership_guard::GroupManagerCapability::ManageSettings,
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
        let questions_value = serde_json::to_value(&request.questions).map_err(|error| {
            GroupsError::Invariant(format!(
                "application questions are not serializable: {error}"
            ))
        })?;
        let rules_value = serde_json::to_value(&request.rules).map_err(|error| {
            GroupsError::Invariant(format!("application rules are not serializable: {error}"))
        })?;
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

    pub(crate) async fn submit_application_effective_owned(
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
        crate::effective_membership_guard::require_user_not_denied_owned(
            &transaction,
            tenant_id,
            request.group_id,
            actor_user_id,
            true,
        )
        .await?;
        let policy =
            load_policy_for_locale(&transaction, tenant_id, request.group_id, &context.locale)
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
        let answers_value = serde_json::to_value(&request.answers).map_err(|error| {
            GroupsError::Invariant(format!("application answers are not serializable: {error}"))
        })?;
        let acknowledged_value =
            serde_json::to_value(&request.acknowledged_rule_keys).map_err(|error| {
                GroupsError::Invariant(format!(
                    "application acknowledgements are not serializable: {error}"
                ))
            })?;
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

    pub(crate) async fn review_application_effective_owned(
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

        let application_model =
            find_application_for_update(&transaction, tenant_id, request.application_id).await?;
        let group_model =
            find_group_for_update(&transaction, tenant_id, application_model.group_id).await?;
        require_active_group(&group_model)?;
        crate::effective_membership_guard::require_effective_manager_owned(
            &transaction,
            context,
            tenant_id,
            application_model.group_id,
            actor_user_id,
            crate::effective_membership_guard::GroupManagerCapability::Moderate,
        )
        .await?;
        crate::effective_membership_guard::require_user_not_denied_owned(
            &transaction,
            tenant_id,
            application_model.group_id,
            application_model.user_id,
            true,
        )
        .await?;
        if application_model.status != GroupApplicationStatus::Pending.as_str() {
            return Err(GroupsError::Conflict(
                "membership application is no longer pending".to_string(),
            ));
        }
        let membership_model = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(application_model.group_id))
            .filter(membership::Column::UserId.eq(application_model.user_id))
            .one(&transaction)
            .await?
            .ok_or_else(|| {
                GroupsError::Invariant("pending application membership is missing".to_string())
            })?;

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
    impl GroupApplicationService {
    pub(crate) async fn upsert_policy_if_current_effective_owned(
        &self,
        context: &PortContext,
        mut request: UpsertGroupApplicationPolicyIfCurrentRequest,
    ) -> GroupsResult<UpsertGroupApplicationPolicyResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        request.policy.locale = normalize_locale_tag(&request.policy.locale).ok_or_else(|| {
            GroupsError::Validation("invalid application policy locale".to_string())
        })?;
        normalize_policy(&mut request.policy.questions, &mut request.policy.rules)?;
        if let Some(expected) = request.expected_policy.as_mut() {
            normalize_policy_precondition(expected)?;
            if expected.locale != request.policy.locale {
                return Err(GroupsError::Validation(
                    "expected policy locale must match the policy locale being updated".to_string(),
                ));
            }
        }
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;

        if let Some(mut replayed) = replay_receipt::<UpsertGroupApplicationPolicyResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            UPSERT_POLICY_IF_CURRENT_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        let group_model =
            find_group_for_update(&transaction, tenant_id, request.policy.group_id).await?;
        require_active_group(&group_model)?;
        crate::effective_membership_guard::require_effective_manager_owned(
            &transaction,
            context,
            tenant_id,
            request.policy.group_id,
            actor_user_id,
            crate::effective_membership_guard::GroupManagerCapability::ManageSettings,
        )
        .await?;

        let existing_policy = membership_policy::Entity::find()
            .filter(membership_policy::Column::TenantId.eq(tenant_id))
            .filter(membership_policy::Column::GroupId.eq(request.policy.group_id))
            .one(&transaction)
            .await?;
        ensure_policy_update_precondition(
            request.expected_policy.as_ref(),
            existing_policy.as_ref(),
            &request.policy.locale,
        )?;

        let now = Utc::now();
        let created = existing_policy.is_none();
        let policy_model = if let Some(existing) = existing_policy {
            let next_revision = existing.revision.saturating_add(1).max(1);
            let mut active: membership_policy::ActiveModel = existing.into();
            active.revision = Set(next_revision);
            active.enabled = Set(request.policy.enabled);
            active.updated_by_user_id = Set(actor_user_id);
            active.updated_at = Set(now.fixed_offset());
            active.update(&transaction).await?
        } else {
            membership_policy::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                group_id: Set(request.policy.group_id),
                revision: Set(1),
                enabled: Set(request.policy.enabled),
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
            .filter(membership_policy_translation::Column::Locale.eq(request.policy.locale.clone()))
            .one(&transaction)
            .await?;
        let questions_value = serde_json::to_value(&request.policy.questions).map_err(|error| {
            GroupsError::Invariant(format!(
                "application questions are not serializable: {error}"
            ))
        })?;
        let rules_value = serde_json::to_value(&request.policy.rules).map_err(|error| {
            GroupsError::Invariant(format!("application rules are not serializable: {error}"))
        })?;
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
                locale: Set(request.policy.locale.clone()),
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
                group_id: request.policy.group_id,
                revision: policy_model.revision.max(1) as u64,
                enabled: policy_model.enabled,
                locale: request.policy.locale,
                questions: request.policy.questions,
                rules: request.policy.rules,
            },
            group_version,
            created,
            replayed: false,
        };
        append_audit(
            &transaction,
            context,
            tenant_id,
            result.policy.group_id,
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
                "group_version": group_version,
                "expected_revision_enforced": true
            }),
        )
        .await?;
        store_receipt(
            &transaction,
            tenant_id,
            result.policy.group_id,
            actor_user_id,
            idempotency_key,
            UPSERT_POLICY_IF_CURRENT_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }

    pub(crate) async fn submit_application_if_current_effective_owned(
        &self,
        context: &PortContext,
        mut request: SubmitGroupMembershipApplicationIfCurrentRequest,
    ) -> GroupsResult<SubmitGroupMembershipApplicationResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        normalize_policy_precondition(&mut request.expected_policy)?;
        normalize_application_submission(&mut request.submission)?;
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;

        if let Some(mut replayed) = replay_receipt::<SubmitGroupMembershipApplicationResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            SUBMIT_APPLICATION_IF_CURRENT_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        let prelocked_application = find_candidate_application_for_update(
            &transaction,
            tenant_id,
            request.submission.group_id,
            actor_user_id,
        )
        .await?;
        let group_model =
            find_group_for_update(&transaction, tenant_id, request.submission.group_id).await?;
        require_application_group(&group_model)?;
        crate::effective_membership_guard::require_user_not_denied_owned(
            &transaction,
            tenant_id,
            request.submission.group_id,
            actor_user_id,
            true,
        )
        .await?;
        let policy = load_policy_for_locale(
            &transaction,
            tenant_id,
            request.submission.group_id,
            &context.locale,
        )
        .await?;
        ensure_loaded_policy_precondition(&request.expected_policy, &policy)?;
        if !policy.enabled {
            return Err(GroupsError::Conflict(
                "group membership applications are disabled".to_string(),
            ));
        }
        validate_submission(&policy, &request.submission)?;

        let existing_membership = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(request.submission.group_id))
            .filter(membership::Column::UserId.eq(actor_user_id))
            .one(&transaction)
            .await?;
        let existing_application = match prelocked_application {
            Some(existing) => Some(existing),
            None => {
                find_candidate_application_for_update(
                    &transaction,
                    tenant_id,
                    request.submission.group_id,
                    actor_user_id,
                )
                .await?
            }
        };
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
                group_id: Set(request.submission.group_id),
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
        let answers_value = serde_json::to_value(&request.submission.answers).map_err(|error| {
            GroupsError::Invariant(format!("application answers are not serializable: {error}"))
        })?;
        let acknowledged_value = serde_json::to_value(&request.submission.acknowledged_rule_keys)
            .map_err(|error| {
            GroupsError::Invariant(format!(
                "application acknowledgements are not serializable: {error}"
            ))
        })?;
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
                group_id: Set(request.submission.group_id),
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
            request.submission.group_id,
            actor_user_id,
            "group.membership_application_submitted",
            Some(actor_user_id),
            json!({
                "application_id": result.application.id,
                "policy_id": result.application.policy_id,
                "policy_revision": result.application.policy_revision,
                "policy_locale": result.application.policy_locale,
                "group_version": group_version,
                "expected_revision_enforced": true,
                "application_lock_order": "application_then_group_when_existing"
            }),
        )
        .await?;
        store_receipt(
            &transaction,
            tenant_id,
            request.submission.group_id,
            actor_user_id,
            idempotency_key,
            SUBMIT_APPLICATION_IF_CURRENT_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }
}
    pub(crate) fn map_effective_application_cas_error(error: GroupsError) -> PortError {
    map_application_cas_error(error)
}
    impl GroupApplicationService {
    pub(crate) async fn cancel_application_effective_owned(
        &self,
        context: &PortContext,
        request: CancelGroupMembershipApplicationRequest,
    ) -> GroupsResult<GroupApplicationLifecycleResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;

        if let Some(mut replayed) = replay_receipt::<GroupApplicationLifecycleResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            CANCEL_APPLICATION_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        let application_model =
            find_application_for_update(&transaction, tenant_id, request.application_id).await?;
        if application_model.user_id != actor_user_id {
            return Err(GroupsError::NotFound);
        }
        let group_model =
            find_group_for_update(&transaction, tenant_id, application_model.group_id).await?;
        require_active_group(&group_model)?;
        crate::effective_membership_guard::require_user_not_denied_owned(
            &transaction,
            tenant_id,
            application_model.group_id,
            actor_user_id,
            false,
        )
        .await?;
        if application_model.status != GroupApplicationStatus::Pending.as_str() {
            return Err(GroupsError::Conflict(
                "only a pending membership application can be cancelled".to_string(),
            ));
        }
        let membership_model = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(application_model.group_id))
            .filter(membership::Column::UserId.eq(actor_user_id))
            .one(&transaction)
            .await?
            .ok_or_else(|| {
                GroupsError::Invariant(
                    "pending application membership is missing during cancellation".to_string(),
                )
            })?;
        if membership_model.status != GroupMembershipStatus::Pending.as_str() {
            return Err(GroupsError::Conflict(
                "membership is no longer pending".to_string(),
            ));
        }

        let now = Utc::now();
        let mut membership_active: membership::ActiveModel = membership_model.into();
        membership_active.status = Set(GroupMembershipStatus::Left.as_str().to_string());
        membership_active.joined_at = Set(None);
        membership_active.left_at = Set(Some(now.fixed_offset()));
        membership_active.updated_at = Set(now.fixed_offset());
        let membership_model = membership_active.update(&transaction).await?;

        let mut application_active: membership_application::ActiveModel = application_model.into();
        application_active.status = Set(GroupApplicationStatus::Cancelled.as_str().to_string());
        application_active.reviewed_at = Set(None);
        application_active.reviewed_by_user_id = Set(None);
        application_active.review_note = Set(None);
        application_active.updated_at = Set(now.fixed_offset());
        let application_model = application_active.update(&transaction).await?;

        let group_version = increment_group_version(&transaction, group_model, now).await?;
        let result = GroupApplicationLifecycleResult {
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
            "group.membership_application_cancelled",
            Some(actor_user_id),
            json!({
                "application_id": result.application.id,
                "previous_status": "pending",
                "membership_status": result.membership.status,
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
            CANCEL_APPLICATION_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }

    pub(crate) async fn reopen_application_effective_owned(
        &self,
        context: &PortContext,
        request: ReopenGroupMembershipApplicationRequest,
    ) -> GroupsResult<GroupApplicationLifecycleResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;

        if let Some(mut replayed) = replay_receipt::<GroupApplicationLifecycleResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            REOPEN_APPLICATION_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        let application_model =
            find_application_for_update(&transaction, tenant_id, request.application_id).await?;
        let group_model =
            find_group_for_update(&transaction, tenant_id, application_model.group_id).await?;
        require_application_group(&group_model)?;
        crate::effective_membership_guard::require_effective_manager_owned(
            &transaction,
            context,
            tenant_id,
            application_model.group_id,
            actor_user_id,
            crate::effective_membership_guard::GroupManagerCapability::Moderate,
        )
        .await?;
        crate::effective_membership_guard::require_user_not_denied_owned(
            &transaction,
            tenant_id,
            application_model.group_id,
            application_model.user_id,
            true,
        )
        .await?;

        let previous_status = GroupApplicationStatus::from_str(&application_model.status)
            .map_err(GroupsError::Invariant)?;
        if !matches!(
            previous_status,
            GroupApplicationStatus::Rejected | GroupApplicationStatus::Cancelled
        ) {
            return Err(GroupsError::Conflict(
                "only a rejected or cancelled membership application can be reopened".to_string(),
            ));
        }
        let membership_model = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(application_model.group_id))
            .filter(membership::Column::UserId.eq(application_model.user_id))
            .one(&transaction)
            .await?
            .ok_or_else(|| {
                GroupsError::Invariant(
                    "application membership is missing during reopen".to_string(),
                )
            })?;
        if membership_model.status != GroupMembershipStatus::Left.as_str() {
            return Err(GroupsError::Conflict(
                "membership must be left before the application can be reopened".to_string(),
            ));
        }

        let now = Utc::now();
        let target_user_id = application_model.user_id;
        let mut membership_active: membership::ActiveModel = membership_model.into();
        membership_active.role = Set(GroupRole::Member.as_str().to_string());
        membership_active.status = Set(GroupMembershipStatus::Pending.as_str().to_string());
        membership_active.joined_at = Set(None);
        membership_active.left_at = Set(None);
        membership_active.updated_at = Set(now.fixed_offset());
        let membership_model = membership_active.update(&transaction).await?;

        let mut application_active: membership_application::ActiveModel = application_model.into();
        application_active.status = Set(GroupApplicationStatus::Pending.as_str().to_string());
        application_active.reviewed_at = Set(None);
        application_active.reviewed_by_user_id = Set(None);
        application_active.review_note = Set(None);
        application_active.updated_at = Set(now.fixed_offset());
        let application_model = application_active.update(&transaction).await?;

        let group_version = increment_group_version(&transaction, group_model, now).await?;
        let result = GroupApplicationLifecycleResult {
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
            "group.membership_application_reopened",
            Some(target_user_id),
            json!({
                "application_id": result.application.id,
                "previous_status": previous_status.as_str(),
                "policy_id": result.application.policy_id,
                "policy_revision": result.application.policy_revision,
                "policy_locale": result.application.policy_locale,
                "snapshot_preserved": true,
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
            REOPEN_APPLICATION_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }
}
}
pub mod domain;
pub mod dto;
mod effective_applications;
mod effective_invitations;
mod effective_membership_guard;
mod effective_service;
pub mod entities;
pub mod error;
pub mod governance;
pub mod governance_entities;
#[cfg(feature = "graphql")]
pub mod graphql;
#[cfg(feature = "graphql")]
pub mod graphql_application_cas;
#[cfg(feature = "graphql")]
pub mod graphql_application_lifecycle;
#[cfg(feature = "graphql")]
pub mod graphql_application_policy_management;
#[cfg(feature = "graphql")]
pub mod graphql_applications;
#[cfg(feature = "graphql")]
pub mod graphql_governance;
#[cfg(feature = "graphql")]
pub mod graphql_invitations;
#[cfg(feature = "graphql")]
pub mod graphql_localization;
#[cfg(feature = "graphql")]
pub mod graphql_membership_enforcement;
#[cfg(feature = "graphql")]
pub mod graphql_policy_history;
pub mod group_event_entities;
pub mod invitation_entities;
mod invitations_legacy {
    use std::str::FromStr;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction,
    DbBackend, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set,
    TransactionTrait,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::domain::{GroupMembershipStatus, GroupRole, GroupStatus};
use crate::dto::GroupMembership;
use crate::entities::{group, membership};
use crate::error::{GroupsError, GroupsResult};
use crate::governance_entities::{audit_entry, command_receipt};
use crate::invitation_entities::{invitation, redemption};

const CREATE_INVITATION_COMMAND: &str = "groups.create_invitation.v1";
const REVOKE_INVITATION_COMMAND: &str = "groups.revoke_invitation.v1";
const ACCEPT_INVITATION_COMMAND: &str = "groups.accept_invitation.v1";
const MIN_EXPIRY_SECONDS: u64 = 300;
const MAX_EXPIRY_SECONDS: u64 = 30 * 24 * 60 * 60;
const MAX_INVITATION_USES: u32 = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupInvitationStatus {
    Active,
    Exhausted,
    Revoked,
    Expired,
}

impl GroupInvitationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Exhausted => "exhausted",
            Self::Revoked => "revoked",
            Self::Expired => "expired",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupInvitation {
    pub id: Uuid,
    pub group_id: Uuid,
    pub invited_by_user_id: Uuid,
    pub target_user_id: Option<Uuid>,
    pub max_uses: u32,
    pub use_count: u32,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub revoked_by_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub status: GroupInvitationStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupInvitationConnection {
    pub items: Vec<GroupInvitation>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListGroupInvitationsRequest {
    pub group_id: Uuid,
    pub page: u64,
    pub per_page: u64,
    pub include_inactive: bool,
}

impl ListGroupInvitationsRequest {
    pub fn validate(&self) -> GroupsResult<()> {
        if self.group_id.is_nil() {
            return Err(GroupsError::Validation(
                "group_id cannot be nil UUID".to_string(),
            ));
        }
        if self.page == 0 {
            return Err(GroupsError::Validation(
                "page must be greater than zero".to_string(),
            ));
        }
        if self.per_page == 0 || self.per_page > 100 {
            return Err(GroupsError::Validation(
                "per_page must be between 1 and 100".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateGroupInvitationRequest {
    pub group_id: Uuid,
    pub target_user_id: Option<Uuid>,
    pub expires_in_seconds: u64,
    pub max_uses: u32,
}

impl CreateGroupInvitationRequest {
    pub fn validate(&self) -> GroupsResult<()> {
        if self.group_id.is_nil() {
            return Err(GroupsError::Validation(
                "group_id cannot be nil UUID".to_string(),
            ));
        }
        if self
            .target_user_id
            .is_some_and(|target_user_id| target_user_id.is_nil())
        {
            return Err(GroupsError::Validation(
                "target_user_id cannot be nil UUID".to_string(),
            ));
        }
        if !(MIN_EXPIRY_SECONDS..=MAX_EXPIRY_SECONDS).contains(&self.expires_in_seconds) {
            return Err(GroupsError::Validation(format!(
                "invitation expiry must be between {MIN_EXPIRY_SECONDS} and {MAX_EXPIRY_SECONDS} seconds"
            )));
        }
        if !(1..=MAX_INVITATION_USES).contains(&self.max_uses) {
            return Err(GroupsError::Validation(format!(
                "invitation max_uses must be between 1 and {MAX_INVITATION_USES}"
            )));
        }
        if self.target_user_id.is_some() && self.max_uses != 1 {
            return Err(GroupsError::Validation(
                "a targeted invitation must have max_uses equal to 1".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateGroupInvitationResult {
    pub invitation: GroupInvitation,
    pub token: Option<String>,
    pub group_version: u64,
    pub replayed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevokeGroupInvitationRequest {
    pub invitation_id: Uuid,
}

impl RevokeGroupInvitationRequest {
    pub fn validate(&self) -> GroupsResult<()> {
        if self.invitation_id.is_nil() {
            return Err(GroupsError::Validation(
                "invitation_id cannot be nil UUID".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevokeGroupInvitationResult {
    pub invitation: GroupInvitation,
    pub group_version: u64,
    pub replayed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptGroupInvitationRequest {
    pub token: String,
}

impl AcceptGroupInvitationRequest {
    pub fn validate(&self) -> GroupsResult<()> {
        let token = self.token.trim();
        if token.is_empty() {
            return Err(invalid_invitation_token());
        }
        if token.len() < 32 || token.len() > 160 {
            return Err(invalid_invitation_token());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptGroupInvitationResult {
    pub invitation_id: Uuid,
    pub group_id: Uuid,
    pub membership: GroupMembership,
    pub group_version: u64,
    pub replayed: bool,
}

#[async_trait]
pub trait GroupInvitationReadPort: Send + Sync {
    async fn list_group_invitations(
        &self,
        context: PortContext,
        request: ListGroupInvitationsRequest,
    ) -> Result<GroupInvitationConnection, PortError>;
}

#[async_trait]
pub trait GroupInvitationCommandPort: Send + Sync {
    async fn create_group_invitation(
        &self,
        context: PortContext,
        request: CreateGroupInvitationRequest,
    ) -> Result<CreateGroupInvitationResult, PortError>;

    async fn revoke_group_invitation(
        &self,
        context: PortContext,
        request: RevokeGroupInvitationRequest,
    ) -> Result<RevokeGroupInvitationResult, PortError>;

    async fn accept_group_invitation(
        &self,
        context: PortContext,
        request: AcceptGroupInvitationRequest,
    ) -> Result<AcceptGroupInvitationResult, PortError>;
}

#[derive(Clone)]
pub struct GroupInvitationService {
    db: DatabaseConnection,
}

impl GroupInvitationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn list_group_invitations_owned(
        &self,
        context: &PortContext,
        request: ListGroupInvitationsRequest,
    ) -> GroupsResult<GroupInvitationConnection> {
        require_read(context)?;
        request.validate()?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let group_model = group::Entity::find()
            .filter(group::Column::TenantId.eq(tenant_id))
            .filter(group::Column::Id.eq(request.group_id))
            .one(&self.db)
            .await?
            .ok_or(GroupsError::NotFound)?;
        authorize_invitation_management(
            &self.db,
            context,
            tenant_id,
            group_model.id,
            actor_user_id,
        )
        .await?;

        let page = request.page.max(1);
        let per_page = request.per_page.clamp(1, 100);
        let now = Utc::now().fixed_offset();
        let mut query = invitation::Entity::find()
            .filter(invitation::Column::TenantId.eq(tenant_id))
            .filter(invitation::Column::GroupId.eq(request.group_id));
        if !request.include_inactive {
            query = query
                .filter(invitation::Column::RevokedAt.is_null())
                .filter(invitation::Column::ExpiresAt.gt(now))
                .filter(sea_orm::sea_query::ExprTrait::lt(
                    Expr::col(invitation::Column::UseCount),
                    Expr::col(invitation::Column::MaxUses),
                ));
        }
        let paginator = query
            .order_by_desc(invitation::Column::CreatedAt)
            .paginate(&self.db, per_page);
        let total = paginator.num_items().await?;
        let items = paginator
            .fetch_page(page.saturating_sub(1))
            .await?
            .into_iter()
            .map(map_invitation)
            .collect::<GroupsResult<Vec<_>>>()?;

        Ok(GroupInvitationConnection {
            items,
            total,
            page,
            per_page,
        })
    }

    async fn create_group_invitation_owned(
        &self,
        context: &PortContext,
        request: CreateGroupInvitationRequest,
    ) -> GroupsResult<CreateGroupInvitationResult> {
        require_write(context)?;
        validate_create_request(&request)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;
        let group_model = find_group_for_update(&transaction, tenant_id, request.group_id).await?;

        if let Some(mut replayed) = replay_receipt::<CreateGroupInvitationResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            CREATE_INVITATION_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.token = None;
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        require_active_group(&group_model)?;
        authorize_invitation_management_in_transaction(
            &transaction,
            context,
            tenant_id,
            request.group_id,
            actor_user_id,
        )
        .await?;

        let now = Utc::now();
        let expires_at = now + Duration::seconds(request.expires_in_seconds as i64);
        let token = generate_invitation_token();
        let token_hash = invitation_token_hash(&token);
        let invitation_model = invitation::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            group_id: Set(request.group_id),
            invited_by_user_id: Set(actor_user_id),
            target_user_id: Set(request.target_user_id),
            token_hash: Set(token_hash),
            max_uses: Set(request.max_uses as i32),
            use_count: Set(0),
            expires_at: Set(expires_at.fixed_offset()),
            revoked_at: Set(None),
            revoked_by_user_id: Set(None),
            created_at: Set(now.fixed_offset()),
            updated_at: Set(now.fixed_offset()),
        }
        .insert(&transaction)
        .await?;

        let group_version = increment_group_version(&transaction, group_model, now).await?;
        let result = CreateGroupInvitationResult {
            invitation: map_invitation(invitation_model)?,
            token: Some(token),
            group_version,
            replayed: false,
        };
        append_audit(
            &transaction,
            context,
            tenant_id,
            request.group_id,
            actor_user_id,
            "group.invitation_created",
            request.target_user_id,
            json!({
                "invitation_id": result.invitation.id,
                "target_user_id": request.target_user_id,
                "max_uses": request.max_uses,
                "expires_at": expires_at,
                "group_version": group_version
            }),
        )
        .await?;
        let mut stored_result = result.clone();
        stored_result.token = None;
        store_receipt(
            &transaction,
            tenant_id,
            request.group_id,
            actor_user_id,
            idempotency_key,
            CREATE_INVITATION_COMMAND,
            request_hash,
            &stored_result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }

    async fn revoke_group_invitation_owned(
        &self,
        context: &PortContext,
        request: RevokeGroupInvitationRequest,
    ) -> GroupsResult<RevokeGroupInvitationResult> {
        require_write(context)?;
        request.validate()?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;
        let invitation_model =
            find_invitation_for_update(&transaction, tenant_id, request.invitation_id).await?;

        if let Some(mut replayed) = replay_receipt::<RevokeGroupInvitationResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            REVOKE_INVITATION_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }
        if invitation_model.revoked_at.is_some() {
            return Err(GroupsError::Conflict(
                "group invitation is already revoked".to_string(),
            ));
        }

        let group_model =
            find_group_for_update(&transaction, tenant_id, invitation_model.group_id).await?;
        authorize_invitation_management_in_transaction(
            &transaction,
            context,
            tenant_id,
            invitation_model.group_id,
            actor_user_id,
        )
        .await?;
        let now = Utc::now();
        let mut active: invitation::ActiveModel = invitation_model.into();
        active.revoked_at = Set(Some(now.fixed_offset()));
        active.revoked_by_user_id = Set(Some(actor_user_id));
        active.updated_at = Set(now.fixed_offset());
        let revoked = active.update(&transaction).await?;
        let group_version = increment_group_version(&transaction, group_model, now).await?;
        let result = RevokeGroupInvitationResult {
            invitation: map_invitation(revoked)?,
            group_version,
            replayed: false,
        };
        append_audit(
            &transaction,
            context,
            tenant_id,
            result.invitation.group_id,
            actor_user_id,
            "group.invitation_revoked",
            result.invitation.target_user_id,
            json!({
                "invitation_id": result.invitation.id,
                "group_version": group_version
            }),
        )
        .await?;
        store_receipt(
            &transaction,
            tenant_id,
            result.invitation.group_id,
            actor_user_id,
            idempotency_key,
            REVOKE_INVITATION_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }

    async fn accept_group_invitation_owned(
        &self,
        context: &PortContext,
        request: AcceptGroupInvitationRequest,
    ) -> GroupsResult<AcceptGroupInvitationResult> {
        require_write(context)?;
        request.validate()?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let token = request.token.trim();
        if token.len() < 32 || token.len() > 160 {
            return Err(invalid_invitation_token());
        }
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;

        if let Some(mut replayed) = replay_receipt::<AcceptGroupInvitationResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            ACCEPT_INVITATION_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        let token_hash = invitation_token_hash(token);
        let invitation_model =
            find_invitation_by_token_for_update(&transaction, tenant_id, &token_hash).await?;
        ensure_invitation_active(&invitation_model, actor_user_id)?;
        let group_model =
            find_group_for_update(&transaction, tenant_id, invitation_model.group_id).await?;
        require_active_group(&group_model)?;

        if redemption::Entity::find()
            .filter(redemption::Column::TenantId.eq(tenant_id))
            .filter(redemption::Column::InvitationId.eq(invitation_model.id))
            .filter(redemption::Column::UserId.eq(actor_user_id))
            .one(&transaction)
            .await?
            .is_some()
        {
            return Err(GroupsError::Conflict(
                "group invitation was already accepted by this user".to_string(),
            ));
        }

        let existing_membership = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(invitation_model.group_id))
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

        let now = Utc::now();
        let membership_model = if let Some(existing) = existing_membership {
            let mut active: membership::ActiveModel = existing.into();
            active.role = Set(GroupRole::Member.as_str().to_string());
            active.status = Set(GroupMembershipStatus::Active.as_str().to_string());
            active.invited_by_user_id = Set(Some(invitation_model.invited_by_user_id));
            active.joined_at = Set(Some(now.fixed_offset()));
            active.left_at = Set(None);
            active.updated_at = Set(now.fixed_offset());
            active.update(&transaction).await?
        } else {
            membership::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                group_id: Set(invitation_model.group_id),
                user_id: Set(actor_user_id),
                role: Set(GroupRole::Member.as_str().to_string()),
                status: Set(GroupMembershipStatus::Active.as_str().to_string()),
                invited_by_user_id: Set(Some(invitation_model.invited_by_user_id)),
                joined_at: Set(Some(now.fixed_offset())),
                left_at: Set(None),
                metadata: Set(json!({})),
                created_at: Set(now.fixed_offset()),
                updated_at: Set(now.fixed_offset()),
            }
            .insert(&transaction)
            .await?
        };

        redemption::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            invitation_id: Set(invitation_model.id),
            group_id: Set(invitation_model.group_id),
            user_id: Set(actor_user_id),
            redeemed_at: Set(now.fixed_offset()),
        }
        .insert(&transaction)
        .await?;

        let invitation_id = invitation_model.id;
        let group_id = invitation_model.group_id;
        let next_use_count = invitation_model.use_count.saturating_add(1);
        let mut invitation_active: invitation::ActiveModel = invitation_model.into();
        invitation_active.use_count = Set(next_use_count);
        invitation_active.updated_at = Set(now.fixed_offset());
        invitation_active.update(&transaction).await?;

        let group_version =
            increment_group_membership_version(&transaction, group_model, now).await?;
        let result = AcceptGroupInvitationResult {
            invitation_id,
            group_id,
            membership: map_membership(membership_model)?,
            group_version,
            replayed: false,
        };
        append_audit(
            &transaction,
            context,
            tenant_id,
            group_id,
            actor_user_id,
            "group.invitation_accepted",
            Some(actor_user_id),
            json!({
                "invitation_id": invitation_id,
                "use_count": next_use_count,
                "group_version": group_version
            }),
        )
        .await?;
        store_receipt(
            &transaction,
            tenant_id,
            group_id,
            actor_user_id,
            idempotency_key,
            ACCEPT_INVITATION_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }
}

#[async_trait]
impl GroupInvitationReadPort for GroupInvitationService {
    async fn list_group_invitations(
        &self,
        context: PortContext,
        request: ListGroupInvitationsRequest,
    ) -> Result<GroupInvitationConnection, PortError> {
        request.validate().map_err(PortError::from)?;
        self.list_group_invitations_owned(&context, request)
            .await
            .map_err(Into::into)
    }
}

#[async_trait]
impl GroupInvitationCommandPort for GroupInvitationService {
    async fn create_group_invitation(
        &self,
        context: PortContext,
        request: CreateGroupInvitationRequest,
    ) -> Result<CreateGroupInvitationResult, PortError> {
        request.validate().map_err(PortError::from)?;
        self.create_group_invitation_owned(&context, request)
            .await
            .map_err(Into::into)
    }

    async fn revoke_group_invitation(
        &self,
        context: PortContext,
        request: RevokeGroupInvitationRequest,
    ) -> Result<RevokeGroupInvitationResult, PortError> {
        request.validate().map_err(PortError::from)?;
        self.revoke_group_invitation_owned(&context, request)
            .await
            .map_err(Into::into)
    }

    async fn accept_group_invitation(
        &self,
        context: PortContext,
        request: AcceptGroupInvitationRequest,
    ) -> Result<AcceptGroupInvitationResult, PortError> {
        request.validate().map_err(PortError::from)?;
        self.accept_group_invitation_owned(&context, request)
            .await
            .map_err(Into::into)
    }
}

fn validate_create_request(request: &CreateGroupInvitationRequest) -> GroupsResult<()> {
    request.validate()
}

async fn authorize_invitation_management<C: sea_orm::ConnectionTrait>(
    connection: &C,
    context: &PortContext,
    tenant_id: Uuid,
    group_id: Uuid,
    actor_user_id: Uuid,
) -> GroupsResult<()> {
    if has_platform_manage(context) {
        return Ok(());
    }
    let model = membership::Entity::find()
        .filter(membership::Column::TenantId.eq(tenant_id))
        .filter(membership::Column::GroupId.eq(group_id))
        .filter(membership::Column::UserId.eq(actor_user_id))
        .one(connection)
        .await?;
    let allowed = model
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

async fn authorize_invitation_management_in_transaction(
    transaction: &DatabaseTransaction,
    context: &PortContext,
    tenant_id: Uuid,
    group_id: Uuid,
    actor_user_id: Uuid,
) -> GroupsResult<()> {
    authorize_invitation_management(transaction, context, tenant_id, group_id, actor_user_id).await
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
        _ => unreachable!("unsupported SeaORM database backend"),
    }
    .ok_or(GroupsError::NotFound)
}

async fn find_invitation_for_update(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    invitation_id: Uuid,
) -> GroupsResult<invitation::Model> {
    let query = || {
        invitation::Entity::find()
            .filter(invitation::Column::TenantId.eq(tenant_id))
            .filter(invitation::Column::Id.eq(invitation_id))
    };
    match transaction.get_database_backend() {
        DbBackend::Sqlite => query().one(transaction).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().one(transaction).await?,
        _ => unreachable!("unsupported SeaORM database backend"),
    }
    .ok_or(GroupsError::NotFound)
}

async fn find_invitation_by_token_for_update(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    token_hash: &str,
) -> GroupsResult<invitation::Model> {
    let query = || {
        invitation::Entity::find()
            .filter(invitation::Column::TenantId.eq(tenant_id))
            .filter(invitation::Column::TokenHash.eq(token_hash))
    };
    match transaction.get_database_backend() {
        DbBackend::Sqlite => query().one(transaction).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().one(transaction).await?,
        _ => unreachable!("unsupported SeaORM database backend"),
    }
    .ok_or_else(invalid_invitation_token)
}

fn require_active_group(model: &group::Model) -> GroupsResult<()> {
    if model.status == GroupStatus::Active.as_str() {
        Ok(())
    } else {
        Err(GroupsError::Conflict("group is not active".to_string()))
    }
}

fn ensure_invitation_active(model: &invitation::Model, actor_user_id: Uuid) -> GroupsResult<()> {
    if model.revoked_at.is_some()
        || model.expires_at.with_timezone(&Utc) <= Utc::now()
        || model.use_count >= model.max_uses
        || model
            .target_user_id
            .is_some_and(|target_user_id| target_user_id != actor_user_id)
    {
        Err(invalid_invitation_token())
    } else {
        Ok(())
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

fn map_invitation(model: invitation::Model) -> GroupsResult<GroupInvitation> {
    let now = Utc::now();
    let status = if model.revoked_at.is_some() {
        GroupInvitationStatus::Revoked
    } else if model.expires_at.with_timezone(&Utc) <= now {
        GroupInvitationStatus::Expired
    } else if model.use_count >= model.max_uses {
        GroupInvitationStatus::Exhausted
    } else {
        GroupInvitationStatus::Active
    };
    Ok(GroupInvitation {
        id: model.id,
        group_id: model.group_id,
        invited_by_user_id: model.invited_by_user_id,
        target_user_id: model.target_user_id,
        max_uses: model.max_uses.max(0) as u32,
        use_count: model.use_count.max(0) as u32,
        expires_at: model.expires_at.with_timezone(&Utc),
        revoked_at: model.revoked_at.map(|value| value.with_timezone(&Utc)),
        revoked_by_user_id: model.revoked_by_user_id,
        created_at: model.created_at.with_timezone(&Utc),
        status,
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

fn generate_invitation_token() -> String {
    format!("gri_{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn invitation_token_hash(token: &str) -> String {
    crate::domain::sha256_hex(token.as_bytes())
}

fn invalid_invitation_token() -> GroupsError {
    GroupsError::Conflict("group invitation token is invalid or unavailable".to_string())
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
            GroupsError::Invariant(format!(
                "group command response is not serializable: {error}"
            ))
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
        GroupsError::Validation(format!(
            "group command request is not serializable: {error}"
        ))
    })?;
    Ok(crate::domain::sha256_hex(&bytes))
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
    let trimmed = context.tenant_id.trim();
    if trimmed.is_empty() {
        return Err(GroupsError::Validation(
            "tenant_id cannot be empty".to_string(),
        ));
    }
    let parsed = Uuid::parse_str(trimmed)
        .map_err(|_| GroupsError::Validation("tenant_id must be a UUID".to_string()))?;
    if parsed.is_nil() {
        return Err(GroupsError::Validation(
            "tenant_id cannot be nil UUID".to_string(),
        ));
    }
    Ok(parsed)
}

fn actor_user_id(context: &PortContext) -> GroupsResult<Uuid> {
    if context.actor.kind != PortActorKind::User {
        return Err(GroupsError::Forbidden(
            "a user actor is required for group invitations".to_string(),
        ));
    }
    let trimmed = context.actor.id.trim();
    if trimmed.is_empty() {
        return Err(GroupsError::Validation(
            "actor.id cannot be empty".to_string(),
        ));
    }
    let parsed = Uuid::parse_str(trimmed)
        .map_err(|_| GroupsError::Validation("actor.id must be a UUID".to_string()))?;
    if parsed.is_nil() {
        return Err(GroupsError::Validation(
            "actor.id cannot be nil UUID".to_string(),
        ));
    }
    Ok(parsed)
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
    impl GroupInvitationService {
    pub(crate) async fn create_group_invitation_effective_owned(
        &self,
        context: &PortContext,
        request: CreateGroupInvitationRequest,
    ) -> GroupsResult<CreateGroupInvitationResult> {
        require_write(context)?;
        validate_create_request(&request)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;
        let group_model = find_group_for_update(&transaction, tenant_id, request.group_id).await?;

        if let Some(mut replayed) = replay_receipt::<CreateGroupInvitationResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            CREATE_INVITATION_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.token = None;
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        require_active_group(&group_model)?;
        crate::effective_membership_guard::require_effective_manager_owned(
            &transaction,
            context,
            tenant_id,
            request.group_id,
            actor_user_id,
            crate::effective_membership_guard::GroupManagerCapability::Moderate,
        )
        .await?;

        let now = Utc::now();
        let expires_at = now + Duration::seconds(request.expires_in_seconds as i64);
        let token = generate_invitation_token();
        let token_hash = invitation_token_hash(&token);
        let invitation_model = invitation::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            group_id: Set(request.group_id),
            invited_by_user_id: Set(actor_user_id),
            target_user_id: Set(request.target_user_id),
            token_hash: Set(token_hash),
            max_uses: Set(request.max_uses as i32),
            use_count: Set(0),
            expires_at: Set(expires_at.fixed_offset()),
            revoked_at: Set(None),
            revoked_by_user_id: Set(None),
            created_at: Set(now.fixed_offset()),
            updated_at: Set(now.fixed_offset()),
        }
        .insert(&transaction)
        .await?;

        let group_version = increment_group_version(&transaction, group_model, now).await?;
        let result = CreateGroupInvitationResult {
            invitation: map_invitation(invitation_model)?,
            token: Some(token),
            group_version,
            replayed: false,
        };
        append_audit(
            &transaction,
            context,
            tenant_id,
            request.group_id,
            actor_user_id,
            "group.invitation_created",
            request.target_user_id,
            json!({
                "invitation_id": result.invitation.id,
                "target_user_id": request.target_user_id,
                "max_uses": request.max_uses,
                "expires_at": expires_at,
                "group_version": group_version
            }),
        )
        .await?;
        let mut stored_result = result.clone();
        stored_result.token = None;
        store_receipt(
            &transaction,
            tenant_id,
            request.group_id,
            actor_user_id,
            idempotency_key,
            CREATE_INVITATION_COMMAND,
            request_hash,
            &stored_result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }

    pub(crate) async fn revoke_group_invitation_effective_owned(
        &self,
        context: &PortContext,
        request: RevokeGroupInvitationRequest,
    ) -> GroupsResult<RevokeGroupInvitationResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;
        let invitation_model =
            find_invitation_for_update(&transaction, tenant_id, request.invitation_id).await?;

        if let Some(mut replayed) = replay_receipt::<RevokeGroupInvitationResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            REVOKE_INVITATION_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        let group_model =
            find_group_for_update(&transaction, tenant_id, invitation_model.group_id).await?;
        crate::effective_membership_guard::require_effective_manager_owned(
            &transaction,
            context,
            tenant_id,
            invitation_model.group_id,
            actor_user_id,
            crate::effective_membership_guard::GroupManagerCapability::Moderate,
        )
        .await?;
        if invitation_model.revoked_at.is_some() {
            return Err(GroupsError::Conflict(
                "group invitation is already revoked".to_string(),
            ));
        }

        let now = Utc::now();
        let mut active: invitation::ActiveModel = invitation_model.into();
        active.revoked_at = Set(Some(now.fixed_offset()));
        active.revoked_by_user_id = Set(Some(actor_user_id));
        active.updated_at = Set(now.fixed_offset());
        let revoked = active.update(&transaction).await?;
        let group_version = increment_group_version(&transaction, group_model, now).await?;
        let result = RevokeGroupInvitationResult {
            invitation: map_invitation(revoked)?,
            group_version,
            replayed: false,
        };
        append_audit(
            &transaction,
            context,
            tenant_id,
            result.invitation.group_id,
            actor_user_id,
            "group.invitation_revoked",
            result.invitation.target_user_id,
            json!({
                "invitation_id": result.invitation.id,
                "group_version": group_version
            }),
        )
        .await?;
        store_receipt(
            &transaction,
            tenant_id,
            result.invitation.group_id,
            actor_user_id,
            idempotency_key,
            REVOKE_INVITATION_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }

    pub(crate) async fn accept_group_invitation_effective_owned(
        &self,
        context: &PortContext,
        request: AcceptGroupInvitationRequest,
    ) -> GroupsResult<AcceptGroupInvitationResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let token = request.token.trim();
        if token.len() < 32 || token.len() > 160 {
            return Err(invalid_invitation_token());
        }
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;

        if let Some(mut replayed) = replay_receipt::<AcceptGroupInvitationResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            ACCEPT_INVITATION_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        let token_hash = invitation_token_hash(token);
        let invitation_model =
            find_invitation_by_token_for_update(&transaction, tenant_id, &token_hash).await?;
        let group_model =
            find_group_for_update(&transaction, tenant_id, invitation_model.group_id).await?;
        require_active_group(&group_model)?;
        crate::effective_membership_guard::require_user_not_denied_owned(
            &transaction,
            tenant_id,
            invitation_model.group_id,
            actor_user_id,
            true,
        )
        .await?;
        ensure_invitation_active(&invitation_model, actor_user_id)?;

        if redemption::Entity::find()
            .filter(redemption::Column::TenantId.eq(tenant_id))
            .filter(redemption::Column::InvitationId.eq(invitation_model.id))
            .filter(redemption::Column::UserId.eq(actor_user_id))
            .one(&transaction)
            .await?
            .is_some()
        {
            return Err(GroupsError::Conflict(
                "group invitation was already accepted by this user".to_string(),
            ));
        }

        let existing_membership = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(invitation_model.group_id))
            .filter(membership::Column::UserId.eq(actor_user_id))
            .one(&transaction)
            .await?;

        let now = Utc::now();
        let membership_model = if let Some(existing) = existing_membership {
            let mut active: membership::ActiveModel = existing.into();
            active.role = Set(GroupRole::Member.as_str().to_string());
            active.status = Set(GroupMembershipStatus::Active.as_str().to_string());
            active.invited_by_user_id = Set(Some(invitation_model.invited_by_user_id));
            active.joined_at = Set(Some(now.fixed_offset()));
            active.left_at = Set(None);
            active.updated_at = Set(now.fixed_offset());
            active.update(&transaction).await?
        } else {
            membership::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                group_id: Set(invitation_model.group_id),
                user_id: Set(actor_user_id),
                role: Set(GroupRole::Member.as_str().to_string()),
                status: Set(GroupMembershipStatus::Active.as_str().to_string()),
                invited_by_user_id: Set(Some(invitation_model.invited_by_user_id)),
                joined_at: Set(Some(now.fixed_offset())),
                left_at: Set(None),
                metadata: Set(json!({})),
                created_at: Set(now.fixed_offset()),
                updated_at: Set(now.fixed_offset()),
            }
            .insert(&transaction)
            .await?
        };

        redemption::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            invitation_id: Set(invitation_model.id),
            group_id: Set(invitation_model.group_id),
            user_id: Set(actor_user_id),
            redeemed_at: Set(now.fixed_offset()),
        }
        .insert(&transaction)
        .await?;

        let invitation_id = invitation_model.id;
        let group_id = invitation_model.group_id;
        let next_use_count = invitation_model.use_count.saturating_add(1);
        let mut invitation_active: invitation::ActiveModel = invitation_model.into();
        invitation_active.use_count = Set(next_use_count);
        invitation_active.updated_at = Set(now.fixed_offset());
        invitation_active.update(&transaction).await?;

        let group_version =
            increment_group_membership_version(&transaction, group_model, now).await?;
        let result = AcceptGroupInvitationResult {
            invitation_id,
            group_id,
            membership: map_membership(membership_model)?,
            group_version,
            replayed: false,
        };
        append_audit(
            &transaction,
            context,
            tenant_id,
            group_id,
            actor_user_id,
            "group.invitation_accepted",
            Some(actor_user_id),
            json!({
                "invitation_id": invitation_id,
                "use_count": next_use_count,
                "group_version": group_version
            }),
        )
        .await?;
        store_receipt(
            &transaction,
            tenant_id,
            group_id,
            actor_user_id,
            idempotency_key,
            ACCEPT_INVITATION_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }
}
}
pub mod localization;
pub mod membership_enforcement;
mod membership_enforcement_command;
pub mod membership_enforcement_entities;
mod membership_enforcement_transaction;
pub mod migrations;
mod moderation_subject;
pub mod module;
mod notification_source;
pub mod policy_history;
pub mod ports;
// Transitional status-only core implementation delegate. It is crate-private so external
// consumers and module-owned transports cannot bypass the effective membership facade.
mod service;
mod targeted_invitations_legacy {
    use std::str::FromStr;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction,
    DbBackend, EntityTrait, QueryFilter, QuerySelect, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::domain::{GroupMembershipStatus, GroupRole, GroupStatus};
use crate::dto::GroupMembership;
use crate::entities::{group, membership};
use crate::error::{GroupsError, GroupsResult};
use crate::governance_entities::{audit_entry, command_receipt};
use crate::invitation_entities::{invitation, redemption};
use crate::invitations::AcceptGroupInvitationResult;

const ACCEPT_TARGETED_INVITATION_COMMAND: &str = "groups.accept_targeted_invitation.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptTargetedGroupInvitationRequest {
    pub invitation_id: Uuid,
}

impl AcceptTargetedGroupInvitationRequest {
    pub fn validate(&self) -> GroupsResult<()> {
        if self.invitation_id.is_nil() {
            return Err(GroupsError::Validation(
                "invitation_id cannot be nil UUID".to_string(),
            ));
        }
        Ok(())
    }
}

#[async_trait]
pub trait GroupTargetedInvitationCommandPort: Send + Sync {
    async fn accept_targeted_group_invitation(
        &self,
        context: PortContext,
        request: AcceptTargetedGroupInvitationRequest,
    ) -> Result<AcceptGroupInvitationResult, PortError>;
}

#[derive(Clone)]
pub struct GroupTargetedInvitationService {
    db: DatabaseConnection,
}

impl GroupTargetedInvitationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn accept_owned(
        &self,
        context: &PortContext,
        request: AcceptTargetedGroupInvitationRequest,
    ) -> GroupsResult<AcceptGroupInvitationResult> {
        require_write(context)?;
        request.validate()?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;

        if let Some(mut replayed) = replay_receipt::<AcceptGroupInvitationResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            ACCEPT_TARGETED_INVITATION_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        let invitation_model =
            find_invitation_for_update(&transaction, tenant_id, request.invitation_id).await?;
        ensure_targeted_invitation_active(&invitation_model, actor_user_id)?;
        let group_model =
            find_group_for_update(&transaction, tenant_id, invitation_model.group_id).await?;
        require_active_group(&group_model)?;

        if redemption::Entity::find()
            .filter(redemption::Column::TenantId.eq(tenant_id))
            .filter(redemption::Column::InvitationId.eq(invitation_model.id))
            .filter(redemption::Column::UserId.eq(actor_user_id))
            .one(&transaction)
            .await?
            .is_some()
        {
            return Err(targeted_invitation_unavailable());
        }

        let existing_membership = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(invitation_model.group_id))
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

        let now = Utc::now();
        let membership_model = if let Some(existing) = existing_membership {
            let mut active: membership::ActiveModel = existing.into();
            active.role = Set(GroupRole::Member.as_str().to_string());
            active.status = Set(GroupMembershipStatus::Active.as_str().to_string());
            active.invited_by_user_id = Set(Some(invitation_model.invited_by_user_id));
            active.joined_at = Set(Some(now.fixed_offset()));
            active.left_at = Set(None);
            active.updated_at = Set(now.fixed_offset());
            active.update(&transaction).await?
        } else {
            membership::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                group_id: Set(invitation_model.group_id),
                user_id: Set(actor_user_id),
                role: Set(GroupRole::Member.as_str().to_string()),
                status: Set(GroupMembershipStatus::Active.as_str().to_string()),
                invited_by_user_id: Set(Some(invitation_model.invited_by_user_id)),
                joined_at: Set(Some(now.fixed_offset())),
                left_at: Set(None),
                metadata: Set(json!({})),
                created_at: Set(now.fixed_offset()),
                updated_at: Set(now.fixed_offset()),
            }
            .insert(&transaction)
            .await?
        };

        redemption::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            invitation_id: Set(invitation_model.id),
            group_id: Set(invitation_model.group_id),
            user_id: Set(actor_user_id),
            redeemed_at: Set(now.fixed_offset()),
        }
        .insert(&transaction)
        .await?;

        let invitation_id = invitation_model.id;
        let group_id = invitation_model.group_id;
        let mut invitation_active: invitation::ActiveModel = invitation_model.into();
        invitation_active.use_count = Set(1);
        invitation_active.updated_at = Set(now.fixed_offset());
        invitation_active.update(&transaction).await?;

        let group_version =
            increment_group_membership_version(&transaction, group_model, now).await?;
        let result = AcceptGroupInvitationResult {
            invitation_id,
            group_id,
            membership: map_membership(membership_model)?,
            group_version,
            replayed: false,
        };
        append_audit(
            &transaction,
            context,
            tenant_id,
            group_id,
            actor_user_id,
            "group.targeted_invitation_accepted",
            Some(actor_user_id),
            json!({
                "invitation_id": invitation_id,
                "group_version": group_version
            }),
        )
        .await?;
        store_receipt(
            &transaction,
            tenant_id,
            group_id,
            actor_user_id,
            idempotency_key,
            ACCEPT_TARGETED_INVITATION_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }
}

#[async_trait]
impl GroupTargetedInvitationCommandPort for GroupTargetedInvitationService {
    async fn accept_targeted_group_invitation(
        &self,
        context: PortContext,
        request: AcceptTargetedGroupInvitationRequest,
    ) -> Result<AcceptGroupInvitationResult, PortError> {
        request.validate().map_err(PortError::from)?;
        self.accept_owned(&context, request)
            .await
            .map_err(Into::into)
    }
}

async fn find_invitation_for_update(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    invitation_id: Uuid,
) -> GroupsResult<invitation::Model> {
    let query = || {
        invitation::Entity::find()
            .filter(invitation::Column::TenantId.eq(tenant_id))
            .filter(invitation::Column::Id.eq(invitation_id))
    };
    match transaction.get_database_backend() {
        DbBackend::Sqlite => query().one(transaction).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().one(transaction).await?,
        _ => unreachable!("unsupported SeaORM database backend"),
    }
    .ok_or_else(targeted_invitation_unavailable)
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
        _ => unreachable!("unsupported SeaORM database backend"),
    }
    .ok_or_else(targeted_invitation_unavailable)
}

fn ensure_targeted_invitation_active(
    model: &invitation::Model,
    actor_user_id: Uuid,
) -> GroupsResult<()> {
    if model.target_user_id != Some(actor_user_id)
        || model.max_uses != 1
        || model.revoked_at.is_some()
        || model.expires_at.with_timezone(&Utc) <= Utc::now()
        || model.use_count >= model.max_uses
    {
        Err(targeted_invitation_unavailable())
    } else {
        Ok(())
    }
}

fn require_active_group(model: &group::Model) -> GroupsResult<()> {
    if model.status == GroupStatus::Active.as_str() {
        Ok(())
    } else {
        Err(targeted_invitation_unavailable())
    }
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

fn map_membership(model: membership::Model) -> GroupsResult<GroupMembership> {
    Ok(GroupMembership {
        id: model.id,
        group_id: model.group_id,
        user_id: model.user_id,
        role: GroupRole::from_str(&model.role).map_err(GroupsError::Invariant)?,
        status: GroupMembershipStatus::from_str(&model.status).map_err(GroupsError::Invariant)?,
    })
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
            GroupsError::Invariant(format!(
                "group command response is not serializable: {error}"
            ))
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
        GroupsError::Validation(format!(
            "group command request is not serializable: {error}"
        ))
    })?;
    Ok(crate::domain::sha256_hex(&bytes))
}

fn require_write(context: &PortContext) -> GroupsResult<()> {
    context
        .require_policy(PortCallPolicy::write())
        .map_err(|error| GroupsError::Validation(error.message))
}

fn context_tenant_id(context: &PortContext) -> GroupsResult<Uuid> {
    let trimmed = context.tenant_id.trim();
    if trimmed.is_empty() {
        return Err(GroupsError::Validation(
            "tenant_id cannot be empty".to_string(),
        ));
    }
    let parsed = Uuid::parse_str(trimmed)
        .map_err(|_| GroupsError::Validation("tenant_id must be a UUID".to_string()))?;
    if parsed.is_nil() {
        return Err(GroupsError::Validation(
            "tenant_id cannot be nil UUID".to_string(),
        ));
    }
    Ok(parsed)
}

fn actor_user_id(context: &PortContext) -> GroupsResult<Uuid> {
    if context.actor.kind != PortActorKind::User {
        return Err(GroupsError::Forbidden(
            "a user actor is required for targeted group invitations".to_string(),
        ));
    }
    let trimmed = context.actor.id.trim();
    if trimmed.is_empty() {
        return Err(GroupsError::Validation(
            "actor.id cannot be empty".to_string(),
        ));
    }
    let parsed = Uuid::parse_str(trimmed)
        .map_err(|_| GroupsError::Validation("actor.id must be a UUID".to_string()))?;
    if parsed.is_nil() {
        return Err(GroupsError::Validation(
            "actor.id cannot be nil UUID".to_string(),
        ));
    }
    Ok(parsed)
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

fn targeted_invitation_unavailable() -> GroupsError {
    GroupsError::NotFound
}
    impl GroupTargetedInvitationService {
    pub(crate) async fn accept_targeted_group_invitation_effective_owned(
        &self,
        context: &PortContext,
        request: AcceptTargetedGroupInvitationRequest,
    ) -> GroupsResult<AcceptGroupInvitationResult> {
        require_write(context)?;
        let tenant_id = context_tenant_id(context)?;
        let actor_user_id = actor_user_id(context)?;
        let idempotency_key = idempotency_key(context)?;
        let request_hash = request_hash(&request)?;
        let transaction = self.db.begin().await?;

        if let Some(mut replayed) = replay_receipt::<AcceptGroupInvitationResult>(
            &transaction,
            tenant_id,
            actor_user_id,
            &idempotency_key,
            ACCEPT_TARGETED_INVITATION_COMMAND,
            &request_hash,
        )
        .await?
        {
            replayed.replayed = true;
            transaction.commit().await?;
            return Ok(replayed);
        }

        let invitation_model =
            find_invitation_for_update(&transaction, tenant_id, request.invitation_id).await?;
        let group_model =
            find_group_for_update(&transaction, tenant_id, invitation_model.group_id).await?;
        require_active_group(&group_model)?;
        crate::effective_membership_guard::require_user_not_denied_owned(
            &transaction,
            tenant_id,
            invitation_model.group_id,
            actor_user_id,
            true,
        )
        .await?;
        ensure_targeted_invitation_active(&invitation_model, actor_user_id)?;

        if redemption::Entity::find()
            .filter(redemption::Column::TenantId.eq(tenant_id))
            .filter(redemption::Column::InvitationId.eq(invitation_model.id))
            .filter(redemption::Column::UserId.eq(actor_user_id))
            .one(&transaction)
            .await?
            .is_some()
        {
            return Err(targeted_invitation_unavailable());
        }

        let existing_membership = membership::Entity::find()
            .filter(membership::Column::TenantId.eq(tenant_id))
            .filter(membership::Column::GroupId.eq(invitation_model.group_id))
            .filter(membership::Column::UserId.eq(actor_user_id))
            .one(&transaction)
            .await?;

        let now = Utc::now();
        let membership_model = if let Some(existing) = existing_membership {
            let mut active: membership::ActiveModel = existing.into();
            active.role = Set(GroupRole::Member.as_str().to_string());
            active.status = Set(GroupMembershipStatus::Active.as_str().to_string());
            active.invited_by_user_id = Set(Some(invitation_model.invited_by_user_id));
            active.joined_at = Set(Some(now.fixed_offset()));
            active.left_at = Set(None);
            active.updated_at = Set(now.fixed_offset());
            active.update(&transaction).await?
        } else {
            membership::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                group_id: Set(invitation_model.group_id),
                user_id: Set(actor_user_id),
                role: Set(GroupRole::Member.as_str().to_string()),
                status: Set(GroupMembershipStatus::Active.as_str().to_string()),
                invited_by_user_id: Set(Some(invitation_model.invited_by_user_id)),
                joined_at: Set(Some(now.fixed_offset())),
                left_at: Set(None),
                metadata: Set(json!({})),
                created_at: Set(now.fixed_offset()),
                updated_at: Set(now.fixed_offset()),
            }
            .insert(&transaction)
            .await?
        };

        redemption::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            invitation_id: Set(invitation_model.id),
            group_id: Set(invitation_model.group_id),
            user_id: Set(actor_user_id),
            redeemed_at: Set(now.fixed_offset()),
        }
        .insert(&transaction)
        .await?;

        let invitation_id = invitation_model.id;
        let group_id = invitation_model.group_id;
        let mut invitation_active: invitation::ActiveModel = invitation_model.into();
        invitation_active.use_count = Set(1);
        invitation_active.updated_at = Set(now.fixed_offset());
        invitation_active.update(&transaction).await?;

        let group_version =
            increment_group_membership_version(&transaction, group_model, now).await?;
        let result = AcceptGroupInvitationResult {
            invitation_id,
            group_id,
            membership: map_membership(membership_model)?,
            group_version,
            replayed: false,
        };
        append_audit(
            &transaction,
            context,
            tenant_id,
            group_id,
            actor_user_id,
            "group.targeted_invitation_accepted",
            Some(actor_user_id),
            json!({
                "invitation_id": invitation_id,
                "group_version": group_version
            }),
        )
        .await?;
        store_receipt(
            &transaction,
            tenant_id,
            group_id,
            actor_user_id,
            idempotency_key,
            ACCEPT_TARGETED_INVITATION_COMMAND,
            request_hash,
            &result,
        )
        .await?;
        transaction.commit().await?;
        Ok(result)
    }
}
}

/// Compatibility module preserving the public application types and paths while sealing the
/// status-only owner implementation behind the effective-membership facade.
pub mod applications {
    pub use crate::applications_legacy_module::{
        BulkReviewGroupMembershipApplicationItemResult,
        BulkReviewGroupMembershipApplicationsRequest, BulkReviewGroupMembershipApplicationsResult,
        CancelGroupMembershipApplicationRequest, GROUP_APPLICATION_POLICY_CHANGED_CODE,
        GroupApplicationBulkReviewCommandPort, GroupApplicationCasCommandPort,
        GroupApplicationCommandPort, GroupApplicationLifecycleCommandPort,
        GroupApplicationLifecycleReadPort, GroupApplicationLifecycleResult, GroupApplicationPolicy,
        GroupApplicationPolicyLocaleCatalog, GroupApplicationPolicyManagementReadPort,
        GroupApplicationPolicyManagementView, GroupApplicationPolicyPrecondition,
        GroupApplicationQuestion, GroupApplicationReadPort, GroupApplicationReviewCommandPort,
        GroupApplicationReviewDecision, GroupApplicationRule, GroupApplicationStatus,
        GroupMembershipApplication, GroupMembershipApplicationConnection,
        ListGroupApplicationPolicyLocalesRequest, ListGroupMembershipApplicationsRequest,
        ReadGroupApplicationPolicyForManagementRequest, ReadGroupApplicationPolicyRequest,
        ReadMyGroupMembershipApplicationRequest, ReopenGroupMembershipApplicationRequest,
        ReviewGroupMembershipApplicationRequest, ReviewGroupMembershipApplicationResult,
        SubmitGroupMembershipApplicationIfCurrentRequest, SubmitGroupMembershipApplicationRequest,
        SubmitGroupMembershipApplicationResult, UpsertGroupApplicationPolicyIfCurrentRequest,
        UpsertGroupApplicationPolicyRequest, UpsertGroupApplicationPolicyResult,
    };
    pub use crate::effective_applications::GroupApplicationService;
}

/// Compatibility module preserving invitation contracts while routing the service type through
/// effective group-membership authorization.
pub mod invitations {
    pub use crate::effective_invitations::GroupInvitationService;
    pub use crate::invitations_legacy::{
        AcceptGroupInvitationRequest, AcceptGroupInvitationResult, CreateGroupInvitationRequest,
        CreateGroupInvitationResult, GroupInvitation, GroupInvitationCommandPort,
        GroupInvitationConnection, GroupInvitationReadPort, GroupInvitationStatus,
        ListGroupInvitationsRequest, RevokeGroupInvitationRequest, RevokeGroupInvitationResult,
    };
}

/// Compatibility module preserving targeted-invitation contracts while sealing the legacy
/// implementation delegate.
pub mod targeted_invitations {
    pub use crate::effective_invitations::GroupTargetedInvitationService;
    pub use crate::targeted_invitations_legacy::{
        AcceptTargetedGroupInvitationRequest, GroupTargetedInvitationCommandPort,
    };
}

pub use applications::{
    BulkReviewGroupMembershipApplicationItemResult, BulkReviewGroupMembershipApplicationsRequest,
    BulkReviewGroupMembershipApplicationsResult, CancelGroupMembershipApplicationRequest,
    GROUP_APPLICATION_POLICY_CHANGED_CODE, GroupApplicationBulkReviewCommandPort,
    GroupApplicationCasCommandPort, GroupApplicationCommandPort,
    GroupApplicationLifecycleCommandPort, GroupApplicationLifecycleReadPort,
    GroupApplicationLifecycleResult, GroupApplicationPolicy, GroupApplicationPolicyLocaleCatalog,
    GroupApplicationPolicyManagementReadPort, GroupApplicationPolicyManagementView,
    GroupApplicationPolicyPrecondition, GroupApplicationQuestion, GroupApplicationReadPort,
    GroupApplicationReviewCommandPort, GroupApplicationReviewDecision, GroupApplicationRule,
    GroupApplicationService, GroupApplicationStatus, GroupMembershipApplication,
    GroupMembershipApplicationConnection, ListGroupApplicationPolicyLocalesRequest,
    ListGroupMembershipApplicationsRequest, ReadGroupApplicationPolicyForManagementRequest,
    ReadGroupApplicationPolicyRequest, ReadMyGroupMembershipApplicationRequest,
    ReopenGroupMembershipApplicationRequest, ReviewGroupMembershipApplicationRequest,
    ReviewGroupMembershipApplicationResult, SubmitGroupMembershipApplicationIfCurrentRequest,
    SubmitGroupMembershipApplicationRequest, SubmitGroupMembershipApplicationResult,
    UpsertGroupApplicationPolicyIfCurrentRequest, UpsertGroupApplicationPolicyRequest,
    UpsertGroupApplicationPolicyResult,
};
pub use domain::{
    GroupAction, GroupFeatureStatus, GroupJoinPolicy, GroupMembershipEffectiveStatus,
    GroupMembershipEnforcementSourceKind, GroupMembershipEnforcementState, GroupMembershipStatus,
    GroupRole, GroupStatus, GroupVisibility, normalize_feature_key, normalize_group_handle,
};
pub use dto::{
    CreateGroupInput, DeleteGroupTranslationRequest, DeleteGroupTranslationResult,
    GroupAccessDecision, GroupAccessRequest, GroupConnection, GroupDetails, GroupFeatureBinding,
    GroupMembership, GroupMembershipConnection, GroupMembershipEffectiveState,
    GroupMembershipEnforcementSummary, GroupSummary, GroupTranslation,
    GroupTranslationMutationResult, JoinGroupRequest, LeaveGroupRequest,
    ListGroupMembershipsRequest, ListGroupTranslationsRequest, ListGroupsRequest,
    ReadGroupMembershipEnforcementRequest, ReadGroupMembershipRequest, ReadGroupRequest,
    SetGroupFeatureRequest, UpsertGroupTranslationRequest,
};
pub use effective_service::GroupsService;
pub use error::{GroupsError, GroupsResult};
pub use governance::{
    ChangeGroupRoleRequest, GroupGovernanceCommandPort, GroupGovernanceResult,
    GroupGovernanceService, TransferGroupOwnershipRequest,
};
pub use invitations::{
    AcceptGroupInvitationRequest, AcceptGroupInvitationResult, CreateGroupInvitationRequest,
    CreateGroupInvitationResult, GroupInvitation, GroupInvitationCommandPort,
    GroupInvitationConnection, GroupInvitationReadPort, GroupInvitationService,
    GroupInvitationStatus, ListGroupInvitationsRequest, RevokeGroupInvitationRequest,
    RevokeGroupInvitationResult,
};
pub use localization::GroupLocalizationService;
pub use membership_enforcement::GroupMembershipEnforcementService;
pub use membership_enforcement_command::{
    GroupMembershipEnforcementCommandService, GroupMembershipEnforcementMutationResult,
    RevokeGroupMembershipSuspensionRequest, SuspendGroupMembershipRequest,
};
pub use moderation_subject::GroupsModerationSubjectAdapterFactory;
pub use policy_history::{
    GroupApplicationPolicyHistoryReadPort, GroupApplicationPolicyHistoryService,
    GroupApplicationPolicyRevision, GroupApplicationPolicyRevisionConnection,
    ListGroupApplicationPolicyRevisionsRequest,
};
pub use ports::{
    GroupAccessReadPort, GroupCapabilityDescriptor, GroupCommandPort, GroupLocalizationCommandPort,
    GroupLocalizationReadPort, GroupMembershipEnforcementCommandPort,
    GroupMembershipEnforcementReadPort, GroupMembershipReadPort, GroupSummaryReadPort,
    SharedGroupAccessReadPort, SharedGroupCommandPort, SharedGroupLocalizationCommandPort,
    SharedGroupLocalizationReadPort, SharedGroupMembershipEnforcementCommandPort,
    SharedGroupMembershipEnforcementReadPort, SharedGroupMembershipReadPort,
    SharedGroupSummaryReadPort,
};
pub use targeted_invitations::{
    AcceptTargetedGroupInvitationRequest, GroupTargetedInvitationCommandPort,
    GroupTargetedInvitationService,
};

pub use module::GroupsModule;

#[cfg(test)]
mod contract_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_descriptor_fails_closed() {
        let descriptor = GroupCapabilityDescriptor::default();
        assert_eq!(descriptor.contract_version, "groups.access.v1");
        assert_eq!(descriptor.private_content_fallback, "deny");
        assert!(!descriptor.implicit_transport_fallback);
        assert!(
            descriptor
                .ports
                .contains(&"GroupMembershipEnforcementReadPort")
        );
        assert!(
            descriptor
                .ports
                .contains(&"GroupMembershipEnforcementCommandPort")
        );
    }
}
