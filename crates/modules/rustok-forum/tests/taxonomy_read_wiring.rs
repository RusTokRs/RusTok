#![recursion_limit = "512"]
//! Consumer-wiring regressions with a read-only provider; not a concrete host adapter test.
#[path = "support/taxonomy_reads.rs"]
mod taxonomy_reads;

use rustok_forum as forum_under_test;
use std::sync::Arc;

use async_graphql::{EmptyMutation, EmptySubscription, Schema, Value};
use async_trait::async_trait;
use rustok_api::{
    AuthContext, HostRuntimeContext, Permission, PortError, SharedStaticModuleSettingsReader,
    StaticModuleSettingsReader, StaticModuleSettingsSnapshot, TenantContext,
    graphql::GraphqlRuntimeInputs,
};
use rustok_forum::{
    ForumCategoryAudienceReadService, ForumError, ForumQuery, ForumSettingsProviders,
    ForumTaxonomyPort,
    entities::*,
    graphql::{ForumGraphqlErrorExtension, attach_schema_data},
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ConnectionTrait, Database, DatabaseBackend,
    DatabaseConnection, Schema as DatabaseSchema, Statement,
};
use taxonomy_reads::TestTaxonomy;
use uuid::Uuid;

type QuerySchema = Schema<ForumQuery, EmptyMutation, EmptySubscription>;

struct OpenForum;
#[async_trait]
impl StaticModuleSettingsReader for OpenForum {
    async fn settings(
        &self,
        _tenant: Uuid,
        _module: &str,
    ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
        Ok(Some(StaticModuleSettingsSnapshot {
            enabled: Some(true),
            settings: serde_json::json!({"allow_anonymous_reading": true}),
        }))
    }
}

async fn fixture(port: &TestTaxonomy, configured: bool) -> (DatabaseConnection, QuerySchema) {
    fixture_with_permissions(port, configured, None).await
}

async fn fixture_with_permissions(
    port: &TestTaxonomy,
    configured: bool,
    permissions: Option<Vec<Permission>>,
) -> (DatabaseConnection, QuerySchema) {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let schema = DatabaseSchema::new(DatabaseBackend::Sqlite);
    macro_rules! table {
        ($module:ident) => {
            db.execute_raw(
                DatabaseBackend::Sqlite.build(&schema.create_table_from_entity($module::Entity)),
            )
            .await
            .unwrap();
        };
    }
    table!(forum_category);
    table!(forum_category_lifecycle);
    table!(forum_category_policy);
    table!(forum_category_subscription);
    table!(forum_category_audience_policy);
    table!(forum_category_audience_role);
    table!(forum_category_audience_channel);
    table!(forum_category_audience_group);
    table!(forum_category_audience_user);
    table!(forum_topic);
    // Raw topic writer locking uses the lifecycle column intentionally omitted
    // from the read-model entity. Mirror it for command admission regressions.
    db.execute_unprepared("ALTER TABLE forum_topics ADD COLUMN deleted_at TEXT NULL")
        .await
        .unwrap();
    table!(forum_topic_channel_access);
    table!(forum_topic_audience_policy);
    table!(forum_topic_audience_role);
    table!(forum_topic_audience_channel);
    table!(forum_topic_audience_group);
    table!(forum_topic_audience_user);
    table!(forum_reply);
    for id in [port.parent, port.category] {
        forum_category::ActiveModel {
            id: Set(id),
            tenant_id: Set(port.tenant),
            moderated: Set(false),
            topic_count: Set(0),
            reply_count: Set(0),
            created_at: Set(chrono::Utc::now().into()),
            updated_at: Set(chrono::Utc::now().into()),
        }
        .insert(&db)
        .await
        .unwrap();
    }
    db.execute_unprepared(
        "CREATE TABLE tenant_modules (tenant_id TEXT, module_slug TEXT, enabled INTEGER)",
    )
    .await
    .unwrap();
    db.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO tenant_modules VALUES (?, 'forum', 1)",
        [port.tenant.to_string().into()],
    ))
    .await
    .unwrap();
    let mut host = HostRuntimeContext::new(db.clone())
        .with_shared_value(SharedStaticModuleSettingsReader(Arc::new(OpenForum)));
    if configured {
        let provider: Arc<dyn ForumTaxonomyPort> = Arc::new(port.clone());
        host = host.with_shared_value(provider);
    }
    let runtime = attach_schema_data(&GraphqlRuntimeInputs::new(host)).unwrap();
    let mut query = Schema::build(ForumQuery::default(), EmptyMutation, EmptySubscription)
        .extension(ForumGraphqlErrorExtension)
        .data(db.clone())
        .data(rustok_outbox::TransactionalEventBus::new(Arc::new(
            rustok_outbox::OutboxTransport::new(db.clone()),
        )))
        .data(runtime)
        .data(TenantContext {
            id: port.tenant,
            name: "Forum wiring".into(),
            slug: "forum-wiring".into(),
            domain: None,
            settings: serde_json::json!({}),
            default_locale: "en".into(),
            is_active: true,
        });
    if let Some(permissions) = permissions {
        query = query.data(AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id: port.tenant,
            permissions,
            client_id: None,
            scopes: Vec::new(),
            grant_type: "user".into(),
        });
    }
    let query = query.finish();
    // Deliberately no Taxonomy tables: all foreign metadata/hierarchy comes from the port.
    (db, query)
}

