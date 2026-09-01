use async_graphql::{Context, InputObject, Object, Result, SimpleObject};
use rustok_api::graphql::require_module_enabled;
use rustok_api::{AuthContext, Permission, TenantContext};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{ForumTopicMergeResult, ForumTopicMergeService, MergeForumTopicInput};

const MODULE_SLUG: &str = "forum";

#[derive(Default)]
pub(crate) struct ForumTopicMergeMutation;

#[Object]
impl ForumTopicMergeMutation {
    async fn merge_forum_topic(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        target_topic_id: Uuid,
        input: MergeForumTopicGraphqlInput,
    ) -> Result<GqlForumTopicMerge> {
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

        execute_merge_forum_topic(
            db,
            event_bus,
            tenant_id,
            auth,
            target_topic_id,
            input,
        )
        .await
    }

    async fn merge_forum_topic_resolving_solution(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        target_topic_id: Uuid,
        input: ResolveForumTopicMergeSolutionGraphqlInput,
    ) -> Result<GqlForumTopicMergeSolutionResolution> {
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

        execute_merge_forum_topic_resolving_solution(
            db,
            event_bus,
            tenant_id,
            auth,
            target_topic_id,
            input,
        )
        .await
    }
}

async fn execute_merge_forum_topic(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    auth: &AuthContext,
    target_topic_id: Uuid,
    input: MergeForumTopicGraphqlInput,
) -> Result<GqlForumTopicMerge> {
    let result = ForumTopicMergeService::new(db.clone(), event_bus.clone())
        .merge_topic(
            tenant_id,
            target_topic_id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
            MergeForumTopicInput {
                operation_id: input.operation_id,
                source_topic_id: input.source_topic_id,
                reason: input.reason,
            },
        )
        .await?;

    Ok(result.into())
}

async fn execute_merge_forum_topic_resolving_solution(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    auth: &AuthContext,
    target_topic_id: Uuid,
    input: ResolveForumTopicMergeSolutionGraphqlInput,
) -> Result<GqlForumTopicMergeSolutionResolution> {
    let selected_solution_reply_id = input.selected_solution_reply_id;

    let result = ForumTopicMergeService::new(db.clone(), event_bus.clone())
        .merge_topic_resolving_solution(
            tenant_id,
            target_topic_id,
            rustok_core::SecurityContext::from_permission_snapshot(
                Some(auth.user_id),
                &auth.permissions,
            ),
            selected_solution_reply_id,
            MergeForumTopicInput {
                operation_id: input.operation_id,
                source_topic_id: input.source_topic_id,
                reason: input.reason,
            },
        )
        .await?;

    Ok(GqlForumTopicMergeSolutionResolution {
        selected_solution_reply_id,
        merge: result.into(),
    })
}

#[derive(Clone, Debug, Eq, PartialEq, InputObject)]
pub struct MergeForumTopicGraphqlInput {
    pub operation_id: Uuid,
    pub source_topic_id: Uuid,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, InputObject)]
pub struct ResolveForumTopicMergeSolutionGraphqlInput {
    pub operation_id: Uuid,
    pub source_topic_id: Uuid,
    pub selected_solution_reply_id: Uuid,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct GqlForumTopicMerge {
    pub operation_id: Uuid,
    pub event_id: Uuid,
    pub source_topic_id: Uuid,
    pub target_topic_id: Uuid,
    pub category_id: Uuid,
    pub actor_id: Uuid,
    pub reason: String,
    pub moved_reply_count: i32,
    pub moved_published_reply_count: i32,
    pub resulting_published_reply_count: i32,
    pub position_offset: i64,
    pub merged_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]
pub struct GqlForumTopicMergeSolutionResolution {
    pub selected_solution_reply_id: Uuid,
    pub merge: GqlForumTopicMerge,
}

impl From<ForumTopicMergeResult> for GqlForumTopicMerge {
    fn from(value: ForumTopicMergeResult) -> Self {
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
            resulting_published_reply_count: value.resulting_published_reply_count,
            position_offset: value.position_offset,
            merged_at: value.merged_at.to_rfc3339(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rustok_api::{AuthContext, Permission, TenantContext};
    use rustok_core::{MigrationSource, SecurityContext, UserRole};
    use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
    use rustok_taxonomy::TaxonomyModule;
    use sea_orm::{
        ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
    };
    use sea_orm_migration::SchemaManager;
    use uuid::Uuid;

    use crate::{CategoryService, CreateCategoryInput, ForumModule};

    use super::{MergeForumTopicGraphqlInput, execute_merge_forum_topic};

    type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

    async fn setup() -> TestResult<(DatabaseConnection, TransactionalEventBus)> {
        let db_url = format!(
            "sqlite:file:forum_topic_merge_graphql_{}?mode=memory&cache=shared",
            Uuid::new_v4()
        );
        let mut options = ConnectOptions::new(db_url);
        options
            .max_connections(5)
            .min_connections(1)
            .sqlx_logging(false);
        let db = Database::connect(options).await?;
        db.execute_unprepared(
            "CREATE TABLE users (\
                id TEXT NOT NULL PRIMARY KEY, \
                tenant_id TEXT NOT NULL, \
                UNIQUE (tenant_id, id)\
            )",
        )
        .await?;
        let schema = SchemaManager::new(&db);
        for migration in OutboxModule.migrations() {
            migration.up(&schema).await?;
        }
        for migration in TaxonomyModule.migrations() {
            migration.up(&schema).await?;
        }
        for migration in ForumModule.migrations() {
            migration.up(&schema).await?;
        }
        let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
        Ok((db, event_bus))
    }

    async fn insert_user(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        user_id: Uuid,
    ) -> TestResult<()> {
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO users (id, tenant_id) VALUES (?, ?)",
            vec![user_id.into(), tenant_id.into()],
        ))
        .await?;
        Ok(())
    }

