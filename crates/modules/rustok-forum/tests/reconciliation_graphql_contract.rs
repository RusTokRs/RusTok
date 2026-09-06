#![recursion_limit = "256"]

use async_graphql::{EmptyMutation, EmptySubscription, Request, Schema};
use rustok_api::{Action, AuthContext, Permission, Resource, TenantContext};
use rustok_core::MigrationSource;
use rustok_forum::graphql::ForumGraphqlErrorExtension;
use rustok_forum::{ForumModule, ForumQuery};
use rustok_outbox::OutboxModule;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

const COUNTER_AND_SOLUTION_GRAPHQL: &str =
    include_str!("../src/graphql/reconciliation_query.rs");
const SUBSCRIPTION_GRAPHQL: &str =
    include_str!("../src/graphql/subscription_reconciliation_query.rs");
const MENTION_GRAPHQL: &str =
    include_str!("../src/graphql/mention_reconciliation_query.rs");

#[test]
fn graphql_schema_exposes_all_reconciliation_reports() {
    let schema = Schema::build(
        ForumQuery::default(),
        EmptyMutation,
        EmptySubscription,
    )
    .extension(ForumGraphqlErrorExtension)
    .finish();
    let sdl = schema.sdl();

    for marker in [
        "forumCounterReconciliationReport",
        "forumSolutionReconciliationReport",
        "forumSubscriptionReconciliationReport",
        "forumMentionReconciliationReport",
        "GqlForumCounterReconciliationReport",
        "GqlForumCounterDrift",
        "GqlForumSolutionReconciliationReport",
        "GqlForumSolutionDrift",
        "GqlForumSubscriptionReconciliationReport",
        "GqlForumSubscriptionDrift",
        "GqlForumSubscriptionCursor",
        "GqlForumMentionReconciliationReport",
        "GqlForumMentionDrift",
        "inspectedTopics",
        "inspectedCategories",
        "topicCursor",
        "categoryCursor",
        "inspectedSolutions",
        "inspectedSolutionStats",
        "solutionCursor",
        "solutionStatCursor",
        "inspectedTopicSubscriptions",
        "inspectedCategorySubscriptions",
        "inspectedRelationRevisions",
        "inspectedMentionRevisions",
        "relationCursor",
        "driftCount",
        "clean",
    ] {
        assert!(
            sdl.contains(marker),
            "missing GraphQL reconciliation marker {marker}"
        );
    }
}

#[test]
fn graphql_reconciliation_adapters_enforce_security_scope_and_isolation() {
    for (source_name, source) in [
        ("counter/solution", COUNTER_AND_SOLUTION_GRAPHQL),
        ("subscription", SUBSCRIPTION_GRAPHQL),
        ("mention", MENTION_GRAPHQL),
    ] {
        for marker in [
            "require_module_enabled(ctx, MODULE_SLUG).await?",
            "Permission::FORUM_CATEGORIES_MANAGE",
            "Permission::FORUM_TOPICS_MANAGE",
            "auth.tenant_id != tenant.id",
            "Permission denied: tenant scope mismatch",
            "SecurityContext::from_permission_snapshot",
        ] {
            assert!(
                source.contains(marker),
                "{source_name} adapter missing security marker {marker}"
            );
        }

        for forbidden in [
            "UPDATE forum_",
            "DELETE FROM forum_",
            "INSERT INTO forum_",
            "ActiveModel",
        ] {
            assert!(
                !source.contains(forbidden),
                "{source_name} adapter contains forbidden write mutation {forbidden}"
            );
        }
    }

    assert!(COUNTER_AND_SOLUTION_GRAPHQL.contains("ForumCounterReconciliationService::new(db)"));
    assert!(COUNTER_AND_SOLUTION_GRAPHQL.contains("ForumSolutionReconciliationService::new(db)"));
    assert!(SUBSCRIPTION_GRAPHQL.contains("ForumSubscriptionReconciliationService::new(db)"));
    assert!(MENTION_GRAPHQL.contains("ForumMentionReconciliationService::new(db)"));
}

