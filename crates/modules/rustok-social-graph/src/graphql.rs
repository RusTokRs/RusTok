use std::time::Duration;

use async_graphql::{Context, FieldError, Object, Result, SimpleObject};
use rustok_api::{
    AuthContext, ChannelContext, PortActor, PortContext, PortError, PortErrorKind, TenantContext,
    graphql::{GraphQLError, require_module_enabled},
    request::RequestContext,
};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    SetSocialRelationCommand, SocialGraphCommandPort, SocialGraphFollowReadPort,
    SocialGraphFollowState, SocialGraphPairRequest, SocialGraphService, SocialRelationKind,
};

const MODULE_SLUG: &str = "social_graph";
const PORT_DEADLINE: Duration = Duration::from_secs(5);

#[derive(Default)]
pub struct SocialGraphQuery;

#[Object]
impl SocialGraphQuery {
    async fn is_following(&self, ctx: &Context<'_>, user_id: Uuid) -> Result<bool> {
        Ok(read_follow_state(ctx, user_id).await?.following)
    }

    async fn follow_state(&self, ctx: &Context<'_>, user_id: Uuid) -> Result<FollowReadStateGql> {
        let state = read_follow_state(ctx, user_id).await?;
        Ok(FollowReadStateGql {
            user_id: state.target_user_id,
            following: state.following,
            revision: state.revision.map(|revision| revision.to_string()),
        })
    }
}

#[derive(Default)]
pub struct SocialGraphMutation;

#[Object]
impl SocialGraphMutation {
    async fn follow_user(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        user_id: Uuid,
        expected_revision: Option<String>,
    ) -> Result<FollowStateGql> {
        set_follow_state(ctx, idempotency_key, user_id, expected_revision, true).await
    }

    async fn unfollow_user(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        user_id: Uuid,
        expected_revision: Option<String>,
    ) -> Result<FollowStateGql> {
        set_follow_state(ctx, idempotency_key, user_id, expected_revision, false).await
    }
}

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct FollowReadStateGql {
    pub user_id: Uuid,
    pub following: bool,
    pub revision: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct FollowStateGql {
    pub user_id: Uuid,
    pub following: bool,
    pub revision: String,
}

async fn read_follow_state(ctx: &Context<'_>, user_id: Uuid) -> Result<SocialGraphFollowState> {
    require_module_enabled(ctx, MODULE_SLUG).await?;
    let auth = require_human_user(ctx)?;
    if auth.user_id == user_id {
        return Ok(SocialGraphFollowState {
            target_user_id: user_id,
            following: false,
            revision: None,
        });
    }

    let service = read_service(ctx)?;
    SocialGraphFollowReadPort::source_follow_state(
        &service,
        port_context(ctx, auth, None)?,
        SocialGraphPairRequest {
            source_user_id: auth.user_id,
            target_user_id: user_id,
        },
    )
    .await
    .map_err(map_port_error)
}

async fn set_follow_state(
    ctx: &Context<'_>,
    idempotency_key: String,
    user_id: Uuid,
    expected_revision: Option<String>,
    active: bool,
) -> Result<FollowStateGql> {
    require_module_enabled(ctx, MODULE_SLUG).await?;
    let auth = require_human_user(ctx)?;
    let service = write_service(ctx)?;
    let relation = SocialGraphCommandPort::set_relation(
        &service,
        port_context(ctx, auth, Some(idempotency_key))?,
        SetSocialRelationCommand {
            source_user_id: auth.user_id,
            target_user_id: user_id,
            relation_kind: SocialRelationKind::Follow,
            active,
            expected_revision: parse_expected_revision(expected_revision)?,
        },
    )
    .await
    .map_err(map_port_error)?;

    Ok(FollowStateGql {
        user_id: relation.target_user_id,
        following: relation.active,
        revision: relation.revision.to_string(),
    })
}

fn read_service(ctx: &Context<'_>) -> Result<SocialGraphService> {
    let db = ctx.data::<DatabaseConnection>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error(
            "Social Graph database context is not registered",
        )
    })?;
    Ok(SocialGraphService::new(db.clone()))
}

fn write_service(ctx: &Context<'_>) -> Result<SocialGraphService> {
    let db = ctx.data::<DatabaseConnection>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error(
            "Social Graph database context is not registered",
        )
    })?;
    let event_bus = ctx.data::<TransactionalEventBus>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error(
            "Social Graph transactional event bus is not registered",
        )
    })?;
    Ok(SocialGraphService::with_event_bus(
        db.clone(),
        event_bus.clone(),
    ))
}

fn require_human_user<'a>(ctx: &'a Context<'a>) -> Result<&'a AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    if auth.is_service_principal() {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "follow operations require human-user credentials",
        ));
    }
    require_tenant(ctx, auth)?;
    Ok(auth)
}

fn require_tenant<'a>(ctx: &'a Context<'a>, auth: &AuthContext) -> Result<&'a TenantContext> {
    let tenant = ctx.data::<TenantContext>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error(
            "Social Graph tenant context is not registered",
        )
    })?;
    if auth.tenant_id != tenant.id {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "follow operations must use the authenticated tenant",
        ));
    }
    Ok(tenant)
}

fn port_context(
    ctx: &Context<'_>,
    auth: &AuthContext,
    idempotency_key: Option<String>,
) -> Result<PortContext> {
    let tenant = require_tenant(ctx, auth)?;
    let locale = ctx
        .data_opt::<RequestContext>()
        .map(|request| request.locale.clone())
        .unwrap_or_else(|| tenant.default_locale.clone());
    let mut context = PortContext::new(
        tenant.id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        locale,
        format!("graphql-social-graph-{}", Uuid::new_v4()),
    )
    .with_deadline(PORT_DEADLINE);

    for permission in &auth.permissions {
        context = context.with_claim(permission.to_string());
    }
    if let Some(channel) = ctx.data_opt::<ChannelContext>() {
        context = context.with_channel(channel.slug.clone());
    }
    if let Some(key) = idempotency_key {
        let key = key.trim();
        if key.is_empty() {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "idempotencyKey must not be empty",
            ));
        }
        context = context.with_idempotency_key(key.to_string());
    }

    Ok(context)
}

fn parse_expected_revision(value: Option<String>) -> Result<Option<i64>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let revision = value.trim().parse::<i64>().map_err(|_| {
        <FieldError as GraphQLError>::bad_user_input(
            "expectedRevision must be a positive 64-bit integer string",
        )
    })?;
    if revision <= 0 {
        return Err(<FieldError as GraphQLError>::bad_user_input(
            "expectedRevision must be a positive 64-bit integer string",
        ));
    }
    Ok(Some(revision))
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
                "Social Graph service is temporarily unavailable",
            )
        }
        PortErrorKind::InvariantViolation => {
            <FieldError as GraphQLError>::internal_error("Social Graph operation requires review")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_expected_revision;

    #[test]
    fn expected_revision_uses_positive_i64_strings() {
        assert_eq!(parse_expected_revision(None).unwrap(), None);
        assert_eq!(
            parse_expected_revision(Some("9223372036854775807".into())).unwrap(),
            Some(i64::MAX)
        );
        assert!(parse_expected_revision(Some("0".into())).is_err());
        assert!(parse_expected_revision(Some("-1".into())).is_err());
        assert!(parse_expected_revision(Some("not-a-revision".into())).is_err());
    }
}
