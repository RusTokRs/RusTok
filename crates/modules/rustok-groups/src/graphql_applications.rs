use std::collections::BTreeMap;
use std::time::Duration;

use async_graphql::{
    Context, Enum, FieldError, InputObject, MergedObject, Object, Result, SimpleObject,
};
use chrono::{DateTime, Utc};
use rustok_api::graphql::GraphQLError;
use rustok_api::request::RequestContext;
use rustok_api::{
    AuthContext, ChannelContext, HostRuntimeContext, PortActor, PortContext, PortError,
    PortErrorKind, TenantContext,
};
use uuid::Uuid;

use crate::graphql::GroupMembershipGql;
use crate::graphql_invitations::{
    GroupsMutationRoot as GroupsBaseMutationRoot, GroupsQueryRoot as GroupsBaseQueryRoot,
};
use crate::{
    GroupApplicationCommandPort, GroupApplicationPolicy, GroupApplicationQuestion,
    GroupApplicationReadPort, GroupApplicationReviewDecision, GroupApplicationRule,
    GroupApplicationService, GroupApplicationStatus, GroupMembershipApplication,
    GroupMembershipApplicationConnection, ListGroupMembershipApplicationsRequest,
    ReadGroupApplicationPolicyRequest, ReviewGroupMembershipApplicationRequest,
    ReviewGroupMembershipApplicationResult, SubmitGroupMembershipApplicationRequest,
    SubmitGroupMembershipApplicationResult, UpsertGroupApplicationPolicyRequest,
    UpsertGroupApplicationPolicyResult,
};

const PORT_DEADLINE: Duration = Duration::from_secs(5);

#[derive(MergedObject, Default)]
pub struct GroupsQueryRoot(GroupsBaseQueryRoot, GroupsApplicationsQuery);

#[derive(MergedObject, Default)]
pub struct GroupsMutationRoot(GroupsBaseMutationRoot, GroupsApplicationsMutation);

#[derive(Default)]
pub struct GroupsApplicationsQuery;