#[tokio::test]
async fn graphql_injected_category_route_resolves_alias_and_canonical_metadata() {
    let port = TestTaxonomy::new();
    let (_, schema) = fixture(&port, true).await;
    for (slug, disposition) in [("help", "CANONICAL"), ("old-help", "REDIRECT")] {
        let response = schema.execute(format!("{{ result: forumStorefrontCategoryRoute(locale: \"en\", slug: \"{slug}\") {{ disposition canonical {{ categoryId slug path }} }} }}")).await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert_eq!(
            response.data.into_json().unwrap(),
            serde_json::json!({"result": {
                "disposition": disposition, "canonical": {"categoryId": port.category.to_string(), "slug": "help", "path": "/en/forum/c/help"}
            }})
        );
    }
    let calls = port.calls.lock().unwrap();
    for operation in ["route", "parents", "snapshots"] {
        assert!(calls.contains(&operation), "{calls:?}");
    }
}

#[tokio::test]
async fn graphql_configured_provider_failures_are_typed_redacted_and_never_null() {
    for operation in ["route", "parents", "snapshots"] {
        let mut port = TestTaxonomy::new();
        port.fail_at = Some(operation);
        let (_, schema) = fixture(&port, true).await;
        let response = schema.execute("{ result: forumStorefrontCategoryRoute(locale: \"en\", slug: \"help\") { disposition } }").await;
        assert_eq!(
            response.errors.len(),
            1,
            "{operation}: {:?}",
            response.errors
        );
        let error = &response.errors[0];
        assert!(
            matches!(
                error.source::<ForumError>(),
                Some(ForumError::CapabilityFailure { .. })
            ),
            "{operation}: {error:?}"
        );
        assert_eq!(error.message, "Forum capability operation failed");
        let extensions = error.extensions.as_ref().unwrap();
        assert_eq!(
            extensions.get("code"),
            Some(&Value::from("FORUM_CAPABILITY_FAILURE"))
        );
        assert_eq!(extensions.get("retryable"), Some(&Value::from(true)));
        assert_eq!(extensions.get("retryAfter"), Some(&Value::from(5)));
        assert!(port.calls.lock().unwrap().contains(&operation));
    }
}

#[tokio::test]
async fn anonymous_route_still_hides_a_child_of_an_authenticated_category() {
    let port = TestTaxonomy::new();
    let (db, schema) = fixture(&port, true).await;
    forum_category_policy::ActiveModel {
        category_id: Set(port.parent),
        tenant_id: Set(port.tenant),
        allows_topics: Set(true),
        visibility_override: Set(Some(rustok_forum::ForumCategoryVisibility::Authenticated)),
        updated_at: Set(chrono::Utc::now().into()),
    }
    .insert(&db)
    .await
    .unwrap();
    let response = schema.execute("{ result: forumStorefrontCategoryRoute(locale: \"en\", slug: \"help\") { disposition } }").await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap(),
        serde_json::json!({"result": null})
    );
    assert!(port.calls.lock().unwrap().contains(&"parents"));
}

#[tokio::test]
async fn anonymous_route_still_hides_a_child_with_an_inherited_richer_audience_layer() {
    let port = TestTaxonomy::new();
    let (db, schema) = fixture(&port, true).await;
    forum_category_audience_policy::ActiveModel {
        category_id: Set(port.parent),
        tenant_id: Set(port.tenant),
        minimum_trust_level: Set(Some(1)),
        updated_at: Set(chrono::Utc::now().into()),
    }
    .insert(&db)
    .await
    .unwrap();
    let response = schema.execute("{ result: forumStorefrontCategoryRoute(locale: \"en\", slug: \"help\") { disposition } }").await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap(),
        serde_json::json!({"result": null})
    );
    assert!(port.calls.lock().unwrap().contains(&"parents"));
}

