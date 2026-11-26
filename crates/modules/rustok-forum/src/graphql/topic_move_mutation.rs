use async_graphql::{Context, InputObject, Object, Result, SimpleObject};
use rustok_api::graphql::require_module_enabled;
use rustok_api::{AuthContext, Permission, TenantContext};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{ForumTopicMoveResult, ForumTopicMoveService, MoveForumTopicInput};

const MODULE_SLUG: &str = "forum";

#[derive(Default)]
pub(crate) struct ForumTopicMoveMutation;

#[Object]
impl ForumTopicMoveMutation {
    async fn move_forum_topic(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        topic_id: Uuid,
        input: MoveForumTopicGraphqlInput,
    ) -> Result<GqlForumTopicMove> {
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

        execute_move_forum_topic(db, event_bus, tenant_id, auth, topic_id, input).await
    }
}

async fn execute_move_forum_topic(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    auth: &AuthContext,
    topic_id: Uuid,
    input: MoveForumTopicGraphqlInput,
) -> Result<GqlForumTopicMove> {
    let result = ForumTopicMoveService::new(db.clone(), event_bus.clone())
        .move_topic(
            tenant_id,
            topic_id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
            MoveForumTopicInput {
                operation_id: input.operation_id,
                target_category_id: input.target_category_id,
                reason: input.reason,
            },
        )
        .await?;

    Ok(result.into())
}

#[derive(Clone, Debug, Eq, PartialEq, InputObject)]
pub struct MoveForumTopicGraphqlInput {
    pub operation_id: Uuid,
    pub target_category_id: Uuid,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct GqlForumTopicMove {
    pub operation_id: Uuid,
    pub event_id: Uuid,
    pub topic_id: Uuid,
    pub source_category_id: Uuid,
    pub target_category_id: Uuid,
    pub actor_id: Uuid,
    pub reason: String,
    pub published_reply_count: i32,
    pub moved_at: String,
}

impl From<ForumTopicMoveResult> for GqlForumTopicMove {
    fn from(value: ForumTopicMoveResult) -> Self {
        Self {
            operation_id: value.operation_id,
            event_id: value.event_id,
            topic_id: value.topic_id,
            source_category_id: value.source_category_id,
            target_category_id: value.target_category_id,
            actor_id: value.actor_id,
            reason: value.reason,
            published_reply_count: value.published_reply_count,
            moved_at: value.moved_at.to_rfc3339(),
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
            name: "Topic move GraphQL tenant".to_string(),
            slug: "topic-move-graphql".to_string(),
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
    fn move_transport_requires_topic_manage_permission() {
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
    fn move_transport_rejects_tenant_override_mismatch() {
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
