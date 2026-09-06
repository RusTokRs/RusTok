use async_graphql::{Context, InputObject, Object, Result, SimpleObject};
use rustok_api::graphql::require_module_enabled;
use rustok_api::{AuthContext, Permission, TenantContext};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{ForkForumReplyBranchInput, ForumTopicForkResult, ForumTopicForkService};

const MODULE_SLUG: &str = "forum";

#[derive(Default)]
pub(crate) struct ForumTopicForkMutation;

#[Object]
impl ForumTopicForkMutation {
    async fn fork_forum_topic_reply_branch(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        source_topic_id: Uuid,
        input: ForkForumTopicReplyBranchGraphqlInput,
    ) -> Result<GqlForumTopicFork> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = super::require_forum_permission(
            ctx,
            &[Permission::FORUM_TOPICS_MANAGE],
            "Permission denied: forum_topics:manage required",
        )?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = super::resolve_tenant_scope(tenant, tenant_id)?;

        execute_fork_forum_topic_reply_branch(
            db,
            event_bus,
            tenant_id,
            auth,
            source_topic_id,
            input,
        )
        .await
    }
}

async fn execute_fork_forum_topic_reply_branch(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    auth: &AuthContext,
    source_topic_id: Uuid,
    input: ForkForumTopicReplyBranchGraphqlInput,
) -> Result<GqlForumTopicFork> {
    let result = ForumTopicForkService::new(db.clone(), event_bus.clone())
        .fork_reply_branch(
            tenant_id,
            source_topic_id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
            ForkForumReplyBranchInput {
                operation_id: input.operation_id,
                target_topic_id: input.target_topic_id,
                root_reply_id: input.root_reply_id,
                locale: input.locale,
                title: input.title,
                slug: input.slug,
                reason: input.reason,
            },
        )
        .await?;

    Ok(result.into())
}

#[derive(Clone, Debug, Eq, PartialEq, InputObject)]
pub struct ForkForumTopicReplyBranchGraphqlInput {
    pub operation_id: Uuid,
    pub target_topic_id: Uuid,
    pub root_reply_id: Uuid,
    pub locale: String,
    pub title: String,
    pub slug: Option<String>,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct GqlForumTopicFork {
    pub operation_id: Uuid,
    pub event_id: Uuid,
    pub source_topic_id: Uuid,
    pub target_topic_id: Uuid,
    pub root_reply_id: Uuid,
    pub category_id: Uuid,
    pub actor_id: Uuid,
    pub reason: String,
    pub copied_reply_count: i32,
    pub copied_published_reply_count: i32,
    pub copied_body_count: i32,
    pub copied_reply_revision_count: i32,
    pub copied_relation_revision_count: i32,
    pub copied_mention_count: i32,
    pub copied_quote_count: i32,
    pub forked_at: String,
}

impl From<ForumTopicForkResult> for GqlForumTopicFork {
    fn from(value: ForumTopicForkResult) -> Self {
        Self {
            operation_id: value.operation_id,
            event_id: value.event_id,
            source_topic_id: value.source_topic_id,
            target_topic_id: value.target_topic_id,
            root_reply_id: value.root_reply_id,
            category_id: value.category_id,
            actor_id: value.actor_id,
            reason: value.reason,
            copied_reply_count: value.copied_reply_count,
            copied_published_reply_count: value.copied_published_reply_count,
            copied_body_count: value.copied_body_count,
            copied_reply_revision_count: value.copied_reply_revision_count,
            copied_relation_revision_count: value.copied_relation_revision_count,
            copied_mention_count: value.copied_mention_count,
            copied_quote_count: value.copied_quote_count,
            forked_at: value.forked_at.to_rfc3339(),
        }
    }
}

#[cfg(test)]
mod tests {
    use rustok_api::{AuthContext, Permission, TenantContext, has_any_effective_permission};
    use uuid::Uuid;

    use crate::graphql::resolve_tenant_scope;

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
            name: "Topic fork GraphQL tenant".to_string(),
            slug: "topic-fork-graphql".to_string(),
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
    fn fork_transport_requires_topic_manage_permission() {
        let read_only = auth_context(vec![Permission::FORUM_TOPICS_READ]);
        assert!(!has_any_effective_permission(
            &read_only.permissions,
            &[Permission::FORUM_TOPICS_MANAGE]
        ));

        let manager = auth_context(vec![Permission::FORUM_TOPICS_MANAGE]);
        assert!(has_any_effective_permission(
            &manager.permissions,
            &[Permission::FORUM_TOPICS_MANAGE]
        ));
    }

    #[test]
    fn fork_transport_rejects_tenant_override_mismatch() {
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
        assert_eq!(error_code(&denied).as_deref(), Some("FORBIDDEN"));
    }
}
