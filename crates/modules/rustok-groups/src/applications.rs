use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError, normalize_locale_tag};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction,
    DbBackend, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set,
    TransactionTrait,
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
            .order_by_desc(membership_application::Column::Id)
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
        self.upsert_policy_effective_owned(&context, request)
            .await
            .map_err(Into::into)
    }

    async fn submit_group_membership_application(
        &self,
        context: PortContext,
        request: SubmitGroupMembershipApplicationRequest,
    ) -> Result<SubmitGroupMembershipApplicationResult, PortError> {
        request.validate().map_err(PortError::from)?;
        self.submit_application_effective_owned(&context, request)
            .await
            .map_err(Into::into)
    }

    async fn review_group_membership_application(
        &self,
        context: PortContext,
        request: ReviewGroupMembershipApplicationRequest,
    ) -> Result<ReviewGroupMembershipApplicationResult, PortError> {
        request.validate().map_err(PortError::from)?;
        self.review_application_effective_owned(&context, request)
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
        revision: crate::domain::stored_revision(policy.revision)?,
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
        policy_revision: crate::domain::stored_revision(model.policy_revision)?,
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
    let locator = query()
        .one(transaction)
        .await?
        .ok_or_else(|| GroupsError::NotFound)?;
    crate::membership_enforcement_transaction::reserve_group_write_for_update(
        transaction,
        tenant_id,
        locator.group_id,
    )
    .await?;
    match transaction.get_database_backend() {
        DbBackend::Sqlite => query().one(transaction).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().one(transaction).await?,
        _ => unreachable!("unsupported SeaORM database backend"),
    }
    .ok_or(GroupsError::NotFound)
}

fn require_candidate_group(model: &group::Model) -> GroupsResult<()> {
    // Candidate surfaces must not reveal whether a secret group is archived or accepts requests.
    // Visibility is checked before lifecycle, join policy, or candidate-specific state.
    if GroupVisibility::from_str(&model.visibility).map_err(GroupsError::Invariant)?
        == GroupVisibility::Secret
    {
        return Err(GroupsError::NotFound);
    }
    require_active_group(model)
}

fn require_application_group(model: &group::Model) -> GroupsResult<()> {
    require_candidate_group(model)?;
    if GroupJoinPolicy::from_str(&model.join_policy).map_err(GroupsError::Invariant)?
        != GroupJoinPolicy::Request
    {
        return Err(GroupsError::Conflict(
            "group does not accept membership applications".to_string(),
        ));
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
    let group_version = crate::domain::next_revision(group_model.version)? as u64;
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
    let group_version = crate::domain::next_revision(group_model.version)? as u64;
    let member_count = crate::domain::increment_member_count(group_model.member_count)?;
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

#[path = "applications_review.rs"]
mod review;
pub use review::GroupApplicationReviewCommandPort;
#[path = "applications_bulk_review.rs"]
mod bulk_review;
pub use bulk_review::{
    BulkReviewGroupMembershipApplicationItemResult, BulkReviewGroupMembershipApplicationsRequest,
    BulkReviewGroupMembershipApplicationsResult, GroupApplicationBulkReviewCommandPort,
};
#[path = "applications_cas.rs"]
mod cas;
pub use cas::{
    GROUP_APPLICATION_POLICY_CHANGED_CODE, GroupApplicationCasCommandPort,
    GroupApplicationPolicyPrecondition, SubmitGroupMembershipApplicationIfCurrentRequest,
    UpsertGroupApplicationPolicyIfCurrentRequest,
};
use cas::{
    SUBMIT_APPLICATION_IF_CURRENT_COMMAND, UPSERT_POLICY_IF_CURRENT_COMMAND,
    ensure_loaded_policy_precondition, ensure_policy_update_precondition,
    find_candidate_application_for_update, map_application_cas_error,
    normalize_policy_precondition,
};
#[path = "applications_lifecycle.rs"]
mod lifecycle;
use lifecycle::{CANCEL_APPLICATION_COMMAND, REOPEN_APPLICATION_COMMAND};
pub use lifecycle::{
    CancelGroupMembershipApplicationRequest, GroupApplicationLifecycleCommandPort,
    GroupApplicationLifecycleReadPort, GroupApplicationLifecycleResult,
    ReadMyGroupMembershipApplicationRequest, ReopenGroupMembershipApplicationRequest,
};
#[path = "applications_policy_management.rs"]
mod policy_management;
pub use policy_management::{
    GroupApplicationPolicyLocaleCatalog, GroupApplicationPolicyManagementReadPort,
    GroupApplicationPolicyManagementView, ListGroupApplicationPolicyLocalesRequest,
    ReadGroupApplicationPolicyForManagementRequest,
};
#[path = "applications_transactional.rs"]
mod transactional;
#[path = "applications_transactional_cas.rs"]
mod transactional_cas;
#[path = "applications_transactional_cas_bridge.rs"]
mod transactional_cas_bridge;
pub(crate) use transactional_cas_bridge::map_effective_application_cas_error;
#[path = "applications_transactional_lifecycle.rs"]
mod transactional_lifecycle;

pub(crate) enum ApplicationSettlement {
    Invitation,
    Join,
    Leave,
}

/// Called under the group writer lock by non-application lifecycle commands.
/// The current snapshot is preserved; only the pending attempt is completed.
pub(crate) async fn settle_pending_application(
    transaction: &DatabaseTransaction,
    context: &PortContext,
    tenant_id: Uuid,
    group_id: Uuid,
    user_id: Uuid,
    actor_id: Uuid,
    settlement: ApplicationSettlement,
) -> GroupsResult<()> {
    let activated = !matches!(settlement, ApplicationSettlement::Leave);
    let action = match settlement {
        ApplicationSettlement::Invitation => "group.membership_application_completed_by_invitation",
        ApplicationSettlement::Join => "group.membership_application_completed_by_join",
        ApplicationSettlement::Leave => "group.membership_application_cancelled_by_leave",
    };
    let Some(application) =
        find_candidate_application_for_update(transaction, tenant_id, group_id, user_id).await?
    else {
        return Ok(());
    };
    if application.status != GroupApplicationStatus::Pending.as_str() {
        return Ok(());
    }
    let application_id = application.id;
    let now = Utc::now().fixed_offset();
    let mut active: membership_application::ActiveModel = application.into();
    active.status = Set(if activated {
        GroupApplicationStatus::Approved
    } else {
        GroupApplicationStatus::Cancelled
    }
    .as_str()
    .to_string());
    active.reviewed_at = Set(activated.then_some(now));
    active.reviewed_by_user_id = Set(activated.then_some(actor_id));
    active.review_note = Set(None);
    active.updated_at = Set(now);
    active.update(transaction).await?;
    append_audit(
        transaction,
        context,
        tenant_id,
        group_id,
        actor_id,
        action,
        Some(user_id),
        json!({"application_id": application_id, "previous_status": "pending"}),
    )
    .await
}