#[Object]
impl GroupsApplicationsQuery {
    async fn group_application_policy(
        &self,
        ctx: &Context<'_>,
        group_id: Uuid,
    ) -> Result<GroupApplicationPolicyGql> {
        let auth = require_authenticated(ctx)?;
        GroupApplicationReadPort::read_group_application_policy(
            &application_service(ctx)?,
            port_context(ctx, auth, None)?,
            ReadGroupApplicationPolicyRequest { group_id },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    async fn group_membership_applications(
        &self,
        ctx: &Context<'_>,
        group_id: Uuid,
        status: Option<GroupApplicationStatusGql>,
        page: Option<i32>,
        per_page: Option<i32>,
    ) -> Result<GroupMembershipApplicationConnectionGql> {
        let auth = require_authenticated(ctx)?;
        GroupApplicationReadPort::list_group_membership_applications(
            &application_service(ctx)?,
            port_context(ctx, auth, None)?,
            ListGroupMembershipApplicationsRequest {
                group_id,
                status: status.map(Into::into),
                page: page.unwrap_or(1).max(1) as u64,
                per_page: per_page.unwrap_or(24).clamp(1, 100) as u64,
            },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }
}

#[derive(Default)]
pub struct GroupsApplicationsMutation;

#[Object]
impl GroupsApplicationsMutation {
    async fn upsert_group_application_policy(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        group_id: Uuid,
        input: UpsertGroupApplicationPolicyInputGql,
    ) -> Result<UpsertGroupApplicationPolicyResultGql> {
        let auth = require_authenticated(ctx)?;
        let questions = input
            .questions
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<_>>>()?;
        let rules = input.rules.into_iter().map(Into::into).collect();
        GroupApplicationCommandPort::upsert_group_application_policy(
            &application_service(ctx)?,
            port_context(ctx, auth, Some(idempotency_key))?,
            UpsertGroupApplicationPolicyRequest {
                group_id,
                locale: input.locale,
                enabled: input.enabled,
                questions,
                rules,
            },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    async fn submit_group_membership_application(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        group_id: Uuid,
        input: SubmitGroupMembershipApplicationInputGql,
    ) -> Result<SubmitGroupMembershipApplicationResultGql> {
        let auth = require_authenticated(ctx)?;
        let mut answers = BTreeMap::new();
        for answer in input.answers {
            if answers.insert(answer.key, answer.value).is_some() {
                return Err(<FieldError as GraphQLError>::bad_user_input(
                    "membership application answer keys must be unique",
                ));
            }
        }
        GroupApplicationCommandPort::submit_group_membership_application(
            &application_service(ctx)?,
            port_context(ctx, auth, Some(idempotency_key))?,
            SubmitGroupMembershipApplicationRequest {
                group_id,
                answers,
                acknowledged_rule_keys: input.acknowledged_rule_keys,
            },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    async fn review_group_membership_application(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        application_id: Uuid,
        decision: GroupApplicationReviewDecisionGql,
        note: Option<String>,
    ) -> Result<ReviewGroupMembershipApplicationResultGql> {
        let auth = require_authenticated(ctx)?;
        GroupApplicationCommandPort::review_group_membership_application(
            &application_service(ctx)?,
            port_context(ctx, auth, Some(idempotency_key))?,
            ReviewGroupMembershipApplicationRequest {
                application_id,
                decision: decision.into(),
                note,
            },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }
}

#[derive(InputObject)]
pub struct UpsertGroupApplicationPolicyInputGql {
    pub locale: String,
    pub enabled: bool,
    pub questions: Vec<GroupApplicationQuestionInputGql>,
    pub rules: Vec<GroupApplicationRuleInputGql>,
}

#[derive(InputObject)]
pub struct GroupApplicationQuestionInputGql {
    pub key: String,
    pub prompt: String,
    pub help_text: Option<String>,
    pub required: bool,
    pub max_answer_chars: i32,
}

impl TryFrom<GroupApplicationQuestionInputGql> for GroupApplicationQuestion {
    type Error = FieldError;

    fn try_from(value: GroupApplicationQuestionInputGql) -> Result<Self, Self::Error> {
        let max_answer_chars = u32::try_from(value.max_answer_chars).map_err(|_| {
            <FieldError as GraphQLError>::bad_user_input(
                "question maxAnswerChars must be a positive integer",
            )
        })?;
        Ok(Self {
            key: value.key,
            prompt: value.prompt,
            help_text: value.help_text,
            required: value.required,
            max_answer_chars,
        })
    }
}

#[derive(InputObject)]
pub struct GroupApplicationRuleInputGql {
    pub key: String,
    pub title: String,
    pub body: String,
    pub required: bool,
}

impl From<GroupApplicationRuleInputGql> for GroupApplicationRule {
    fn from(value: GroupApplicationRuleInputGql) -> Self {
        Self {
            key: value.key,
            title: value.title,
            body: value.body,
            required: value.required,
        }
    }
}

#[derive(InputObject)]
pub struct SubmitGroupMembershipApplicationInputGql {
    pub answers: Vec<GroupApplicationAnswerInputGql>,
    pub acknowledged_rule_keys: Vec<String>,
}

#[derive(InputObject)]
pub struct GroupApplicationAnswerInputGql {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum GroupApplicationStatusGql {
    Pending,
    Approved,
    Rejected,
    Cancelled,
}

impl From<GroupApplicationStatusGql> for GroupApplicationStatus {
    fn from(value: GroupApplicationStatusGql) -> Self {
        match value {
            GroupApplicationStatusGql::Pending => Self::Pending,
            GroupApplicationStatusGql::Approved => Self::Approved,
            GroupApplicationStatusGql::Rejected => Self::Rejected,
            GroupApplicationStatusGql::Cancelled => Self::Cancelled,
        }
    }
}

impl From<GroupApplicationStatus> for GroupApplicationStatusGql {
    fn from(value: GroupApplicationStatus) -> Self {
        match value {
            GroupApplicationStatus::Pending => Self::Pending,
            GroupApplicationStatus::Approved => Self::Approved,
            GroupApplicationStatus::Rejected => Self::Rejected,
            GroupApplicationStatus::Cancelled => Self::Cancelled,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
pub enum GroupApplicationReviewDecisionGql {
    Approve,
    Reject,
}

impl From<GroupApplicationReviewDecisionGql> for GroupApplicationReviewDecision {
    fn from(value: GroupApplicationReviewDecisionGql) -> Self {
        match value {
            GroupApplicationReviewDecisionGql::Approve => Self::Approve,
            GroupApplicationReviewDecisionGql::Reject => Self::Reject,
        }
    }
}

#[derive(SimpleObject)]
pub struct GroupApplicationQuestionGql {
    pub key: String,
    pub prompt: String,
    pub help_text: Option<String>,
    pub required: bool,
    pub max_answer_chars: u32,
}

impl From<GroupApplicationQuestion> for GroupApplicationQuestionGql {
    fn from(value: GroupApplicationQuestion) -> Self {
        Self {
            key: value.key,
            prompt: value.prompt,
            help_text: value.help_text,
            required: value.required,
            max_answer_chars: value.max_answer_chars,
        }
    }
}

#[derive(SimpleObject)]
pub struct GroupApplicationRuleGql {
    pub key: String,
    pub title: String,
    pub body: String,
    pub required: bool,
}

impl From<GroupApplicationRule> for GroupApplicationRuleGql {
    fn from(value: GroupApplicationRule) -> Self {
        Self {
            key: value.key,
            title: value.title,
            body: value.body,
            required: value.required,
        }
    }
}

#[derive(SimpleObject)]
pub struct GroupApplicationPolicyGql {
    pub id: Uuid,
    pub group_id: Uuid,
    pub revision: u64,
    pub enabled: bool,
    pub locale: String,
    pub questions: Vec<GroupApplicationQuestionGql>,
    pub rules: Vec<GroupApplicationRuleGql>,
}

impl From<GroupApplicationPolicy> for GroupApplicationPolicyGql {
    fn from(value: GroupApplicationPolicy) -> Self {
        Self {
            id: value.id,
            group_id: value.group_id,
            revision: value.revision,
            enabled: value.enabled,
            locale: value.locale,
            questions: value.questions.into_iter().map(Into::into).collect(),
            rules: value.rules.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(SimpleObject)]
pub struct GroupApplicationAnswerGql {
    pub key: String,
    pub value: String,
}

#[derive(SimpleObject)]
pub struct GroupMembershipApplicationGql {
    pub id: Uuid,
    pub group_id: Uuid,
    pub user_id: Uuid,
    pub policy_id: Uuid,
    pub policy_revision: u64,
    pub policy_locale: String,
    pub questions: Vec<GroupApplicationQuestionGql>,
    pub rules: Vec<GroupApplicationRuleGql>,
    pub answers: Vec<GroupApplicationAnswerGql>,
    pub acknowledged_rule_keys: Vec<String>,
    pub status: GroupApplicationStatusGql,
    pub submitted_at: DateTime<Utc>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub reviewed_by_user_id: Option<Uuid>,
    pub review_note: Option<String>,
}

impl From<GroupMembershipApplication> for GroupMembershipApplicationGql {
    fn from(value: GroupMembershipApplication) -> Self {
        Self {
            id: value.id,
            group_id: value.group_id,
            user_id: value.user_id,
            policy_id: value.policy_id,
            policy_revision: value.policy_revision,
            policy_locale: value.policy_locale,
            questions: value.questions.into_iter().map(Into::into).collect(),
            rules: value.rules.into_iter().map(Into::into).collect(),
            answers: value
                .answers
                .into_iter()
                .map(|(key, value)| GroupApplicationAnswerGql { key, value })
                .collect(),
            acknowledged_rule_keys: value.acknowledged_rule_keys,
            status: value.status.into(),
            submitted_at: value.submitted_at,
            reviewed_at: value.reviewed_at,
            reviewed_by_user_id: value.reviewed_by_user_id,
            review_note: value.review_note,
        }
    }
}

#[derive(SimpleObject)]
pub struct GroupMembershipApplicationConnectionGql {
    pub items: Vec<GroupMembershipApplicationGql>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

impl From<GroupMembershipApplicationConnection> for GroupMembershipApplicationConnectionGql {
    fn from(value: GroupMembershipApplicationConnection) -> Self {
        Self {
            items: value.items.into_iter().map(Into::into).collect(),
            total: value.total,
            page: value.page,
            per_page: value.per_page,
        }
    }
}

#[derive(SimpleObject)]
pub struct UpsertGroupApplicationPolicyResultGql {
    pub policy: GroupApplicationPolicyGql,
    pub group_version: u64,
    pub created: bool,
    pub replayed: bool,
}

impl From<UpsertGroupApplicationPolicyResult> for UpsertGroupApplicationPolicyResultGql {
    fn from(value: UpsertGroupApplicationPolicyResult) -> Self {
        Self {
            policy: value.policy.into(),
            group_version: value.group_version,
            created: value.created,
            replayed: value.replayed,
        }
    }
}

#[derive(SimpleObject)]
pub struct SubmitGroupMembershipApplicationResultGql {
    pub application: GroupMembershipApplicationGql,
    pub membership: GroupMembershipGql,
    pub group_version: u64,
    pub replayed: bool,
}

impl From<SubmitGroupMembershipApplicationResult> for SubmitGroupMembershipApplicationResultGql {
    fn from(value: SubmitGroupMembershipApplicationResult) -> Self {
        Self {
            application: value.application.into(),
            membership: value.membership.into(),
            group_version: value.group_version,
            replayed: value.replayed,
        }
    }
}

#[derive(SimpleObject)]
pub struct ReviewGroupMembershipApplicationResultGql {
    pub application: GroupMembershipApplicationGql,
    pub membership: GroupMembershipGql,
    pub group_version: u64,
    pub replayed: bool,
}

impl From<ReviewGroupMembershipApplicationResult> for ReviewGroupMembershipApplicationResultGql {
    fn from(value: ReviewGroupMembershipApplicationResult) -> Self {
        Self {
            application: value.application.into(),
            membership: value.membership.into(),
            group_version: value.group_version,
            replayed: value.replayed,
        }
    }
}

fn application_service(ctx: &Context<'_>) -> Result<GroupApplicationService> {
    let runtime = ctx.data::<HostRuntimeContext>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error("Groups runtime is not registered")
    })?;
    Ok(GroupApplicationService::new(runtime.db_clone()))
}

fn require_authenticated<'a>(ctx: &'a Context<'a>) -> Result<&'a AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    let tenant = ctx.data::<TenantContext>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error("Groups tenant context is not registered")
    })?;
    if auth.tenant_id != tenant.id {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "groups tenant mismatch",
        ));
    }
    Ok(auth)
}

fn port_context(
    ctx: &Context<'_>,
    auth: &AuthContext,
    idempotency_key: Option<String>,
) -> Result<PortContext> {
    let tenant = ctx.data::<TenantContext>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error("Groups tenant context is not registered")
    })?;
    if idempotency_key
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err(<FieldError as GraphQLError>::bad_user_input(
            "groups application idempotency key is required",
        ));
    }
    let locale = ctx
        .data::<RequestContext>()
        .map(|request| request.locale.clone())
        .or_else(|_| {
            ctx.data::<rustok_core::Locale>()
                .map(|locale| locale.as_str().to_string())
        })
        .unwrap_or_else(|_| tenant.default_locale.clone());
    let mut context = PortContext::new(
        tenant.id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        locale,
        format!("graphql-groups-applications-{}", Uuid::new_v4()),
    )
    .with_deadline(PORT_DEADLINE);
    if let Some(idempotency_key) = idempotency_key {
        context = context.with_idempotency_key(idempotency_key);
    }
    for permission in &auth.permissions {
        context = context.with_claim(permission.to_string());
    }
    if let Ok(channel) = ctx.data::<ChannelContext>() {
        context = context.with_channel(channel.slug.clone());
    }
    Ok(context)
}

fn map_port_error(error: PortError) -> FieldError {
    match error.kind {
        PortErrorKind::Validation | PortErrorKind::Conflict => {
            <FieldError as GraphQLError>::bad_user_input(&error.message)
        }
        PortErrorKind::NotFound => <FieldError as GraphQLError>::not_found(&error.message),
        PortErrorKind::Forbidden => <FieldError as GraphQLError>::permission_denied(&error.message),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => {
            <FieldError as GraphQLError>::internal_error(
                "Groups membership application service is temporarily unavailable",
            )
        }
        PortErrorKind::InvariantViolation => <FieldError as GraphQLError>::internal_error(
            "Groups membership application operation requires review",
        ),
    }
}
