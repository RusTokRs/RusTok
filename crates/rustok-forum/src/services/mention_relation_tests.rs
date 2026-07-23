use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use rustok_core::SecurityContext;
use rustok_profiles::{
    ProfileError, ProfileRecord, ProfileResult, ProfileStatus, ProfileSummary, ProfileVisibility,
    ProfilesReader,
};
use sea_orm::{
    ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, TransactionTrait,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

use crate::dto::{ForumQuoteReferenceInput, ForumQuoteTargetKindInput, SetForumQuotesInput};
use crate::entities::forum_relation_revision;
use crate::mentions::{ForumContentTarget, ForumQuoteReference};

use super::{ForumQuoteCommandService, MentionRelationService};

struct FakeProfilesReader {
    records: HashMap<(Uuid, String), ProfileRecord>,
}

#[async_trait]
impl ProfilesReader for FakeProfilesReader {
    async fn find_profile_summary(
        &self,
        _tenant_id: Uuid,
        _user_id: Uuid,
        _requested_locale: Option<&str>,
        _tenant_default_locale: Option<&str>,
    ) -> ProfileResult<Option<ProfileSummary>> {
        unreachable!("mention persistence uses handle lookup")
    }

    async fn find_profile_summaries(
        &self,
        _tenant_id: Uuid,
        _user_ids: &[Uuid],
        _requested_locale: Option<&str>,
        _tenant_default_locale: Option<&str>,
    ) -> ProfileResult<HashMap<Uuid, ProfileSummary>> {
        unreachable!("mention persistence uses handle lookup")
    }

    async fn get_profile_by_handle(
        &self,
        tenant_id: Uuid,
        handle: &str,
        _requested_locale: Option<&str>,
        _tenant_default_locale: Option<&str>,
    ) -> ProfileResult<ProfileRecord> {
        self.records
            .get(&(tenant_id, handle.to_string()))
            .cloned()
            .ok_or_else(|| ProfileError::ProfileByHandleNotFound(handle.to_string()))
    }
}

fn profile(tenant_id: Uuid, user_id: Uuid, handle: &str) -> ProfileRecord {
    ProfileRecord {
        tenant_id,
        user_id,
        handle: handle.to_string(),
        display_name: handle.to_string(),
        bio: None,
        tags: Vec::new(),
        avatar_media_id: None,
        banner_media_id: None,
        preferred_locale: Some("en".to_string()),
        visibility: ProfileVisibility::Public,
        status: ProfileStatus::Active,
    }
}

async fn setup_db() -> DatabaseConnection {
    let mut options = ConnectOptions::new(format!(
        "sqlite:file:forum_mentions_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    ));
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("SQLite mention database should connect");
    db.execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .expect("SQLite foreign keys should be enabled");
    for statement in [
        r#"CREATE TABLE forum_topics (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            category_id TEXT NOT NULL,
            author_id TEXT,
            status TEXT NOT NULL,
            metadata TEXT NOT NULL,
            is_pinned INTEGER NOT NULL,
            is_locked INTEGER NOT NULL,
            reply_count INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            last_reply_at TEXT,
            deleted_at TEXT
        )"#,
        r#"CREATE TABLE forum_topic_translations (
            id TEXT PRIMARY KEY,
            topic_id TEXT NOT NULL,
            tenant_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            title TEXT NOT NULL,
            slug TEXT,
            body TEXT NOT NULL,
            body_format TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )"#,
        r#"CREATE TABLE forum_replies (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            topic_id TEXT NOT NULL,
            author_id TEXT,
            parent_reply_id TEXT,
            status TEXT NOT NULL,
            position INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            deleted_at TEXT
        )"#,
        r#"CREATE TABLE forum_reply_bodies (
            id TEXT PRIMARY KEY,
            reply_id TEXT NOT NULL,
            tenant_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            body TEXT NOT NULL,
            body_format TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )"#,
        r#"CREATE TABLE forum_domain_events (
            sequence_no INTEGER PRIMARY KEY AUTOINCREMENT,
            event_id TEXT NOT NULL UNIQUE,
            tenant_id TEXT NOT NULL,
            aggregate_type TEXT NOT NULL,
            aggregate_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            schema_version INTEGER NOT NULL DEFAULT 1,
            actor_id TEXT,
            payload TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )"#,
    ] {
        db.execute_unprepared(statement)
            .await
            .expect("source table should be created");
    }
    db
}

