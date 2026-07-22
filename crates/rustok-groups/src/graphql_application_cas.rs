use std::collections::BTreeMap;
use std::time::Duration;

use async_graphql::{Context, FieldError, InputObject, MergedObject, Object, Result};
use rustok_api::graphql::GraphQLError;
use rustok_api::request::RequestContext;
use rustok_api::{
    AuthContext, ChannelContext, HostRuntimeContext, PortActor, PortContext, PortError,
    PortErrorKind, TenantContext,
};
use uuid::Uuid;

use crate::graphql_applications::{
    SubmitGroupMembershipApplicationInputGql, SubmitGroupMembershipApplicationResultGql,
    UpsertGroupApplicationPolicyInputGql, UpsertGroupApplicationPolicyResultGql,
};
use crate::graphql_policy_history::{
    GroupsMutationRoot as GroupsBaseMutationRoot, GroupsQueryRoot as GroupsBaseQueryRoot,
};
use crate::{
    GroupApplicationCasCommandPort, GroupApplicationPolicyPrecondition,
    GroupApplicationService, SubmitGroupMembershipApplicationIfCurrentRequest,
    SubmitGroupMembershipApplicationRequest, UpsertGroupApplicationPolicyIfCurrentRequest,
    UpsertGroupApplicationPolicyRequest, GROUP_APPLICATION_POLICY_CHANGED_CODE,
};

const PORT_DEADLINE: Duration = Duration::from_secs(5);

pub type GroupsQueryRoot = GroupsBaseQueryRoot;

#[derive(MergedObject, Default)]
pub struct GroupsMutationRoot(GroupsBaseMutationRoot, GroupsApplicationCasMutation);

#[derive(Default)]
pub struct GroupsApplicationCasMutation;

#[Object]
impl GroupsApplicationCasMutation {
    async fn upsert_group_application_policy_if_current(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        group_id: Uuid,
        expected_policy: Option<GroupApplicationPolicyPreconditionInputGql>,
        input: UpsertGroupApplicationPolicyInputGql,
    ) -> Result<UpsertGroupApplicationPolicyResultGql> {
        let auth = require_authenticated(ctx)?;
        let questions = input
            .questions
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<_>>>()?;
        let rules = input.rules.into_iter().map(Into::into).collect();
        GroupApplicationCasCommandPort::upsert_group_application_policy_if_current(
            &application_service(ctx)?,
            port_context(ctx, auth, Some(idempotency_key))?,
            UpsertGroupApplicationPolicyIfCurrentRequest {
                expected_policy: expected_policy.map(Into::into),
                policy: UpsertGroupApplicationPolicyRequest {
                    group_id,
                    locale: input.locale,
                    enabled: input.enabled,
                    questions,
                    rules,
                },
            },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    async fn submit_group_membership_application_if_current(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        group_id: Uuid,
        expected_policy: GroupApplicationPolicyPreconditionInputGql,
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
        GroupApplicationCasCommandPort::submit_group_membership_application_if_current(
            &application_service(ctx)?,
            port_context(ctx, auth, Some(idempotency_key))?,
            SubmitGroupMembershipApplicationIfCurrentRequest {
                expected_policy: expected_policy.into(),
                submission: SubmitGroupMembershipApplicationRequest {
                    group_id,
                    answers,
                    acknowledged_rule_keys: input.acknowledged_rule_keys,
                },
            },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }
}

#[derive(InputObject)]
pub struct GroupApplicationPolicyPreconditionInputGql {
    pub policy_id: Uuid,
    pub revision: u64,
    pub locale: String,
}

impl From<GroupApplicationPolicyPreconditionInputGql> for GroupApplicationPolicyPrecondition {
    fn from(value: GroupApplicationPolicyPreconditionInputGql) -> Self {
        Self {
            policy_id: value.policy_id,
            revision: value.revision,
            locale: value.locale,
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
        format!("graphql-groups-application-cas-{}", Uuid::new_v4()),
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
    if error.code == GROUP_APPLICATION_POLICY_CHANGED_CODE {
        return <FieldError as GraphQLError>::bad_user_input(&format!(
            "{}: {}",
            error.code, error.message
        ));
    }
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
                "Groups membership application service is temporarily unavailable",
            )
        }
        PortErrorKind::InvariantViolation => {
            <FieldError as GraphQLError>::internal_error(
                "Groups membership application operation requires review",
            )
        }
    }
}
