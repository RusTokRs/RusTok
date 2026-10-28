use std::sync::Arc;

use async_graphql::{EmptySubscription, Schema};
use rustok_api::{AuthContext, Permission, TenantContext};
use rustok_forum::{ForumMutation, ForumQuery};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::{ConnectOptions, ConnectionTrait, Database};
use uuid::Uuid;

#[tokio::test]
async fn stats_and_vote_clears_reject_foreign_tenants_before_read_or_write() {
    let mut options = ConnectOptions::new(format!(
        "sqlite:file:forum_graphql_tenant_scope_{}?mode=memory&cache=shared",
        Uuid::new_v4(),
    ));
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await.expect("sqlite");
    db.execute_unprepared(
        "CREATE TABLE tenant_modules (tenant_id TEXT, module_slug TEXT, enabled INTEGER)",
    )
    .await
    .expect("module table");
    db.execute_unprepared("CREATE TABLE forum_user_stats (\
        tenant_id TEXT NOT NULL, user_id TEXT NOT NULL, \
        topic_count INTEGER NOT NULL, reply_count INTEGER NOT NULL, solution_count INTEGER NOT NULL, \
        created_at TEXT NOT NULL, updated_at TEXT NOT NULL, \
        PRIMARY KEY (tenant_id, user_id))")
        .await.expect("stats table");

    let tenant_id = Uuid::new_v4();
    let foreign_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    db.execute_unprepared(&format!(
        "INSERT INTO tenant_modules VALUES ('{tenant_id}', 'forum', 1)"
    ))
    .await
    .expect("enabled request tenant");
    for (id, count) in [(tenant_id, 3), (foreign_id, 77)] {
        db.execute_unprepared(&format!(
            "INSERT INTO forum_user_stats VALUES (\
                '{id}', '{user_id}', {count}, 0, 0, \
                '2026-10-15T00:00:00+00:00', '2026-10-15T00:00:00+00:00')"
        ))
        .await
        .expect("stats row");
    }

    let bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let schema = Schema::build(
        ForumQuery::default(),
        ForumMutation::default(),
        EmptySubscription,
    )
    .data(db)
    .data(bus)
    .data(TenantContext {
        id: tenant_id,
        name: "Tenant A".into(),
        slug: "a".into(),
        domain: None,
        settings: serde_json::json!({}),
        default_locale: "en".into(),
        is_active: true,
    })
    .data(AuthContext {
        user_id,
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions: vec![
            Permission::FORUM_TOPICS_READ,
            Permission::FORUM_REPLIES_READ,
        ],
        client_id: None,
        scopes: Vec::new(),
        grant_type: "user".into(),
    })
    .finish();

    // These commands acknowledge completion rather than materializing a
    // possibly hidden object after the owner write has already committed.
    let sdl = schema.sdl();
    for name in [
        "setForumTopicVote",
        "clearForumTopicVote",
        "setForumReplyVote",
        "clearForumReplyVote",
        "markForumTopicSolution",
        "clearForumTopicSolution",
    ] {
        let signature = sdl
            .lines()
            .find(|line| line.trim_start().starts_with(&format!("{name}(")))
            .unwrap_or_else(|| panic!("{name} missing from GraphQL mutation contract"));
        assert!(
            signature.trim_end().ends_with(": Boolean!"),
            "{name} must return a non-null acknowledgement, found {signature}"
        );
    }

    let own = schema.execute(format!(
        "{{ forumUserStats(tenantId: \"{tenant_id}\", userId: \"{user_id}\") {{ topicCount }} }}"
    )).await;
    assert!(own.errors.is_empty(), "own stats errors: {:?}", own.errors);
    assert_eq!(
        own.data.into_json().unwrap()["forumUserStats"]["topicCount"],
        3
    );

    let foreign_stats = schema.execute(format!(
        "{{ forumUserStats(tenantId: \"{foreign_id}\", userId: \"{user_id}\") {{ topicCount }} }}"
    )).await;
    assert_eq!(foreign_stats.data, async_graphql::Value::Null);
    assert_eq!(foreign_stats.errors.len(), 1);
    assert!(
        foreign_stats.errors[0]
            .message
            .contains("tenant scope mismatch")
    );

    // No Forum vote, topic, reply or settings tables exist in this fixture.
    // A foreign-tenant clear must be rejected before consulting any of them.
    for (mutation, target) in [
        ("clearForumTopicVote", "topicId"),
        ("clearForumReplyVote", "replyId"),
    ] {
        let response = schema
            .execute(format!(
                "mutation {{ {mutation}(tenantId: \"{foreign_id}\", {target}: \"{}\") }}",
                Uuid::new_v4(),
            ))
            .await;
        assert_eq!(response.data, async_graphql::Value::Null, "{mutation}");
        assert_eq!(
            response.errors.len(),
            1,
            "{mutation}: {:?}",
            response.errors
        );
        assert!(
            response.errors[0].message.contains("tenant scope mismatch"),
            "{mutation}: {:?}",
            response.errors
        );
    }
}
