use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::RichTextDocument;
use rustok_core::SecurityContext;
use rustok_profiles::{
    ProfileError, ProfileRecord, ProfileResult, ProfileStatus, ProfileSummary, ProfileVisibility,
    ProfilesReader,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectOptions, ConnectionTrait, Database,
    DatabaseConnection, EntityTrait, IntoActiveModel, PaginatorTrait, QueryFilter,
    TransactionTrait,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

use crate::dto::{ForumQuoteReferenceInput, ForumQuoteTargetKindInput, SetForumQuotesInput};
use crate::entities::{
    forum_relation_revision, forum_reply, forum_reply_body, forum_topic, forum_topic_translation,
    forum_user_mention,
};
use crate::mentions::{ForumContentTarget, ForumQuoteReference};
use crate::state_machine::{ReplyStatus, TopicStatus};

use super::{ForumQuoteCommandService, mention_relation::MentionRelationService};

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

    async fn find_profile_records_by_handles(
        &self,
        tenant_id: Uuid,
        handles: &[String],
        _requested_locale: Option<&str>,
        _tenant_default_locale: Option<&str>,
    ) -> ProfileResult<HashMap<String, ProfileRecord>> {
        let mut result = HashMap::new();
        for handle in handles {
            if let Some(record) = self.records.get(&(tenant_id, handle.clone())) {
                result.insert(handle.clone(), record.clone());
            }
        }
        Ok(result)
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
    for migration in crate::migrations::relation_migrations() {
        migration
            .up(&manager)
            .await
            .expect("mention relation migration should apply");
    }
}

async fn insert_topic_source(db: &DatabaseConnection, tenant_id: Uuid, topic_id: Uuid, body: &str) {
    let body = stored_document(body);
    let category_id = Uuid::new_v4();
    let now = Utc::now().into();
    forum_topic::ActiveModel {
        id: Set(topic_id),
        tenant_id: Set(tenant_id),
        category_id: Set(category_id),
        author_id: Set(None),
        status: Set(TopicStatus::Open),
        metadata: Set(serde_json::json!({})),
        is_pinned: Set(false),
        is_locked: Set(false),
        reply_count: Set(0),
        created_at: Set(now),
        updated_at: Set(now),
        last_reply_at: Set(None),
    }
    .insert(db)
    .await
    .expect("topic source should be inserted");
    forum_topic_translation::ActiveModel {
        id: Set(Uuid::new_v4()),
        topic_id: Set(topic_id),
        tenant_id: Set(tenant_id),
        locale: Set("en".to_string()),
        title: Set("Topic".to_string()),
        slug: Set(None),
        body: Set(body),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(db)
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
    let body = stored_document(body);
    let now = Utc::now().into();
    forum_reply::ActiveModel {
        id: Set(reply_id),
        tenant_id: Set(tenant_id),
        topic_id: Set(topic_id),
        author_id: Set(None),
        parent_reply_id: Set(None),
        status: Set(ReplyStatus::Approved),
        position: Set(1),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(db)
    .await
    .expect("reply source should be inserted");
    forum_reply_body::ActiveModel {
        id: Set(Uuid::new_v4()),
        reply_id: Set(reply_id),
        tenant_id: Set(tenant_id),
        locale: Set("en".to_string()),
        body: Set(body),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(db)
    .await
    .expect("reply body should be inserted");
}

async fn update_topic_body(db: &DatabaseConnection, tenant_id: Uuid, topic_id: Uuid, body: &str) {
    let body = stored_document(body);
    let model = forum_topic_translation::Entity::find()
        .filter(forum_topic_translation::Column::TenantId.eq(tenant_id))
        .filter(forum_topic_translation::Column::TopicId.eq(topic_id))
        .filter(forum_topic_translation::Column::Locale.eq("en"))
        .one(db)
        .await
        .expect("topic body query should succeed")
        .expect("topic body should exist");
    let mut active = model.into_active_model();
    active.body = Set(body);
    active.updated_at = Set(Utc::now().into());
    active.update(db).await.expect("topic body should update");
}

fn document(body: &str) -> RichTextDocument {
    RichTextDocument::single_paragraph(body)
}

fn stored_document(body: &str) -> String {
    crate::richtext::serialize_discussion(document(body)).expect("test document should serialize")
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

    assert_eq!(
        relation_revision_count(&db, tenant_id).await,
        2,
        "bodies written before the relation stream existed receive one baseline identity each"
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
            &document("Hello @alice"),
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
            &document("Hello @alice"),
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
            &document("Hello @alice and @bob"),
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
            &document("Quoted reply"),
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
            &document("Foreign quote"),
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
        .execute_raw(sea_orm::Statement::from_sql_and_values(
            sea_orm::DbBackend::Sqlite,
            "UPDATE forum_user_mentions
             SET handle_snapshot = 'mallory'
             WHERE tenant_id = ? AND source_revision_id = ?",
            [tenant_id.into(), first.source().revision_id().into()],
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
async fn new_source_rows_must_receive_one_legacy_relation_identity_before_forum_12b2() {
    let db = setup_db().await;
    apply_relation_migrations(&db).await;
    let tenant_id = Uuid::new_v4();
    let topic_id = Uuid::new_v4();
    let reply_id = Uuid::new_v4();

    // Both tables carry an AFTER INSERT seed, so a writer that knows nothing about relations - an
    // import, a fork, a split, a translation apply - still leaves a readable revision behind.
    // new source rows must receive one legacy relation identity before FORUM-12B2.
    insert_topic_source(&db, tenant_id, topic_id, "Imported topic").await;
    insert_reply_source(&db, tenant_id, topic_id, reply_id, "Imported reply").await;

    let topic_revisions = forum_relation_revision::Entity::find()
        .filter(forum_relation_revision::Column::TenantId.eq(tenant_id))
        .filter(forum_relation_revision::Column::TargetKind.eq("topic"))
        .filter(forum_relation_revision::Column::TargetId.eq(topic_id))
        .all(&db)
        .await
        .expect("topic revision query should succeed");
    assert_eq!(
        topic_revisions.len(),
        1,
        "a source INSERT must seed exactly one baseline revision"
    );
    assert_eq!(topic_revisions[0].locale, "en");
    assert_eq!(
        topic_revisions[0].projection_fingerprint, "legacy",
        "a baseline revision must not claim a computed projection fingerprint"
    );

    let reply_revisions = forum_relation_revision::Entity::find()
        .filter(forum_relation_revision::Column::TenantId.eq(tenant_id))
        .filter(forum_relation_revision::Column::TargetKind.eq("reply"))
        .filter(forum_relation_revision::Column::TargetId.eq(reply_id))
        .all(&db)
        .await
        .expect("reply revision query should succeed");
    assert_eq!(reply_revisions.len(), 1);
    assert_eq!(reply_revisions[0].projection_fingerprint, "legacy");

    // The baseline is the whole revision: nothing claims a mention that was never resolved.
    assert_eq!(
        forum_user_mention::Entity::find()
            .filter(forum_user_mention::Column::TenantId.eq(tenant_id))
            .count(&db)
            .await
            .expect("mention count should load"),
        0
    );

    // A relation-aware writer appends its own projection revision on top of the baseline, so the
    // read path keeps returning the resolved snapshot as the latest revision.
    let reader = FakeProfilesReader {
        records: [(
            (tenant_id, "alice".to_string()),
            profile(tenant_id, Uuid::new_v4(), "alice"),
        )]
        .into_iter()
        .collect(),
    };
    let service = MentionRelationService::with_profiles(Arc::new(reader));
    let prepared = service
        .prepare(
            tenant_id,
            ForumContentTarget::topic(topic_id),
            "en",
            &document("Imported topic"),
            &SecurityContext::system(),
            [],
        )
        .await
        .expect("projection over a seeded baseline should prepare");
    let txn = db.begin().await.expect("transaction should begin");
    service
        .persist_in_tx(&txn, prepared)
        .await
        .expect("projection over a seeded baseline should persist");
    txn.commit().await.expect("transaction should commit");
    assert_eq!(
        relation_revision_count(&db, tenant_id).await,
        3,
        "the projection appends to the seeded baseline instead of replacing it"
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
            &document("Quote source"),
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

struct QuoteEditSettings {
    tenant_id: Uuid,
    minutes: u32,
    unavailable: bool,
}

#[async_trait]
impl rustok_api::StaticModuleSettingsReader for QuoteEditSettings {
    async fn settings(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<Option<rustok_api::StaticModuleSettingsSnapshot>, rustok_api::PortError> {
        assert_eq!(tenant_id, self.tenant_id);
        assert_eq!(module_slug, "forum");
        if self.unavailable {
            return Err(rustok_api::PortError::unavailable(
                "settings_down",
                "settings unavailable",
            ));
        }
        Ok(Some(rustok_api::StaticModuleSettingsSnapshot {
            enabled: Some(true),
            settings: serde_json::json!({ "max_edit_window_minutes": self.minutes }),
        }))
    }
}

#[tokio::test]
async fn standalone_quote_commands_obey_the_author_edit_window_for_topics_and_replies() {
    use rustok_api::{Permission, SharedStaticModuleSettingsReader};
    use rustok_core::UserRole;

    let db = setup_db().await;
    let tenant_id = Uuid::new_v4();
    let topic_id = Uuid::new_v4();
    let reply_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    insert_topic_source(&db, tenant_id, topic_id, "Topic").await;
    insert_reply_source(&db, tenant_id, topic_id, reply_id, "Reply").await;
    apply_relation_migrations(&db).await;
    let created_at = (Utc::now() - chrono::Duration::minutes(30)).fixed_offset();
    let mut topic = forum_topic::Entity::find_by_id(topic_id)
        .one(&db)
        .await
        .unwrap()
        .unwrap()
        .into_active_model();
    topic.author_id = Set(Some(author_id));
    topic.created_at = Set(created_at);
    topic.update(&db).await.unwrap();
    let mut reply = forum_reply::Entity::find_by_id(reply_id)
        .one(&db)
        .await
        .unwrap()
        .unwrap()
        .into_active_model();
    reply.author_id = Set(Some(author_id));
    reply.created_at = Set(created_at);
    reply.update(&db).await.unwrap();

    for (minutes, unavailable, staff, allowed) in [
        (10, false, false, false), // Expired author window.
        (60, false, false, true),  // Author still inside the window.
        (0, false, false, true),   // Explicit unlimited policy.
        (10, false, true, true),   // Moderator is also the author.
        (0, true, false, false),   // Reader failures must not grant permission.
    ] {
        let security = if staff {
            SecurityContext::new(UserRole::Manager, Some(author_id))
        } else {
            SecurityContext::from_permissions(
                UserRole::Customer,
                Some(author_id),
                [
                    Permission::FORUM_TOPICS_UPDATE,
                    Permission::FORUM_REPLIES_UPDATE,
                ],
            )
        };
        let command = ForumQuoteCommandService::new(db.clone()).with_settings_providers(
            super::engagement_mode::ForumSettingsProviders::default().with_static_reader(
                SharedStaticModuleSettingsReader(Arc::new(QuoteEditSettings {
                    tenant_id,
                    minutes,
                    unavailable,
                })),
            ),
        );
        for is_reply in [false, true] {
            let before = relation_revision_count(&db, tenant_id).await;
            let input = SetForumQuotesInput {
                locale: "en".into(),
                quotes: Vec::new(),
            };
            let result = if is_reply {
                command
                    .set_reply_quotes(tenant_id, reply_id, security.clone(), input)
                    .await
            } else {
                command
                    .set_topic_quotes(tenant_id, topic_id, security.clone(), input)
                    .await
            };
            assert_eq!(
                result.is_ok(),
                allowed,
                "minutes={minutes}, unavailable={unavailable}, staff={staff}, reply={is_reply}: {result:?}"
            );
            if !allowed {
                assert_eq!(relation_revision_count(&db, tenant_id).await, before);
                if !unavailable {
                    assert_eq!(result.unwrap_err().stable_code(), "FORUM_VALIDATION_FAILED");
                }
            }
        }
    }
}
