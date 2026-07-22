use async_graphql::{Context, Enum, FieldError, InputObject, Object, Result, SimpleObject};
use rustok_api::graphql::{GraphQLError, require_module_enabled};
use rustok_api::{
    AuthContext, Permission, TenantContext, has_any_effective_permission,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    ForumQuoteCommandService, ForumQuoteReferenceInput, ForumQuoteTargetKindInput,
    ForumRelationSnapshotResponse, SetForumQuotesInput,
};

const MODULE_SLUG: &str = "forum";

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum GqlForumQuoteTargetKind {
    Topic,
    Reply,
}

#[derive(InputObject)]
pub struct GqlForumQuoteReferenceInput {
    pub target_kind: GqlForumQuoteTargetKind,
    pub target_id: Uuid,
    pub revision_id: i64,
}

#[derive(InputObject)]
pub struct SetForumQuoteRelationsInput {
    pub locale: String,
    #[graphql(default)]
    pub quotes: Vec<GqlForumQuoteReferenceInput>,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlForumRelationQuote {
    pub target_kind: String,
    pub target_id: Uuid,
    pub revision_id: i64,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlForumRelationSnapshot {
    pub revision_id: i64,
    pub target_kind: String,
    pub target_id: Uuid,
    pub locale: String,
    pub user_ids: Vec<Uuid>,
    pub audiences: Vec<String>,
    pub quotes: Vec<GqlForumRelationQuote>,
    pub created_at: String,
}

#[derive(Default)]
pub struct ForumQuoteCommandMutation;

#[Object]
impl ForumQuoteCommandMutation {
    async fn set_forum_topic_quotes(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        topic_id: Uuid,
        input: SetForumQuoteRelationsInput,
    ) -> Result<GqlForumRelationSnapshot> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_permission(
            ctx,
            Permission::FORUM_TOPICS_UPDATE,
            "Permission denied: forum_topics:update required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let response = ForumQuoteCommandService::new(db.clone())
            .set_topic_quotes(
                tenant_id,
                topic_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
                map_input(input),
            )
            .await?;
        Ok(map_response(response))
    }

    async fn set_forum_reply_quotes(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        reply_id: Uuid,
        input: SetForumQuoteRelationsInput,
    ) -> Result<GqlForumRelationSnapshot> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let auth = require_permission(
            ctx,
            Permission::FORUM_REPLIES_UPDATE,
            "Permission denied: forum_replies:update required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let response = ForumQuoteCommandService::new(db.clone())
            .set_reply_quotes(
                tenant_id,
                reply_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
                map_input(input),
            )
            .await?;
        Ok(map_response(response))
    }
}

fn require_permission(
    ctx: &Context<'_>,
    permission: Permission,
    message: &str,
) -> Result<AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?
        .clone();
    if !has_any_effective_permission(&auth.permissions, &[permission]) {
        return Err(<FieldError as GraphQLError>::permission_denied(message));
    }
    Ok(auth)
}

fn resolve_tenant_scope(tenant: &TenantContext, requested_tenant_id: Option<Uuid>) -> Result<Uuid> {
    match requested_tenant_id {
        Some(requested) if requested != tenant.id => Err(
            <FieldError as GraphQLError>::permission_denied(
                "Permission denied: tenant scope mismatch",
            ),
        ),
        Some(requested) => Ok(requested),
        None => Ok(tenant.id),
    }
}

fn map_input(input: SetForumQuoteRelationsInput) -> SetForumQuotesInput {
    SetForumQuotesInput {
        locale: input.locale,
        quotes: input
            .quotes
            .into_iter()
            .map(|quote| ForumQuoteReferenceInput {
                target_kind: match quote.target_kind {
                    GqlForumQuoteTargetKind::Topic => ForumQuoteTargetKindInput::Topic,
                    GqlForumQuoteTargetKind::Reply => ForumQuoteTargetKindInput::Reply,
                },
                target_id: quote.target_id,
                revision_id: quote.revision_id,
            })
            .collect(),
    }
}

fn map_response(response: ForumRelationSnapshotResponse) -> GqlForumRelationSnapshot {
    GqlForumRelationSnapshot {
        revision_id: response.revision_id,
        target_kind: response.target_kind,
        target_id: response.target_id,
        locale: response.locale,
        user_ids: response.user_ids,
        audiences: response.audiences,
        quotes: response
            .quotes
            .into_iter()
            .map(|quote| GqlForumRelationQuote {
                target_kind: quote.target_kind,
                target_id: quote.target_id,
                revision_id: quote.revision_id,
            })
            .collect(),
        created_at: response.created_at,
    }
}