async fn apply_relation_migrations(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
    let mut migrations = crate::migrations::migrations();
    let relation_migrations = migrations.split_off(
        migrations
            .len()
            .checked_sub(3)
            .expect("three mention relation migrations should be registered"),
    );
    for migration in relation_migrations {
        migration
            .up(&manager)
            .await
            .expect("mention relation migration should apply");
    }
}

async fn insert_topic_source(db: &DatabaseConnection, tenant_id: Uuid, topic_id: Uuid, body: &str) {
    let category_id = Uuid::new_v4();
    db.execute_unprepared(&format!(
        "INSERT INTO forum_topics (
            id, tenant_id, category_id, author_id, status, metadata,
            is_pinned, is_locked, reply_count, created_at, updated_at, last_reply_at
        ) VALUES (
            '{topic_id}', '{tenant_id}', '{category_id}', NULL, 'open', '{{}}',
            0, 0, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, NULL
        )"
    ))
    .await
    .expect("topic source should be inserted");
    db.execute_unprepared(&format!(
        "INSERT INTO forum_topic_translations (
            id, topic_id, tenant_id, locale, title, slug, body, body_format,
            created_at, updated_at
        ) VALUES (
            '{}', '{topic_id}', '{tenant_id}', 'en', 'Topic', NULL, '{}',
            'markdown', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
        )",
        Uuid::new_v4(),
        body.replace('\'', "''")
    ))
    .await
    .expect("topic translation should be inserted");
}

async fn insert_reply_source(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
    reply_id: Uuid,
    body: &str,
) {
    db.execute_unprepared(&format!(
        "INSERT INTO forum_replies (
            id, tenant_id, topic_id, author_id, parent_reply_id, status, position,
            created_at, updated_at
        ) VALUES (
            '{reply_id}', '{tenant_id}', '{topic_id}', NULL, NULL, 'approved', 1,
            CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
        )"
    ))
    .await
    .expect("reply source should be inserted");
    db.execute_unprepared(&format!(
        "INSERT INTO forum_reply_bodies (
            id, reply_id, tenant_id, locale, body, body_format, created_at, updated_at
        ) VALUES (
            '{}', '{reply_id}', '{tenant_id}', 'en', '{}', 'markdown',
            CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
        )",
        Uuid::new_v4(),
        body.replace('\'', "''")
    ))
    .await
    .expect("reply body should be inserted");
}

async fn update_topic_body(db: &DatabaseConnection, tenant_id: Uuid, topic_id: Uuid, body: &str) {
    db.execute_unprepared(&format!(
        "UPDATE forum_topic_translations
         SET body = '{}', updated_at = CURRENT_TIMESTAMP
         WHERE tenant_id = '{tenant_id}' AND topic_id = '{topic_id}' AND locale = 'en'",
        body.replace('\'', "''")
    ))
    .await
    .expect("topic body should update");
}

async fn relation_revision_count(db: &DatabaseConnection, tenant_id: Uuid) -> u64 {
    forum_relation_revision::Entity::find()
        .filter(forum_relation_revision::Column::TenantId.eq(tenant_id))
        .count(db)
        .await
        .expect("relation revision count should load")
}