#[tokio::test]
async fn standalone_route_keeps_absence_without_invoking_an_unconfigured_provider() {
    let port = TestTaxonomy::new();
    let (_, schema) = fixture(&port, false).await;
    let response = schema.execute("{ result: forumStorefrontCategoryRoute(locale: \"en\", slug: \"help\") { disposition } }").await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap(),
        serde_json::json!({"result": null})
    );
    assert!(port.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn composed_audience_reader_uses_provider_metadata_and_propagates_hierarchy_failure() {
    let mut port = TestTaxonomy::new();
    let (db, _) = fixture(&port, true).await;
    let settings = ForumSettingsProviders::default()
        .with_static_reader(SharedStaticModuleSettingsReader(Arc::new(OpenForum)));
    let service = ForumCategoryAudienceReadService::new(db.clone())
        .with_settings_providers(settings.clone())
        .with_taxonomy_port(Arc::new(port.clone()));
    let category = service
        .get_public_storefront_visible_with_locale_fallback(port.tenant, port.category, "en", None)
        .await
        .unwrap();
    assert_eq!(category.name, "Provider category");
    assert_eq!(category.slug, "help");
    assert_eq!(category.parent_id, Some(port.parent));
    port.fail_at = Some("parents");
    let service = ForumCategoryAudienceReadService::new(db)
        .with_settings_providers(settings)
        .with_taxonomy_port(Arc::new(port.clone()));
    let error = service
        .get_public_storefront_visible_with_locale_fallback(port.tenant, port.category, "en", None)
        .await
        .unwrap_err();
    assert!(matches!(error, ForumError::CapabilityFailure { .. }));
}

#[tokio::test]
async fn cross_tenant_route_request_does_not_reach_the_provider() {
    let port = TestTaxonomy::new();
    let (_, schema) = fixture(&port, true).await;
    let foreign = Uuid::new_v4();
    let response = schema.execute(format!("{{ result: forumStorefrontCategoryRoute(tenantId: \"{foreign}\", locale: \"en\", slug: \"help\") {{ disposition }} }}")).await;
    assert_eq!(response.errors.len(), 1, "{:?}", response.errors);
    assert!(port.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn unknown_provider_route_keeps_privacy_safe_absence() {
    let port = TestTaxonomy::new();
    let (_, schema) = fixture(&port, true).await;
    let response = schema.execute("{ result: forumStorefrontCategoryRoute(locale: \"en\", slug: \"missing\") { disposition } }").await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap(),
        serde_json::json!({"result": null})
    );
    assert_eq!(*port.calls.lock().unwrap(), vec!["route"]);
}

#[tokio::test]
async fn authenticated_route_keeps_list_only_admission_without_granting_owner_read() {
    let port = TestTaxonomy::new();
    for (permissions, admitted) in [
        (vec![Permission::FORUM_CATEGORIES_LIST], true),
        (vec![Permission::FORUM_CATEGORIES_READ], false),
        (vec![], false),
    ] {
        let (_, schema) = fixture_with_permissions(&port, true, Some(permissions)).await;
        let route = schema.execute("{ result: forumStorefrontCategoryRoute(locale: \"en\", slug: \"help\") { canonical { slug } } }").await;
        if admitted {
            assert!(route.errors.is_empty(), "{:?}", route.errors);
            assert_eq!(
                route.data.into_json().unwrap(),
                serde_json::json!({"result": {"canonical": {"slug": "help"}}})
            );
            let owner = schema
                .execute(format!(
                    "{{ result: forumCategory(id: \"{}\") {{ id }} }}",
                    port.category
                ))
                .await;
            assert_eq!(owner.errors.len(), 1, "list must not grant owner read");
            assert_eq!(
                owner.errors[0].message,
                "Permission denied: forum_categories:read required"
            );
        } else {
            assert_eq!(route.errors.len(), 1, "read must not grant storefront list");
            assert!(matches!(
                route.errors[0].source::<ForumError>(),
                Some(ForumError::Forbidden(_))
            ));
        }
    }
}

#[tokio::test]
async fn malformed_provider_hierarchy_is_a_failure_even_behind_a_local_visibility_override() {
    use std::collections::HashMap;
    let original = TestTaxonomy::new();
    let root = original.parent;
    let child = original.category;
    let foreign = Uuid::new_v4();
    for parents in [
        HashMap::new(),
        HashMap::from([(child, Some(root))]),
        HashMap::from([(root, None), (child, None), (foreign, None)]),
        HashMap::from([(root, None), (child, Some(foreign))]),
        HashMap::from([(root, Some(child)), (child, Some(root))]),
    ] {
        let mut port = original.clone();
        port.parent_projection = Some(parents);
        let (db, schema) = fixture(&port, true).await;
        forum_category_policy::ActiveModel {
            category_id: Set(child),
            tenant_id: Set(port.tenant),
            allows_topics: Set(true),
            visibility_override: Set(Some(rustok_forum::ForumCategoryVisibility::Authenticated)),
            updated_at: Set(chrono::Utc::now().into()),
        }
        .insert(&db)
        .await
        .unwrap();
        let response = schema.execute("{ result: forumStorefrontCategoryRoute(locale: \"en\", slug: \"help\") { disposition } }").await;
        assert_eq!(
            response.errors.len(),
            1,
            "invalid hierarchy must not be disguised as absence"
        );
        assert!(matches!(
            response.errors[0].source::<ForumError>(),
            Some(ForumError::Validation(_))
        ));
        let service = rustok_forum::CategoryService::new(db.clone())
            .with_taxonomy_port(Arc::new(port.clone()));
        let result = service
            .list_paginated_with_locale_fallback(
                port.tenant,
                rustok_core::SecurityContext::public_read(),
                "en",
                1,
                20,
                None,
            )
            .await;
        assert!(
            matches!(result, Err(ForumError::Validation(_))),
            "incomplete hierarchy must not produce an unfiltered public list"
        );
        let search = rustok_forum::services::ForumSearchCategoryScopeService::new(db)
            .with_taxonomy_port(Arc::new(port.clone()));
        assert!(matches!(
            search
                .expand_visible_subtrees(
                    port.tenant,
                    rustok_core::SecurityContext::public_read(),
                    &[child]
                )
                .await,
            Err(ForumError::Validation(_))
        ));
    }
}

#[tokio::test]
async fn duplicate_or_foreign_category_tree_metadata_returns_validation_instead_of_panicking() {
    for malformed in ["duplicate", "foreign"] {
        let mut port = TestTaxonomy::new();
        port.malformed_snapshots = Some(malformed);
        let (db, _) = fixture(&port, true).await;
        let service =
            rustok_forum::CategoryService::new(db).with_taxonomy_port(Arc::new(port.clone()));
        let result = service
            .tree(
                port.tenant,
                rustok_core::SecurityContext::public_read(),
                rustok_forum::CategoryTreeQuery {
                    locale: Some("en".into()),
                    fallback_locale: None,
                },
            )
            .await;
        assert!(
            matches!(result, Err(ForumError::Validation(_))),
            "{malformed}: {result:?}"
        );
    }
}

#[tokio::test]
async fn provider_failure_after_policy_write_uses_same_transaction_and_rolls_back() {
    let mut port = TestTaxonomy::new();
    port.require_transaction = true;
    port.fail_at = Some("parents");
    port.fail_on_call = 2;
    let (db, _) = fixture(&port, true).await;
    let service = rustok_forum::ForumCategoryVisibilityPolicyService::new(db.clone())
        .with_taxonomy_port(Arc::new(port.clone()));
    let result = service
        .set(
            port.tenant,
            port.category,
            rustok_core::SecurityContext::system(),
            rustok_forum::SetForumCategoryVisibilityPolicyInput {
                visibility: rustok_forum::ForumCategoryVisibility::Authenticated,
            },
        )
        .await;
    assert!(matches!(result, Err(ForumError::CapabilityFailure { .. })));
    assert_eq!(*port.calls.lock().unwrap(), vec!["parents", "parents"]);
    let row = db
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT COUNT(*) AS count FROM forum_category_policies".to_string(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        row.try_get::<i64>("", "count").unwrap(),
        0,
        "failed post-write reload must roll back the policy"
    );
    // No outbox table exists: the failed write cannot reach success invalidation publication.
}

#[path = "support/taxonomy_topic_reads.rs"]
mod topic_reads;

#[path = "support/taxonomy_create_authorization.rs"]
mod create_authorization;

#[path = "support/taxonomy_moderation_authorization.rs"]
mod moderation_authorization;