async fn setup_test_db() -> DatabaseConnection {
    let mut opt = ConnectOptions::new("sqlite::memory:".to_string());
    opt.max_connections(1);
    let db = Database::connect(opt)
        .await
        .expect("in-memory sqlite should connect");

    let manager = SchemaManager::new(&db);
    for migration in OutboxModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("outbox migration should apply");
    }
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("taxonomy migration should apply");
    }
    db.execute_unprepared(
        "CREATE TABLE IF NOT EXISTS users (
            id TEXT NOT NULL PRIMARY KEY,
            tenant_id TEXT NOT NULL
        );",
    )
    .await
    .expect("users table fixture should apply");
    db.execute_unprepared(
        "CREATE TABLE IF NOT EXISTS tenant_modules (
            tenant_id BLOB NOT NULL,
            module_slug TEXT NOT NULL,
            enabled INTEGER NOT NULL,
            PRIMARY KEY (tenant_id, module_slug)
        );",
    )
    .await
    .expect("tenant_modules fixture should apply");
    for migration in ForumModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("forum migration should apply");
    }

    db
}

fn sql_uuid(id: Uuid) -> String {
    format!("X'{}'", id.simple().to_string().to_uppercase())
}

fn test_tenant_context(tenant_id: Uuid) -> TenantContext {
    TenantContext {
        id: tenant_id,
        name: "Test Tenant".to_string(),
        slug: "test-tenant".to_string(),
        domain: None,
        settings: serde_json::json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    }
}

fn test_auth_context(tenant_id: Uuid, permissions: Vec<Permission>) -> AuthContext {
    AuthContext {
        user_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions,
        client_id: None,
        scopes: Vec::new(),
        grant_type: "direct".to_string(),
    }
}

#[tokio::test]
async fn graphql_reconciliation_execution_rejects_unauthenticated_and_unauthorized() {
    let db = setup_test_db().await;
    let tenant_id = Uuid::new_v4();
    let tenant = test_tenant_context(tenant_id);

    db.execute_unprepared(&format!(
        "INSERT INTO tenant_modules (tenant_id, module_slug, enabled) VALUES ({}, 'forum', 1);",
        sql_uuid(tenant_id)
    ))
    .await
    .expect("tenant_modules seed should apply");

    let schema = Schema::build(
        ForumQuery::default(),
        EmptyMutation,
        EmptySubscription,
    )
    .extension(ForumGraphqlErrorExtension)
    .finish();

    let query = r#"
        query {
            forumCounterReconciliationReport(limit: 10) {
                clean
                driftCount
            }
        }
    "#;

    // 1. Unauthenticated (no AuthContext in request data)
    let req = Request::new(query)
        .data(tenant.clone())
        .data(db.clone());
    let res = schema.execute(req).await;
    assert!(!res.errors.is_empty(), "expected unauthenticated error");
    let err_msg = res.errors[0].message.to_lowercase();
    assert!(
        err_msg.contains("unauthenticated") || err_msg.contains("authentication"),
        "expected unauthenticated error message, got: {}",
        res.errors[0].message
    );

    // 2. Missing topics:manage permission
    let partial_auth = test_auth_context(
        tenant_id,
        vec![Permission::new(Resource::ForumCategories, Action::Manage)],
    );
    let req = Request::new(query)
        .data(tenant.clone())
        .data(partial_auth)
        .data(db.clone());
    let res = schema.execute(req).await;
    assert!(!res.errors.is_empty(), "expected permission denied error");
    assert!(
        res.errors[0].message.contains("Permission denied"),
        "expected permission denied message, got: {}",
        res.errors[0].message
    );

    // 3. Tenant mismatch
    let foreign_tenant_id = Uuid::new_v4();
    let full_perms = vec![
        Permission::new(Resource::ForumCategories, Action::Manage),
        Permission::new(Resource::ForumTopics, Action::Manage),
    ];
    let mismatch_auth = test_auth_context(foreign_tenant_id, full_perms.clone());
    let req = Request::new(query)
        .data(tenant.clone())
        .data(mismatch_auth)
        .data(db.clone());
    let res = schema.execute(req).await;
    assert!(!res.errors.is_empty(), "expected tenant mismatch error");
    assert!(
        res.errors[0].message.contains("tenant scope mismatch"),
        "expected tenant scope mismatch message, got: {}",
        res.errors[0].message
    );
}

