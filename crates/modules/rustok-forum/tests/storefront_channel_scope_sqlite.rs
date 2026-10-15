use async_graphql::{EmptyMutation, EmptySubscription, Request, Schema};
use rustok_api::{AuthContext, Permission, RequestContext, TenantContext};
use rustok_forum::ForumQuery;
use sea_orm::{ConnectOptions, ConnectionTrait, Database};
use uuid::Uuid;

#[tokio::test]
async fn disabled_or_foreign_channel_cannot_disclose_storefront_pages_to_any_viewer() {
    // A single SQLite connection and one statement per call make the fixture
    // deterministic across SQLx versions. There are deliberately no Forum
    // content tables: failing the channel gate must precede all owner reads.
    let mut options = ConnectOptions::new(format!(
        "sqlite:file:forum_storefront_channel_scope_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    ));
    options.max_connections(1).min_connections(1).sqlx_logging(false);
    let db = Database::connect(options).await.expect("sqlite");
    let tenant = Uuid::new_v4();
    let foreign = Uuid::new_v4();
    let channel = Uuid::new_v4();
    let foreign_channel = Uuid::new_v4();
    let binding = Uuid::new_v4();

    for statement in [
        "CREATE TABLE tenant_modules (tenant_id TEXT, module_slug TEXT, enabled INTEGER)",
        "CREATE TABLE channels (\
            id TEXT PRIMARY KEY, tenant_id TEXT, slug TEXT, name TEXT, \
            is_active INTEGER, is_default INTEGER, status TEXT, settings TEXT, \
            created_at TEXT, updated_at TEXT)",
        "CREATE TABLE channel_module_bindings (\
            id TEXT PRIMARY KEY, channel_id TEXT, module_slug TEXT, is_enabled INTEGER, \
            settings TEXT, created_at TEXT, updated_at TEXT)",
    ] {
        db.execute_unprepared(statement).await.expect("channel policy table");
    }
    for statement in [
        format!("INSERT INTO tenant_modules VALUES ('{tenant}', 'forum', 1)"),
        format!("INSERT INTO channels VALUES (\
            '{channel}', '{tenant}', 'web', 'Web', 1, 1, 'active', '{{}}', \
            '2026-10-15T00:00:00+00:00', '2026-10-15T00:00:00+00:00')"),
        format!("INSERT INTO channels VALUES (\
            '{foreign_channel}', '{foreign}', 'foreign', 'Other', 1, 1, 'active', '{{}}', \
            '2026-10-15T00:00:00+00:00', '2026-10-15T00:00:00+00:00')"),
        format!("INSERT INTO channel_module_bindings VALUES (\
            '{binding}', '{channel}', 'forum', 0, '{{}}', \
            '2026-10-15T00:00:00+00:00', '2026-10-15T00:00:00+00:00')"),
    ] {
        db.execute_unprepared(&statement).await.expect("channel policy row");
    }

    let schema = Schema::build(ForumQuery::default(), EmptyMutation, EmptySubscription)
        .data(db)
        .data(TenantContext {
            id: tenant, name: "Tenant".into(), slug: "tenant".into(),
            domain: None, settings: serde_json::json!({}), default_locale: "en".into(),
            is_active: true,
        }).finish();
    let request_context = |channel_id, user_id| RequestContext {
        tenant_id: tenant, user_id, channel_id: Some(channel_id),
        channel_slug: Some("web".into()), channel_resolution_source: None,
        locale: "en".into(), correlation_id: Uuid::new_v4().to_string(),
    };
    let query = "{ forumStorefrontAudienceTopics(locale: \"en\", perPage: 1) { items { id } nextCursor } }";

    let guest = schema.execute(Request::new(query).data(request_context(channel, None))).await;
    assert_eq!(guest.data, async_graphql::Value::Null);
    assert_eq!(guest.errors.len(), 1);
    assert!(guest.errors[0].message.contains("not enabled for this channel"));

    let auth = AuthContext {
        user_id: Uuid::new_v4(), session_id: Uuid::new_v4(), tenant_id: tenant,
        permissions: vec![Permission::FORUM_TOPICS_LIST, Permission::FORUM_TOPICS_READ],
        client_id: None, scopes: Vec::new(), grant_type: "user".into(),
    };
    let signed_in = schema.execute(Request::new(query)
        .data(request_context(channel, Some(auth.user_id))).data(auth.clone())).await;
    assert_eq!(signed_in.data, async_graphql::Value::Null);
    assert_eq!(signed_in.errors.len(), 1);
    assert!(signed_in.errors[0].message.contains("not enabled for this channel"));

    let unread = schema.execute(Request::new(
        "{ forumStorefrontUnreadTopics(locale: \"en\", limit: 1) { items { id } nextCursor } }"
    ).data(request_context(channel, Some(auth.user_id))).data(auth.clone())).await;
    assert_eq!(unread.data, async_graphql::Value::Null);
    assert_eq!(unread.errors.len(), 1);
    assert!(unread.errors[0].message.contains("not enabled for this channel"));

    // Neither a channel owned by another tenant nor an unknown channel may
    // bypass or identify the local Forum. Both receive the same denial as a
    // disabled local binding, without attempting to read Forum rows.
    for other_channel in [foreign_channel, Uuid::new_v4()] {
        let rejected = schema.execute(Request::new(query)
            .data(request_context(other_channel, Some(auth.user_id))).data(auth.clone())).await;
        assert_eq!(rejected.data, async_graphql::Value::Null);
        assert_eq!(rejected.errors.len(), 1);
        assert_eq!(rejected.errors[0].message, signed_in.errors[0].message);
    }
}