#[tokio::test]
async fn relation_revision_replay_diff_quotes_and_guards_are_atomic() {
    let db = setup_db().await;
    let tenant_id = Uuid::new_v4();
    let other_tenant_id = Uuid::new_v4();
    let topic_id = Uuid::new_v4();
    let reply_id = Uuid::new_v4();
    let other_topic_id = Uuid::new_v4();
    insert_topic_source(&db, tenant_id, topic_id, "Hello @alice").await;
    insert_reply_source(&db, tenant_id, topic_id, reply_id, "Quoted reply").await;
    insert_topic_source(&db, other_tenant_id, other_topic_id, "Foreign quote").await;
    apply_relation_migrations(&db).await;

    let before_seed = relation_revision_count(&db, tenant_id).await;
    let seeded_topic_id = Uuid::new_v4();
    insert_topic_source(&db, tenant_id, seeded_topic_id, "Created after migration").await;
    assert_eq!(
        relation_revision_count(&db, tenant_id).await,
        before_seed + 1,
        "new source rows must receive one legacy relation identity before FORUM-12B2"
    );

    let alice_id = Uuid::new_v4();
    let bob_id = Uuid::new_v4();
    let reader = FakeProfilesReader {
        records: [
            (
                (tenant_id, "alice".to_string()),
                profile(tenant_id, alice_id, "alice"),
            ),
            (
                (tenant_id, "bob".to_string()),
                profile(tenant_id, bob_id, "bob"),
            ),
        ]
        .into_iter()
        .collect(),
    };
    let service = MentionRelationService::with_profiles(Arc::new(reader));
    let security = SecurityContext::system();

    let first_prepared = service
        .prepare(
            tenant_id,
            ForumContentTarget::topic(topic_id),
            "en",
            "Hello @alice",
            "markdown",
            &security,
            [],
        )
        .await
        .expect("first mention projection should prepare");
    let txn = db.begin().await.expect("first transaction should begin");
    let first = service
        .persist_in_tx(&txn, first_prepared)
        .await
        .expect("first mention projection should persist");
    txn.commit().await.expect("first transaction should commit");
    assert!(!first.replayed());
    assert_eq!(first.added_user_ids(), &[alice_id]);
    assert_eq!(first.mention_count(), 1);

    let replay_prepared = service
        .prepare(
            tenant_id,
            ForumContentTarget::topic(topic_id),
            "en",
            "Hello @alice",
            "markdown",
            &security,
            [],
        )
        .await
        .expect("replay should prepare");
    let txn = db.begin().await.expect("replay transaction should begin");
    let replay = service
        .persist_in_tx(&txn, replay_prepared)
        .await
        .expect("identical replay should persist idempotently");
    txn.commit()
        .await
        .expect("replay transaction should commit");
    assert!(replay.replayed());
    assert_eq!(replay.source().revision_id(), first.source().revision_id());
    assert!(replay.added_user_ids().is_empty());

    update_topic_body(&db, tenant_id, topic_id, "Hello @alice and @bob").await;
    let changed_prepared = service
        .prepare(
            tenant_id,
            ForumContentTarget::topic(topic_id),
            "en",
            "Hello @alice and @bob",
            "markdown",
            &security,
            [],
        )
        .await
        .expect("changed projection should prepare");
    let txn = db.begin().await.expect("changed transaction should begin");
    let changed = service
        .persist_in_tx(&txn, changed_prepared)
        .await
        .expect("changed projection should persist");
    txn.commit()
        .await
        .expect("changed transaction should commit");
    assert!(!changed.replayed());
    assert_eq!(changed.added_user_ids(), &[bob_id]);
    assert!(changed.source().revision_id() > first.source().revision_id());

    let quote = ForumQuoteReference::new(
        ForumContentTarget::topic(topic_id),
        changed.source().revision_id(),
    )
    .expect("quote reference should be valid");
    let quote_prepared = service
        .prepare(
            tenant_id,
            ForumContentTarget::reply(reply_id),
            "en",
            "Quoted reply",
            "markdown",
            &security,
            [quote.clone()],
        )
        .await
        .expect("quote projection should prepare");
    let txn = db.begin().await.expect("quote transaction should begin");
    let quoted = service
        .persist_in_tx(&txn, quote_prepared)
        .await
        .expect("quote projection should persist");
    txn.commit().await.expect("quote transaction should commit");
    assert_eq!(quoted.quote_count(), 1);

    let before_foreign = relation_revision_count(&db, other_tenant_id).await;
    let foreign_prepared = service
        .prepare(
            other_tenant_id,
            ForumContentTarget::topic(other_topic_id),
            "en",
            "Foreign quote",
            "markdown",
            &security,
            [quote],
        )
        .await
        .expect("foreign quote projection should prepare without profile lookup");
    let txn = db.begin().await.expect("foreign transaction should begin");
    let error = service
        .persist_in_tx(&txn, foreign_prepared)
        .await
        .expect_err("cross-tenant quote revision must fail closed");
    txn.commit()
        .await
        .expect("validation error transaction should contain no writes");
    assert_eq!(error.stable_code(), "FORUM_QUOTE_TARGET_UNAVAILABLE");
    assert_eq!(
        relation_revision_count(&db, other_tenant_id).await,
        before_foreign,
        "quote validation must run before the first relation write"
    );

    let immutable_error = db
        .execute_unprepared(&format!(
            "UPDATE forum_user_mentions
             SET handle_snapshot = 'mallory'
             WHERE tenant_id = '{tenant_id}'
               AND source_revision_id = {}",
            first.source().revision_id()
        ))
        .await
        .expect_err("persisted mention rows must be immutable");
    assert!(
        immutable_error
            .to_string()
            .contains("forum relation projections are immutable")
    );
}

