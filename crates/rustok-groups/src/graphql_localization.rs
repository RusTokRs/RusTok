use std::time::Duration;

use async_graphql::{Context, FieldError, InputObject, MergedObject, Object, Result, SimpleObject};
use rustok_api::graphql::GraphQLError;
use rustok_api::request::RequestContext;
use rustok_api::{
    AuthContext, ChannelContext, HostRuntimeContext, PortActor, PortContext, PortError,
    PortErrorKind, TenantContext,
};
use uuid::Uuid;

use crate::graphql::GroupsQuery;
use crate::{
    DeleteGroupTranslationRequest, DeleteGroupTranslationResult, GroupLocalizationCommandPort,
    GroupLocalizationReadPort, GroupLocalizationService, GroupTranslation,
    GroupTranslationMutationResult, ListGroupTranslationsRequest, UpsertGroupTranslationRequest,
};

const PORT_DEADLINE: Duration = Duration::from_secs(5);

#[derive(MergedObject, Default)]
pub struct GroupsQueryRoot(GroupsQuery, GroupsLocalizationQuery);

#[derive(Default)]
pub struct GroupsLocalizationQuery;

#[Object]
impl GroupsLocalizationQuery {
    async fn group_translations(
        &self,
        ctx: &Context<'_>,
        group_id: Uuid,
    ) -> Result<Vec<GroupTranslationGql>> {
        let auth = require_authenticated(ctx)?;
        let service = localization_service(ctx)?;
        GroupLocalizationReadPort::list_group_translations(
            &service,
            port_context(ctx, auth, None)?,
            ListGroupTranslationsRequest { group_id },
        )
        .await
        .map(|items| items.into_iter().map(Into::into).collect())
        .map_err(map_port_error)
    }
}

#[derive(Default)]
pub struct GroupsLocalizationMutation;

#[Object]
impl GroupsLocalizationMutation {
    async fn upsert_group_translation(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        group_id: Uuid,
        input: UpsertGroupTranslationInputGql,
    ) -> Result<GroupTranslationMutationResultGql> {
        let auth = require_authenticated(ctx)?;
        let service = localization_service(ctx)?;
        GroupLocalizationCommandPort::upsert_group_translation(
            &service,
            port_context(ctx, auth, Some(idempotency_key))?,
            UpsertGroupTranslationRequest {
                group_id,
                locale: input.locale,
                title: input.title,
                summary: input.summary,
                body: input.body,
            },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    async fn delete_group_translation(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        group_id: Uuid,
        locale: String,
    ) -> Result<DeleteGroupTranslationResultGql> {
        let auth = require_authenticated(ctx)?;
        let service = localization_service(ctx)?;
        GroupLocalizationCommandPort::delete_group_translation(
            &service,
            port_context(ctx, auth, Some(idempotency_key))?,
            DeleteGroupTranslationRequest { group_id, locale },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }
}

#[derive(InputObject)]
pub struct UpsertGroupTranslationInputGql {
    pub locale: String,
    pub title: String,
    pub summary: Option<String>,
    pub body: Option<String>,
}

#[derive(SimpleObject)]
pub struct GroupTranslationGql {
    pub id: Uuid,
    pub group_id: Uuid,
    pub locale: String,
    pub title: String,
    pub summary: Option<String>,
    pub body: Option<String>,
}

#[derive(SimpleObject)]
pub struct GroupTranslationMutationResultGql {
    pub translation: GroupTranslationGql,
    pub group_version: u64,
    pub created: bool,
}

#[derive(SimpleObject)]
pub struct DeleteGroupTranslationResultGql {
    pub group_id: Uuid,
    pub locale: String,
    pub group_version: u64,
}

impl From<GroupTranslation> for GroupTranslationGql {
    fn from(value: GroupTranslation) -> Self {
        Self {
            id: value.id,
            group_id: value.group_id,
            locale: value.locale,
            title: value.title,
            summary: value.summary,
            body: value.body,
        }
    }
}

impl From<GroupTranslationMutationResult> for GroupTranslationMutationResultGql {
    fn from(value: GroupTranslationMutationResult) -> Self {
        Self {
            translation: value.translation.into(),
            group_version: value.group_version,
            created: value.created,
        }
    }
}

impl From<DeleteGroupTranslationResult> for DeleteGroupTranslationResultGql {
    fn from(value: DeleteGroupTranslationResult) -> Self {
        Self {
            group_id: value.group_id,
            locale: value.locale,
            group_version: value.group_version,
        }
    }
}

fn localization_service(ctx: &Context<'_>) -> Result<GroupLocalizationService> {
    let runtime = ctx.data::<HostRuntimeContext>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error("Groups runtime is not registered")
    })?;
    Ok(GroupLocalizationService::new(runtime.db_clone()))
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
    if auth.tenant_id != tenant.id {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "groups tenant mismatch",
        ));
    }
    if idempotency_key
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err(<FieldError as GraphQLError>::bad_user_input(
            "groups localization idempotency key is required",
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
        format!("graphql-groups-localization-{}", Uuid::new_v4()),
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
                "Groups localization service is temporarily unavailable",
            )
        }
        PortErrorKind::InvariantViolation => <FieldError as GraphQLError>::internal_error(
            "Groups localization operation requires review",
        ),
    }
}