#[tokio::test]
async fn graphql_reconciliation_execution_succeeds_for_operator_on_clean_state() {
    let db = setup_test_db().await;
    let tenant_id = Uuid::new_v4();
    let tenant = test_tenant_context(tenant_id);

    db.execute_unprepared(&format!(
        "INSERT INTO tenant_modules (tenant_id, module_slug, enabled) VALUES ({}, 'forum', 1);",
        sql_uuid(tenant_id)
    ))
    .await
    .expect("tenant_modules seed should apply");

    let schema = Schema::build(
        ForumQuery::default(),
        EmptyMutation,
        EmptySubscription,
    )
    .extension(ForumGraphqlErrorExtension)
    .finish();

    let operator_auth = test_auth_context(
        tenant_id,
        vec![
            Permission::new(Resource::ForumCategories, Action::Manage),
            Permission::new(Resource::ForumTopics, Action::Manage),
        ],
    );

    // 1. Counter report
    let query = r#"
        query {
            forumCounterReconciliationReport(limit: 10) {
                clean
                driftCount
                inspectedTopics
                inspectedCategories
                hasMoreTopics
                hasMoreCategories
            }
        }
    "#;
    let req = Request::new(query)
        .data(tenant.clone())
        .data(operator_auth.clone())
        .data(db.clone());
    let res = schema.execute(req).await;
    assert!(res.errors.is_empty(), "unexpected counter report errors: {:?}", res.errors);
    let data = res.data.into_json().expect("valid JSON response");
    assert_eq!(data["forumCounterReconciliationReport"]["clean"], true);
    assert_eq!(data["forumCounterReconciliationReport"]["driftCount"], 0);

    // 2. Solution report
    let query = r#"
        query {
            forumSolutionReconciliationReport(limit: 10) {
                clean
                driftCount
                inspectedSolutions
                inspectedSolutionStats
                hasMoreSolutions
                hasMoreSolutionStats
            }
        }
    "#;
    let req = Request::new(query)
        .data(tenant.clone())
        .data(operator_auth.clone())
        .data(db.clone());
    let res = schema.execute(req).await;
    assert!(res.errors.is_empty(), "unexpected solution report errors: {:?}", res.errors);
    let data = res.data.into_json().expect("valid JSON response");
    assert_eq!(data["forumSolutionReconciliationReport"]["clean"], true);
    assert_eq!(data["forumSolutionReconciliationReport"]["driftCount"], 0);

    // 3. Subscription report
    let query = r#"
        query {
            forumSubscriptionReconciliationReport(limit: 10) {
                clean
                driftCount
                inspectedTopicSubscriptions
                inspectedCategorySubscriptions
                hasMoreTopicSubscriptions
                hasMoreCategorySubscriptions
            }
        }
    "#;
    let req = Request::new(query)
        .data(tenant.clone())
        .data(operator_auth.clone())
        .data(db.clone());
    let res = schema.execute(req).await;
    assert!(res.errors.is_empty(), "unexpected subscription report errors: {:?}", res.errors);
    let data = res.data.into_json().expect("valid JSON response");
    assert_eq!(data["forumSubscriptionReconciliationReport"]["clean"], true);
    assert_eq!(data["forumSubscriptionReconciliationReport"]["driftCount"], 0);

    // 4. Mention report
    let query = r#"
        query {
            forumMentionReconciliationReport(limit: 10) {
                clean
                driftCount
                inspectedRelationRevisions
                inspectedMentionRevisions
                hasMoreRelationRevisions
            }
        }
    "#;
    let req = Request::new(query)
        .data(tenant.clone())
        .data(operator_auth.clone())
        .data(db.clone());
    let res = schema.execute(req).await;
    assert!(res.errors.is_empty(), "unexpected mention report errors: {:?}", res.errors);
    let data = res.data.into_json().expect("valid JSON response");
    assert_eq!(data["forumMentionReconciliationReport"]["clean"], true);
    assert_eq!(data["forumMentionReconciliationReport"]["driftCount"], 0);
}
