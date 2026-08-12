use async_graphql::{Context, FieldError, InputObject, Object, Result, SimpleObject};
use rustok_api::{
    AuthContext, Permission, TenantContext,
    graphql::{GraphQLError, require_module_enabled},
    has_any_effective_permission,
};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    ForumTopicRouteDescriptor, ForumTopicSlugRenameResult, RenameForumTopicSlugInput, TopicService,
};

const MODULE_SLUG: &str = "forum";

#[derive(Default)]
pub(crate) struct ForumTopicSlugRenameMutation;

#[Object]
impl ForumTopicSlugRenameMutation {
    async fn rename_forum_topic_slug(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        topic_id: Uuid,
        input: RenameForumTopicSlugGraphqlInput,
    ) -> Result<GqlForumTopicSlugRename> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?
            .clone();
        let tenant = ctx.data::<TenantContext>()?;

        execute_rename_forum_topic_slug(db, event_bus, tenant, &auth, tenant_id, topic_id, input)
            .await
    }
}

async fn execute_rename_forum_topic_slug(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant: &TenantContext,
    auth: &AuthContext,
    requested_tenant_id: Option<Uuid>,
    topic_id: Uuid,
    input: RenameForumTopicSlugGraphqlInput,
) -> Result<GqlForumTopicSlugRename> {
    require_topic_update_permission(auth)?;
    let tenant_id = resolve_tenant_scope(tenant, requested_tenant_id)?;

    let result = TopicService::new(db.clone(), event_bus.clone())
        .rename_slug(
            tenant_id,
            topic_id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
            RenameForumTopicSlugInput {
                locale: input.locale,
                slug: input.slug,
            },
        )
        .await?;

    Ok(result.into())
}

fn require_topic_update_permission(auth: &AuthContext) -> Result<()> {
    if !has_any_effective_permission(&auth.permissions, &[Permission::FORUM_TOPICS_UPDATE]) {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Permission denied: forum_topics:update required",
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
pub struct RenameForumTopicSlugGraphqlInput {
    pub locale: String,
    pub slug: String,
}

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct GqlForumTopicRouteDescriptor {
    pub topic_id: Uuid,
    pub locale: String,
    pub short_id: String,
    pub slug: String,
    pub path: String,
}

impl From<ForumTopicRouteDescriptor> for GqlForumTopicRouteDescriptor {
    fn from(value: ForumTopicRouteDescriptor) -> Self {
        Self {
            topic_id: value.topic_id,
            locale: value.locale,
            short_id: value.short_id,
            slug: value.slug,
            path: value.path,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct GqlForumTopicSlugRename {
    pub topic_id: Uuid,
    pub locale: String,
    pub previous_slug: String,
    pub slug: String,
    pub previous_path: String,
    pub canonical: GqlForumTopicRouteDescriptor,
    pub alias_id: Option<Uuid>,
    pub changed: bool,
}

impl From<ForumTopicSlugRenameResult> for GqlForumTopicSlugRename {
    fn from(value: ForumTopicSlugRenameResult) -> Self {
        Self {
            topic_id: value.topic_id,
            locale: value.locale,
            previous_slug: value.previous_slug,
            slug: value.slug,
            previous_path: value.previous_path,
            canonical: value.canonical.into(),
            alias_id: value.alias_id,
            changed: value.changed,
        }
    }
}

#[cfg(test)]
mod tests {
    use rustok_api::{AuthContext, Permission, TenantContext};
    use uuid::Uuid;

    use super::{require_topic_update_permission, resolve_tenant_scope};

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
            name: "Topic slug rename GraphQL tenant".to_string(),
            slug: "topic-slug-rename-graphql".to_string(),
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
    fn rename_transport_requires_topic_update_permission() {
        let denied =
            require_topic_update_permission(&auth_context(vec![Permission::FORUM_TOPICS_READ]))
                .expect_err("read-only actor must not rename a topic slug");
        assert_eq!(error_code(&denied).as_deref(), Some("PERMISSION_DENIED"));

        require_topic_update_permission(&auth_context(vec![Permission::FORUM_TOPICS_UPDATE]))
            .expect("topic update permission must be accepted");
    }

    #[test]
    fn rename_transport_rejects_tenant_override_mismatch() {
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
