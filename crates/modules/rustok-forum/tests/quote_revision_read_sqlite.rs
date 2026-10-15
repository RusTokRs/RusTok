use async_graphql::{EmptyMutation, EmptySubscription, Schema};
use rustok_forum::{ForumContentTarget, ForumQuery, RevisionService};
use sea_orm::{ConnectOptions, ConnectionTrait, Database};
use uuid::Uuid;

#[test]
fn quote_revision_fields_are_distinct_from_content_version_fields() {
    let schema = Schema::build(ForumQuery::default(), EmptyMutation, EmptySubscription).finish();
    let sdl = schema.sdl();
    for field in [
        "forumStorefrontTopicQuoteRevision(",
        "forumStorefrontReplyQuoteRevision(",
        "forumStorefrontTopicCurrentRevision(",
        "forumStorefrontReplyCurrentRevision(",
    ] {
        assert!(sdl.contains(field), "missing revision contract {field}");
    }
}

#[tokio::test]
async fn quote_revision_uses_global_relation_primary_key_scoped_to_exact_target_and_locale() {
    let mut options = ConnectOptions::new(format!(
        "sqlite:file:forum_quote_revision_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    ));
    options.max_connections(1).min_connections(1).sqlx_logging(false);
    let db = Database::connect(options).await.expect("sqlite");
    db.execute_unprepared(
        "CREATE TABLE forum_relation_revisions (\
            revision_id INTEGER PRIMARY KEY, tenant_id TEXT NOT NULL, target_kind TEXT NOT NULL, \
            target_id TEXT NOT NULL, locale TEXT NOT NULL)",
    )
    .await
    .expect("relation fixture table");
    db.execute_unprepared(
        "CREATE TABLE forum_topic_revisions (\
            id INTEGER PRIMARY KEY, tenant_id TEXT NOT NULL, topic_id TEXT NOT NULL)",
    )
    .await
    .expect("content fixture table");

    let tenant = Uuid::new_v4();
    let foreign_tenant = Uuid::new_v4();
    let topic_id = Uuid::new_v4();
    let reply_id = Uuid::new_v4();
    let other_topic_id = Uuid::new_v4();
    for (revision, scope_tenant, kind, target, locale) in [
        (1, tenant, "topic", other_topic_id, "en"),
        (12, foreign_tenant, "topic", topic_id, "en"),
        (13, tenant, "reply", reply_id, "en"),
        (40, tenant, "topic", topic_id, "en"),
        (41, tenant, "topic", topic_id, "ru"),
        (42, tenant, "reply", topic_id, "en"),
    ] {
        db.execute_unprepared(&format!(
            "INSERT INTO forum_relation_revisions \
             (revision_id, tenant_id, target_kind, target_id, locale) \
             VALUES ({revision}, '{scope_tenant}', '{kind}', '{target}', '{locale}')"
        ))
        .await
        .expect("relation fixture");
    }

    let service = RevisionService::new(db);
    assert_eq!(
        service.current_quote_revision(tenant, ForumContentTarget::topic(topic_id), "en").await.unwrap(),
        Some(40)
    );
    assert_eq!(
        service.current_quote_revision(tenant, ForumContentTarget::topic(topic_id), "ru").await.unwrap(),
        Some(41)
    );
    assert_eq!(
        service.current_quote_revision(tenant, ForumContentTarget::reply(reply_id), "en").await.unwrap(),
        Some(13)
    );
    assert_eq!(
        service.current_quote_revision(tenant, ForumContentTarget::reply(reply_id), "ru").await.unwrap(),
        None
    );
    // Reactions' next content version is not the quote relation primary key.
    assert_eq!(service.current_topic_revision(tenant, topic_id).await.unwrap(), 1);
}
