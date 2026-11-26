#![recursion_limit = "512"]
//! Exercise typed Forum read failures, including settings defaults and aliased paths.
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use async_graphql::{EmptyMutation, EmptySubscription, Schema, Value};
use async_trait::async_trait;
use rustok_api::{
    AuthContext, HostRuntimeContext, Permission, PortError, RequestContext,
    SharedStaticModuleSettingsReader, SharedStaticModuleSettingsTransactionReader,
    StaticModuleSettingsReader, StaticModuleSettingsSnapshot,
    StaticModuleSettingsTransactionReader, TenantContext, graphql::GraphqlRuntimeInputs,
};
use rustok_forum::{
    ForumError, ForumQuery,
    graphql::{ForumGraphqlErrorExtension, attach_schema_data},
};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, DatabaseTransaction, Statement};
use uuid::Uuid;

struct UnavailableSettings(Arc<AtomicUsize>);

#[async_trait]
impl StaticModuleSettingsReader for UnavailableSettings {
    async fn settings(
        &self,
        _tenant: Uuid,
        _slug: &str,
    ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(PortError::unavailable(
            "test.settings_unavailable",
            "secret-owner-diagnostic",
        ))
    }
}

struct TransactionOnlySettings;

#[async_trait]
impl StaticModuleSettingsTransactionReader for TransactionOnlySettings {
    async fn settings_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant: Uuid,
        _slug: &str,
    ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
        panic!("a missing static capability must fail before transactional reads")
    }
}

#[derive(Clone, Copy)]
enum SettingsMode {
    Unavailable,
    TransactionOnly,
    DefaultOnly,
}

type TestSchema = Schema<ForumQuery, EmptyMutation, EmptySubscription>;

async fn test_schema(mode: SettingsMode) -> (TestSchema, Arc<AtomicUsize>) {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    // Deliberately omit Forum source/projection/outbox tables. Settings cases
    // must stop before owner reads; selected-read cases exercise typed DB errors.
    db.execute_unprepared(
        "CREATE TABLE tenant_modules (tenant_id TEXT, module_slug TEXT, enabled INTEGER)",
    )
    .await
    .unwrap();
    let tenant = Uuid::new_v4();
    let actor = Uuid::new_v4();
    db.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "INSERT INTO tenant_modules VALUES (?, 'forum', 1)",
        [tenant.to_string().into()],
    ))
    .await
    .unwrap();
    let reads = Arc::new(AtomicUsize::new(0));
    let host = match mode {
        SettingsMode::TransactionOnly => HostRuntimeContext::new(db.clone()).with_shared_value(
            SharedStaticModuleSettingsTransactionReader(Arc::new(TransactionOnlySettings)),
        ),
        SettingsMode::Unavailable => HostRuntimeContext::new(db.clone()).with_shared_value(
            SharedStaticModuleSettingsReader(Arc::new(UnavailableSettings(reads.clone()))),
        ),
        SettingsMode::DefaultOnly => HostRuntimeContext::new(db.clone()),
    };
    let runtime = attach_schema_data(&GraphqlRuntimeInputs::new(host)).unwrap();
    let schema = Schema::build(ForumQuery::default(), EmptyMutation, EmptySubscription)
        .extension(ForumGraphqlErrorExtension)
        .data(db.clone())
        .data(TransactionalEventBus::new(Arc::new(OutboxTransport::new(
            db,
        ))))
        .data(runtime)
        .data(TenantContext {
            id: tenant,
            name: "Settings test".into(),
            slug: "settings-test".into(),
            domain: None,
            settings: serde_json::json!({}),
            default_locale: "en".into(),
            is_active: true,
        })
        .data(AuthContext {
            user_id: actor,
            session_id: Uuid::new_v4(),
            tenant_id: tenant,
            permissions: vec![
                Permission::FORUM_CATEGORIES_READ,
                Permission::FORUM_TOPICS_LIST,
                Permission::FORUM_TOPICS_READ,
                Permission::FORUM_REPLIES_LIST,
                Permission::FORUM_REPLIES_READ,
            ],
            client_id: None,
            scopes: Vec::new(),
            grant_type: "user".into(),
        })
        .data(RequestContext {
            tenant_id: tenant,
            user_id: Some(actor),
            channel_id: None,
            channel_slug: None,
            channel_resolution_source: None,
            locale: "en".into(),
            correlation_id: Uuid::new_v4().to_string(),
        })
        .finish();
    (schema, reads)
}

#[tokio::test]
async fn graphql_settings_defaults_preserve_capability_codes_and_retry_contract() {
    for transaction_only in [false, true] {
        let (schema, reads) = test_schema(if transaction_only {
            SettingsMode::TransactionOnly
        } else {
            SettingsMode::Unavailable
        })
        .await;
        let topic = Uuid::new_v4();
        for (index, field) in [
            "forumTopics(order: LATEST)".to_string(),
            "forumTopics(perPage: 1)".to_string(),
            "forumStorefrontAudienceTopics(order: LATEST)".to_string(),
            "forumStorefrontAudienceTopics(perPage: 1)".to_string(),
            "forumStorefrontUnreadTopics(order: LATEST)".to_string(),
            "forumStorefrontUnreadTopics(limit: 1)".to_string(),
            format!("forumReplies(topicId: \"{topic}\")"),
            format!("forumStorefrontReplies(topicId: \"{topic}\")"),
        ]
        .into_iter()
        .enumerate()
        {
            // An alias deliberately prevents the legacy Forum-path/message
            // fallback from rescuing a resolver that has erased the source.
            let response = schema
                .execute(format!("{{ result: {field} {{ items {{ id }} }} }}"))
                .await;
            assert_eq!(response.errors.len(), 1, "{field}: {:?}", response.errors);
            let error = &response.errors[0];
            assert!(error.source::<ForumError>().is_some(), "{field}: {error:?}");
            let extensions = error.extensions.as_ref().expect("typed error extensions");
            assert_eq!(
                extensions.get("code"),
                Some(&Value::from(if transaction_only {
                    "FORUM_STATIC_SETTINGS_UNAVAILABLE"
                } else {
                    "FORUM_CAPABILITY_FAILURE"
                })),
                "{field}"
            );
            assert_eq!(
                extensions.get("retryable"),
                Some(&Value::from(!transaction_only))
            );
            if transaction_only {
                assert_eq!(extensions.get("retryAfter"), None);
            } else {
                assert_eq!(extensions.get("retryAfter"), Some(&Value::from(5)));
            }
            assert!(!error.message.contains("secret-owner-diagnostic"));
            assert_eq!(
                reads.load(Ordering::SeqCst),
                if transaction_only { 0 } else { index + 1 }
            );
        }
    }
}