#[tokio::test]
async fn quote_owner_replace_replay_clear_and_cross_tenant_rejection_are_atomic() {
    let db = setup_db().await;
    let tenant_id = Uuid::new_v4();
    let other_tenant_id = Uuid::new_v4();
    let topic_id = Uuid::new_v4();
    let reply_id = Uuid::new_v4();
    let other_topic_id = Uuid::new_v4();
    insert_topic_source(&db, tenant_id, topic_id, "Quote source").await;
    insert_reply_source(&db, tenant_id, topic_id, reply_id, "Quote consumer").await;
    insert_topic_source(&db, other_tenant_id, other_topic_id, "Foreign source").await;
    apply_relation_migrations(&db).await;

    let relation_service = MentionRelationService::with_profiles(Arc::new(FakeProfilesReader {
        records: HashMap::new(),
    }));
    let security = SecurityContext::system();
    let prepared = relation_service
        .prepare(
            tenant_id,
            ForumContentTarget::topic(topic_id),
            "en",
            "Quote source",
            "markdown",
            &security,
            [],
        )
        .await
        .expect("quoted source revision should prepare");
    let txn = db.begin().await.expect("source transaction should begin");
    let source_revision = relation_service
        .persist_in_tx(&txn, prepared)
        .await
        .expect("quoted source revision should persist");
    txn.commit()
        .await
        .expect("source transaction should commit");

    let quote_input = ForumQuoteReferenceInput {
        target_kind: ForumQuoteTargetKindInput::Topic,
        target_id: topic_id,
        revision_id: source_revision.source().revision_id(),
    };
    let command = ForumQuoteCommandService::new(db.clone());
    let replaced = command
        .set_reply_quotes(
            tenant_id,
            reply_id,
            security.clone(),
            SetForumQuotesInput {
                locale: "en".to_string(),
                quotes: vec![quote_input.clone()],
            },
        )
        .await
        .expect("quote replacement should commit");
    assert_eq!(replaced.quotes.len(), 1);
    assert_eq!(replaced.quotes[0].target_kind, "topic");
    assert_eq!(replaced.quotes[0].target_id, topic_id);
    assert_eq!(
        replaced.quotes[0].revision_id,
        source_revision.source().revision_id()
    );

    let replay = command
        .set_reply_quotes(
            tenant_id,
            reply_id,
            security.clone(),
            SetForumQuotesInput {
                locale: "en".to_string(),
                quotes: vec![quote_input.clone()],
            },
        )
        .await
        .expect("identical replacement should replay");
    assert_eq!(replay.revision_id, replaced.revision_id);

    let cleared = command
        .set_reply_quotes(
            tenant_id,
            reply_id,
            security.clone(),
            SetForumQuotesInput {
                locale: "en".to_string(),
                quotes: Vec::new(),
            },
        )
        .await
        .expect("explicit empty list should clear quotes");
    assert!(cleared.quotes.is_empty());
    assert!(cleared.revision_id > replaced.revision_id);

    let before_foreign = relation_revision_count(&db, other_tenant_id).await;
    let error = command
        .set_topic_quotes(
            other_tenant_id,
            other_topic_id,
            security,
            SetForumQuotesInput {
                locale: "en".to_string(),
                quotes: vec![quote_input],
            },
        )
        .await
        .expect_err("cross-tenant quoted revision must fail closed");
    assert_eq!(error.stable_code(), "FORUM_QUOTE_TARGET_UNAVAILABLE");
    assert_eq!(
        relation_revision_count(&db, other_tenant_id).await,
        before_foreign,
        "failed quote replacement must not append a relation revision"
    );
}
