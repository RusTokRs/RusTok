use async_graphql::{Context, FieldError, InputObject, Object, Result, SimpleObject};
use rustok_api::{
    AuthContext, Permission, TenantContext,
    graphql::{GraphQLError, require_module_enabled},
    has_any_effective_permission,
};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{ForumTopicSplitResult, ForumTopicSplitService, SplitForumTopicRepliesInput};

const MODULE_SLUG: &str = "forum";

#[derive(Default)]
pub(crate) struct ForumTopicSplitMutation;

#[Object]
impl ForumTopicSplitMutation {
    async fn split_forum_topic_replies(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        source_topic_id: Uuid,
        input: SplitForumTopicRepliesGraphqlInput,
    ) -> Result<GqlForumTopicSplit> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?
            .clone();
        let tenant = ctx.data::<TenantContext>()?;

        execute_split_forum_topic_replies(
            db,
            event_bus,
            tenant,
            &auth,
            tenant_id,
            source_topic_id,
            input,
        )
        .await
    }
}

async fn execute_split_forum_topic_replies(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant: &TenantContext,
    auth: &AuthContext,
    requested_tenant_id: Option<Uuid>,
    source_topic_id: Uuid,
    input: SplitForumTopicRepliesGraphqlInput,
) -> Result<GqlForumTopicSplit> {
    require_topic_manage_permission(auth)?;
    let tenant_id = resolve_tenant_scope(tenant, requested_tenant_id)?;

    let result = ForumTopicSplitService::new(db.clone(), event_bus.clone())
        .split_selected_replies(
            tenant_id,
            source_topic_id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
            SplitForumTopicRepliesInput {
                operation_id: input.operation_id,
                target_topic_id: input.target_topic_id,
                reply_ids: input.reply_ids,
                locale: input.locale,
                title: input.title,
                slug: input.slug,
                reason: input.reason,
            },
        )
        .await?;

    Ok(result.into())
}

fn require_topic_manage_permission(auth: &AuthContext) -> Result<()> {
    if !has_any_effective_permission(&auth.permissions, &[Permission::FORUM_TOPICS_MANAGE]) {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Permission denied: forum_topics:manage required",
        ));
    }
    Ok(())
}

fn resolve_tenant_scope(tenant: &TenantContext, requested_tenant_id: Option<Uuid>) -> Result<Uuid> {
    match requested_tenant_id {
        Some(requested_tenant_id) if requested_tenant_id != tenant.id => {
            Err(<FieldError as GraphQLError>::permission_denied(
                "Permission denied: tenant scope mismatch",
            ))
        }
        Some(requested_tenant_id) => Ok(requested_tenant_id),
        None => Ok(tenant.id),
    }
}

#[derive(Clone, Debug, Eq, PartialEq, InputObject)]
pub struct SplitForumTopicRepliesGraphqlInput {
    pub operation_id: Uuid,
    pub target_topic_id: Uuid,
    pub reply_ids: Vec<Uuid>,
    pub locale: String,
    pub title: String,
    pub slug: Option<String>,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct GqlForumTopicSplit {
    pub operation_id: Uuid,
    pub event_id: Uuid,
    pub source_topic_id: Uuid,
    pub target_topic_id: Uuid,
    pub category_id: Uuid,
    pub actor_id: Uuid,
    pub reason: String,
    pub moved_reply_count: i32,
    pub moved_published_reply_count: i32,
    pub source_resulting_published_reply_count: i32,
    pub target_resulting_published_reply_count: i32,
    pub solution_reply_id: Option<Uuid>,
    pub split_at: String,
}

impl From<ForumTopicSplitResult> for GqlForumTopicSplit {
    fn from(value: ForumTopicSplitResult) -> Self {
        Self {
            operation_id: value.operation_id,
            event_id: value.event_id,
            source_topic_id: value.source_topic_id,
            target_topic_id: value.target_topic_id,
            category_id: value.category_id,
            actor_id: value.actor_id,
            reason: value.reason,
            moved_reply_count: value.moved_reply_count,
            moved_published_reply_count: value.moved_published_reply_count,
            source_resulting_published_reply_count: value.source_resulting_published_reply_count,
            target_resulting_published_reply_count: value.target_resulting_published_reply_count,
            solution_reply_id: value.solution_reply_id,
            split_at: value.split_at.to_rfc3339(),
        }
    }
}

#[cfg(test)]
mod tests {
    use rustok_api::{AuthContext, Permission, TenantContext};
    use uuid::Uuid;

    use super::{require_topic_manage_permission, resolve_tenant_scope};

    fn auth_context(permissions: Vec<Permission>) -> AuthContext {
        AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            permissions,
            client_id: None,
            scopes: Vec::new(),
            grant_type: "direct".to_string(),
        }
    }

    fn tenant_context(tenant_id: Uuid) -> TenantContext {
        TenantContext {
            id: tenant_id,
            name: "Topic split GraphQL tenant".to_string(),
            slug: "topic-split-graphql".to_string(),
            domain: None,
            settings: serde_json::json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        }
    }

    fn error_code(error: &async_graphql::Error) -> Option<String> {
        error
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("code"))
            .cloned()
            .and_then(|value| value.into_json().ok())
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
    }

    #[test]
    fn split_transport_requires_topic_manage_permission() {
        let denied =
            require_topic_manage_permission(&auth_context(vec![Permission::FORUM_TOPICS_READ]))
                .expect_err("read-only actor must not split topics");
        assert_eq!(error_code(&denied).as_deref(), Some("PERMISSION_DENIED"));

        require_topic_manage_permission(&auth_context(vec![Permission::FORUM_TOPICS_MANAGE]))
            .expect("manager permission must be accepted");
    }

    #[test]
    fn split_transport_rejects_tenant_override_mismatch() {
        let tenant_id = Uuid::new_v4();
        let tenant = tenant_context(tenant_id);

        assert_eq!(
            resolve_tenant_scope(&tenant, None).expect("routed tenant must resolve"),
            tenant_id
        );
        assert_eq!(
            resolve_tenant_scope(&tenant, Some(tenant_id))
                .expect("matching tenant assertion must resolve"),
            tenant_id
        );

        let denied = resolve_tenant_scope(&tenant, Some(Uuid::new_v4()))
            .expect_err("mismatched tenant assertion must fail closed");
        assert_eq!(error_code(&denied).as_deref(), Some("PERMISSION_DENIED"));
    }
}