#[tokio::test]
async fn graphql_selected_reads_preserve_typed_storage_failures_under_aliases() {
    let (schema, _) = test_schema(SettingsMode::DefaultOnly).await;
    let id = Uuid::new_v4();
    for field in [
        format!("forumCategory(id: \"{id}\") {{ id }}"),
        format!("forumTopic(id: \"{id}\") {{ id }}"),
        "forumStorefrontTopicRoute(locale: \"en\", shortId: \"0123456789abcdef\", slug: \"topic\") { disposition }".to_string(),
        "forumStorefrontTopicRouteDecision(locale: \"en\", shortId: \"0123456789abcdef\", slug: \"topic\") { disposition }".to_string(),
    ] {
        let response = schema.execute(format!("{{ result: {field} }}")).await;
        assert_eq!(response.errors.len(), 1, "{field}: {:?}", response.errors);
        let error = &response.errors[0];
        assert!(matches!(error.source::<ForumError>(), Some(ForumError::Database(_))), "{field}: {error:?}");
        assert_eq!(error.message, "Forum persistence operation failed");
        let extensions = error.extensions.as_ref().expect("typed storage failure");
        assert_eq!(extensions.get("code"), Some(&Value::from("FORUM_INTERNAL_ERROR")));
        assert_eq!(extensions.get("retryable"), Some(&Value::from(true)));
        assert_eq!(extensions.get("retryAfter"), None);
    }
}

#[tokio::test]
async fn graphql_member_card_validation_preserves_typed_error_under_alias() {
    let (schema, _) = test_schema(SettingsMode::DefaultOnly).await;
    let response = schema
        .execute(format!(
            "{{ result: forumMemberCards(userIds: [\"{}\"]) {{ userId }} }}",
            Uuid::nil(),
        ))
        .await;
    assert_eq!(response.errors.len(), 1, "{:?}", response.errors);
    let error = &response.errors[0];
    assert!(matches!(
        error.source::<ForumError>(),
        Some(ForumError::Validation(_))
    ));
    let extensions = error.extensions.as_ref().expect("typed validation failure");
    assert_eq!(
        extensions.get("code"),
        Some(&Value::from("FORUM_VALIDATION_FAILED"))
    );
    assert_eq!(extensions.get("retryable"), Some(&Value::from(false)));
    assert_eq!(extensions.get("retryAfter"), None);
}

#[tokio::test]
async fn graphql_route_validation_preserves_typed_error_under_aliases() {
    let (schema, _) = test_schema(SettingsMode::DefaultOnly).await;
    for field in [
        "forumStorefrontCategoryRoute(locale: \"\", slug: \"general\") { disposition }",
        "forumStorefrontTopicRoute(locale: \"\", shortId: \"0123456789abcdef\", slug: \"topic\") { disposition }",
        "forumStorefrontTopicRouteDecision(locale: \"\", shortId: \"0123456789abcdef\", slug: \"topic\") { disposition }",
    ] {
        let response = schema.execute(format!("{{ result: {field} }}")).await;
        assert_eq!(response.errors.len(), 1, "{field}: {:?}", response.errors);
        let error = &response.errors[0];
        assert!(
            matches!(
                error.source::<ForumError>(),
                Some(ForumError::Validation(_))
            ),
            "{field}: {error:?}"
        );
        let extensions = error.extensions.as_ref().expect("typed validation failure");
        assert_eq!(
            extensions.get("code"),
            Some(&Value::from("FORUM_VALIDATION_FAILED")),
            "{field}"
        );
        assert_eq!(
            extensions.get("retryable"),
            Some(&Value::from(false)),
            "{field}"
        );
        assert_eq!(extensions.get("retryAfter"), None, "{field}");
    }
}

#[tokio::test]
async fn graphql_malformed_route_identity_still_returns_null_under_aliases() {
    let (schema, _) = test_schema(SettingsMode::DefaultOnly).await;
    // These identities normalize to absence before Taxonomy or source reads.
    // This does not prove successful resolution/visibility of a real route.
    for field in [
        "forumStorefrontCategoryRoute(locale: \"en\", slug: \"!!!\") { disposition }",
        "forumStorefrontTopicRoute(locale: \"en\", shortId: \"invalid\", slug: \"topic\") { disposition }",
        "forumStorefrontTopicRouteDecision(locale: \"en\", shortId: \"invalid\", slug: \"topic\") { disposition }",
    ] {
        let response = schema.execute(format!("{{ result: {field} }}")).await;
        assert!(response.errors.is_empty(), "{field}: {:?}", response.errors);
        assert_eq!(
            response.data.into_json().unwrap(),
            serde_json::json!({ "result": null }),
            "{field}"
        );
    }
}
