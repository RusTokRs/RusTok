use std::time::Duration;

use async_graphql::{Context, FieldError, MergedObject, Object, Result, SimpleObject};
use chrono::{DateTime, Utc};
use rustok_api::graphql::GraphQLError;
use rustok_api::request::RequestContext;
use rustok_api::{
    AuthContext, ChannelContext, HostRuntimeContext, PortActor, PortContext, PortError,
    PortErrorKind, TenantContext,
};
use uuid::Uuid;

use crate::graphql_applications::{
    GroupApplicationQuestionGql, GroupApplicationRuleGql,
    GroupsMutationRoot as GroupsBaseMutationRoot, GroupsQueryRoot as GroupsBaseQueryRoot,
};
use crate::{
    GroupApplicationPolicyHistoryReadPort, GroupApplicationPolicyHistoryService,
    GroupApplicationPolicyRevision, GroupApplicationPolicyRevisionConnection,
    ListGroupApplicationPolicyRevisionsRequest,
};

const PORT_DEADLINE: Duration = Duration::from_secs(5);

#[derive(MergedObject, Default)]
pub struct GroupsQueryRoot(GroupsBaseQueryRoot, GroupsPolicyHistoryQuery);

pub type GroupsMutationRoot = GroupsBaseMutationRoot;

#[derive(Default)]
pub struct GroupsPolicyHistoryQuery;

#[Object]
impl GroupsPolicyHistoryQuery {
    async fn group_application_policy_revisions(
        &self,
        ctx: &Context<'_>,
        group_id: Uuid,
        page: Option<i32>,
        per_page: Option<i32>,
    ) -> Result<GroupApplicationPolicyRevisionConnectionGql> {
        let auth = require_authenticated(ctx)?;
        GroupApplicationPolicyHistoryReadPort::list_group_application_policy_revisions(
            &policy_history_service(ctx)?,
            port_context(ctx, auth)?,
            ListGroupApplicationPolicyRevisionsRequest {
                group_id,
                page: page.unwrap_or(1).max(1) as u64,
                per_page: per_page.unwrap_or(20).clamp(1, 100) as u64,
            },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }
}

#[derive(SimpleObject)]
pub struct GroupApplicationPolicyRevisionGql {
    pub group_id: Uuid,
    pub policy_id: Uuid,
    pub revision: u64,
    pub locale: String,
    pub enabled: bool,
    pub questions: Vec<GroupApplicationQuestionGql>,
    pub rules: Vec<GroupApplicationRuleGql>,
    pub created_by_user_id: Uuid,
    pub created_at: DateTime<Utc>,
}

impl From<GroupApplicationPolicyRevision> for GroupApplicationPolicyRevisionGql {
    fn from(value: GroupApplicationPolicyRevision) -> Self {
        Self {
            group_id: value.group_id,
            policy_id: value.policy_id,
            revision: value.revision,
            locale: value.locale,
            enabled: value.enabled,
            questions: value.questions.into_iter().map(Into::into).collect(),
            rules: value.rules.into_iter().map(Into::into).collect(),
            created_by_user_id: value.created_by_user_id,
            created_at: value.created_at,
        }
    }
}

#[derive(SimpleObject)]
pub struct GroupApplicationPolicyRevisionConnectionGql {
    pub items: Vec<GroupApplicationPolicyRevisionGql>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

impl From<GroupApplicationPolicyRevisionConnection>
    for GroupApplicationPolicyRevisionConnectionGql
{
    fn from(value: GroupApplicationPolicyRevisionConnection) -> Self {
        Self {
            items: value.items.into_iter().map(Into::into).collect(),
            total: value.total,
            page: value.page,
            per_page: value.per_page,
        }
    }
}

fn policy_history_service(ctx: &Context<'_>) -> Result<GroupApplicationPolicyHistoryService> {
    let runtime = ctx.data::<HostRuntimeContext>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error("Groups runtime is not registered")
    })?;
    Ok(GroupApplicationPolicyHistoryService::new(runtime.db_clone()))
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

fn port_context(ctx: &Context<'_>, auth: &AuthContext) -> Result<PortContext> {
    let tenant = ctx.data::<TenantContext>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error("Groups tenant context is not registered")
    })?;
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
        format!("graphql-groups-policy-history-{}", Uuid::new_v4()),
    )
    .with_deadline(PORT_DEADLINE);
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
        PortErrorKind::Forbidden => {
            <FieldError as GraphQLError>::permission_denied(&error.message)
        }
        PortErrorKind::Unavailable | PortErrorKind::Timeout => {
            <FieldError as GraphQLError>::internal_error(
                "Groups membership policy history is temporarily unavailable",
            )
        }
        PortErrorKind::InvariantViolation => {
            <FieldError as GraphQLError>::internal_error(
                "Groups membership policy history requires review",
            )
        }
    }
}
