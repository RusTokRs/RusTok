use std::time::Duration;

use async_graphql::{Context, FieldError, Object, Result, SimpleObject};
use rustok_api::graphql::GraphQLError;
use rustok_api::request::RequestContext;
use rustok_api::{
    AuthContext, ChannelContext, HostRuntimeContext, PortActor, PortContext, PortError,
    PortErrorKind, TenantContext,
};
use uuid::Uuid;

use crate::graphql_applications::{GroupApplicationQuestionGql, GroupApplicationRuleGql};
use crate::{
    GroupApplicationPolicyLocaleCatalog, GroupApplicationPolicyManagementReadPort,
    GroupApplicationPolicyManagementView, GroupApplicationService,
    ListGroupApplicationPolicyLocalesRequest, ReadGroupApplicationPolicyForManagementRequest,
};

const PORT_DEADLINE: Duration = Duration::from_secs(5);

#[derive(Default)]
pub struct GroupsApplicationPolicyManagementQuery;

#[Object]
impl GroupsApplicationPolicyManagementQuery {
    async fn group_application_policy_locale_catalog(
        &self,
        ctx: &Context<'_>,
        group_id: Uuid,
    ) -> Result<GroupApplicationPolicyLocaleCatalogGql> {
        let auth = require_authenticated(ctx)?;
        GroupApplicationPolicyManagementReadPort::list_group_application_policy_locales(
            &application_service(ctx)?,
            port_context(ctx, auth)?,
            ListGroupApplicationPolicyLocalesRequest { group_id },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    async fn group_application_policy_for_management(
        &self,
        ctx: &Context<'_>,
        group_id: Uuid,
        locale: String,
    ) -> Result<GroupApplicationPolicyManagementViewGql> {
        let auth = require_authenticated(ctx)?;
        GroupApplicationPolicyManagementReadPort::read_group_application_policy_for_management(
            &application_service(ctx)?,
            port_context(ctx, auth)?,
            ReadGroupApplicationPolicyForManagementRequest { group_id, locale },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }
}

#[derive(SimpleObject)]
pub struct GroupApplicationPolicyLocaleCatalogGql {
    pub group_id: Uuid,
    pub policy_id: Option<Uuid>,
    pub revision: Option<u64>,
    pub enabled: bool,
    pub locales: Vec<String>,
}

impl From<GroupApplicationPolicyLocaleCatalog> for GroupApplicationPolicyLocaleCatalogGql {
    fn from(value: GroupApplicationPolicyLocaleCatalog) -> Self {
        Self {
            group_id: value.group_id,
            policy_id: value.policy_id,
            revision: value.revision,
            enabled: value.enabled,
            locales: value.locales,
        }
    }
}

#[derive(SimpleObject)]
pub struct GroupApplicationPolicyManagementViewGql {
    pub group_id: Uuid,
    pub policy_id: Option<Uuid>,
    pub revision: Option<u64>,
    pub enabled: bool,
    pub locale: String,
    pub translation_exists: bool,
    pub questions: Vec<GroupApplicationQuestionGql>,
    pub rules: Vec<GroupApplicationRuleGql>,
}

impl From<GroupApplicationPolicyManagementView> for GroupApplicationPolicyManagementViewGql {
    fn from(value: GroupApplicationPolicyManagementView) -> Self {
        Self {
            group_id: value.group_id,
            policy_id: value.policy_id,
            revision: value.revision,
            enabled: value.enabled,
            locale: value.locale,
            translation_exists: value.translation_exists,
            questions: value.questions.into_iter().map(Into::into).collect(),
            rules: value.rules.into_iter().map(Into::into).collect(),
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
        format!("graphql-groups-policy-management-{}", Uuid::new_v4()),
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
        PortErrorKind::Forbidden => <FieldError as GraphQLError>::permission_denied(&error.message),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => {
            <FieldError as GraphQLError>::internal_error(
                "Groups application policy management is temporarily unavailable",
            )
        }
        PortErrorKind::InvariantViolation => <FieldError as GraphQLError>::internal_error(
            "Groups application policy management requires review",
        ),
    }
}