    fn tenant_context(tenant_id: Uuid) -> TenantContext {
        TenantContext {
            id: tenant_id,
            name: "Topic merge GraphQL tenant".to_string(),
            slug: "topic-merge-graphql".to_string(),
            domain: None,
            settings: serde_json::json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        }
    }

    fn auth_context(tenant_id: Uuid, user_id: Uuid, permissions: Vec<Permission>) -> AuthContext {
        AuthContext {
            user_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions,
            client_id: None,
            scopes: Vec::new(),
            grant_type: "direct".to_string(),
        }
    }

    async fn create_topics(
        db: &DatabaseConnection,
        _event_bus: &TransactionalEventBus,
        tenant_id: Uuid,
        actor_id: Uuid,
    ) -> TestResult<(Uuid, Uuid)> {
        let security = SecurityContext::new(UserRole::Admin, Some(actor_id));
        let category_id = CategoryService::new(db.clone())
            .create(
                tenant_id,
                security.clone(),
                CreateCategoryInput {
                    locale: "en".to_string(),
                    name: "GraphQL merge".to_string(),
                    slug: "graphql-merge".to_string(),
                    description: None,
                    icon: None,
                    color: None,
                    parent_id: None,
                    position: Some(0),
                    moderated: false,
                },
            )
            .await?
            .id;
        let source_topic_id = Uuid::new_v4();
        let target_topic_id = Uuid::new_v4();
        let source_trans_id = Uuid::new_v4();
        let target_trans_id = Uuid::new_v4();
        db.execute_unprepared(&format!(
            "INSERT INTO forum_topics (id, tenant_id, category_id, status, metadata, is_pinned, is_locked, reply_count)
             VALUES (X'{s_id}', X'{t_id}', X'{c_id}', 'open', '{{}}', 0, 0, 0),
                    (X'{tgt_id}', X'{t_id}', X'{c_id}', 'open', '{{}}', 0, 0, 0);
             INSERT INTO forum_topic_translations (id, topic_id, tenant_id, locale, title, body)
             VALUES (X'{st_id}', X'{s_id}', X'{t_id}', 'en', 'GraphQL source', 'Source'),
                    (X'{tt_id}', X'{tgt_id}', X'{t_id}', 'en', 'GraphQL target', 'Target');",
            s_id = source_topic_id.simple().to_string().to_uppercase(),
            tgt_id = target_topic_id.simple().to_string().to_uppercase(),
            t_id = tenant_id.simple().to_string().to_uppercase(),
            c_id = category_id.simple().to_string().to_uppercase(),
            st_id = source_trans_id.simple().to_string().to_uppercase(),
            tt_id = target_trans_id.simple().to_string().to_uppercase(),
        ))
        .await?;
        Ok((source_topic_id, target_topic_id))
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

    fn test_graphql_error(error: async_graphql::Error) -> std::io::Error {
        std::io::Error::other(format!("{error:?}"))
    }

    #[tokio::test]
    async fn merge_transport_enforces_scope_and_replays_one_receipt() -> TestResult<()> {
        let (db, event_bus) = setup().await?;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        insert_user(&db, tenant_id, actor_id).await?;
        let (source_topic_id, target_topic_id) =
            create_topics(&db, &event_bus, tenant_id, actor_id).await?;
        let tenant = tenant_context(tenant_id);
        let operation_id = Uuid::new_v4();
        let input = MergeForumTopicGraphqlInput {
            operation_id,
            source_topic_id,
            reason: "Consolidate duplicate discussion".to_string(),
        };

        let read_only = auth_context(tenant_id, actor_id, vec![Permission::FORUM_TOPICS_READ]);
        assert!(!rustok_api::has_any_effective_permission(
            &read_only.permissions,
            &[Permission::FORUM_TOPICS_MANAGE]
        ));

        let manage_auth = auth_context(tenant_id, actor_id, vec![Permission::FORUM_TOPICS_MANAGE]);
        assert!(rustok_api::has_any_effective_permission(
            &manage_auth.permissions,
            &[Permission::FORUM_TOPICS_MANAGE]
        ));

        assert_eq!(
            crate::graphql::resolve_tenant_scope(&tenant, None).expect("routed tenant must resolve"),
            tenant_id
        );
        let mismatch = crate::graphql::resolve_tenant_scope(&tenant, Some(Uuid::new_v4()))
            .expect_err("tenant override must fail closed");
        assert_eq!(error_code(&mismatch).as_deref(), Some("FORBIDDEN"));

        let first = execute_merge_forum_topic(
            &db,
            &event_bus,
            tenant_id,
            &manage_auth,
            target_topic_id,
            input.clone(),
        )
        .await
        .map_err(test_graphql_error)?;
        let replay = execute_merge_forum_topic(
            &db,
            &event_bus,
            tenant_id,
            &manage_auth,
            target_topic_id,
            input,
        )
        .await
        .map_err(test_graphql_error)?;

        assert_eq!(first, replay);
        assert_eq!(first.operation_id, operation_id);
        assert_eq!(first.event_id, operation_id);
        assert_eq!(first.source_topic_id, source_topic_id);
        assert_eq!(first.target_topic_id, target_topic_id);
        assert_eq!(first.actor_id, actor_id);
        assert_eq!(first.moved_reply_count, 0);
        assert_eq!(first.position_offset, 0);
        assert!(!first.merged_at.is_empty());

        Ok(())
    }
}
