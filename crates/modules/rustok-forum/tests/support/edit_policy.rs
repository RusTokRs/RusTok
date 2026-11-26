//! Topic edit policy must observe the claimed source, not a pre-lock snapshot.
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use async_trait::async_trait;
use rustok_api::{
    PortError, SharedStaticModuleSettingsReader, SharedStaticModuleSettingsTransactionReader,
    StaticModuleSettingsReader, StaticModuleSettingsSnapshot,
    StaticModuleSettingsTransactionReader,
};
use rustok_core::{SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, ForumError, ForumSettingsProviders, TopicService, UpdateTopicInput,
    entities::{forum_topic, forum_topic_revision, forum_topic_translation},
};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

use super::{
    make_category, make_topic, posting_cooldown, reply_input, reply_restore_checkpoint, setup,
    support,
};

struct ClaimedTopicPolicy {
    tenant: Uuid,
    topic: Uuid,
    prior_updated_at: chrono::DateTime<chrono::FixedOffset>,
    reads: Arc<AtomicUsize>,
}

fn snapshot(minutes: u32) -> StaticModuleSettingsSnapshot {
    StaticModuleSettingsSnapshot {
        enabled: Some(true),
        settings: serde_json::json!({"max_edit_window_minutes": minutes}),
    }
}

#[async_trait]
impl StaticModuleSettingsReader for ClaimedTopicPolicy {
    async fn settings(
        &self,
        tenant: Uuid,
        slug: &str,
    ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
        assert_eq!((tenant, slug), (self.tenant, "forum"));
        Ok(Some(snapshot(60)))
    }
}

#[async_trait]
impl StaticModuleSettingsTransactionReader for ClaimedTopicPolicy {
    async fn settings_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant: Uuid,
        slug: &str,
    ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
        assert_eq!((tenant, slug), (self.tenant, "forum"));
        let topic = forum_topic::Entity::find_by_id(self.topic)
            .one(txn)
            .await
            .unwrap()
            .unwrap();
        assert!(
            topic.updated_at > self.prior_updated_at,
            "the source claim must precede the transactional policy/time check"
        );
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(Some(snapshot(10)))
    }
}

#[tokio::test]
async fn topic_edit_reads_policy_after_claim_on_sqlite() {
    let (db, events, _) = setup().await;
    for readers in [(true, true), (true, false), (false, true)] {
        exercise(db.clone(), events.clone(), readers).await;
    }
}

#[tokio::test]
async fn topic_edit_reads_policy_after_claim_on_postgres() -> support::TestResult<()> {
    let Some(database) = support::postgres::PostgresForumTestDb::setup("edit_policy_claim").await?
    else {
        return Ok(());
    };
    let events = TransactionalEventBus::new(Arc::new(OutboxTransport::new(database.db.clone())));
    for readers in [(true, true), (true, false), (false, true)] {
        exercise(database.db.clone(), events.clone(), readers).await;
    }
    database.cleanup().await?;
    Ok(())
}

async fn exercise(db: DatabaseConnection, events: TransactionalEventBus, readers: (bool, bool)) {
    let tenant = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let author = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let providers = posting_cooldown::zero_cooldown_providers();
    let topics =
        TopicService::new(db.clone(), events.clone()).with_settings_providers(providers.clone());
    let category = make_category(
        &CategoryService::new(db.clone()),
        tenant,
        &admin,
        "edit-claim",
        false,
    )
    .await;
    let topic = make_topic(&topics, tenant, &author, category, "edit-claim").await;
    let reply = rustok_forum::ReplyService::new(db.clone(), events.clone())
        .with_settings_providers(providers)
        .create(
            tenant,
            author.clone(),
            topic,
            reply_input("preserved reply", None),
        )
        .await
        .unwrap();
    let mut source: forum_topic::ActiveModel = forum_topic::Entity::find_by_id(topic)
        .one(&db)
        .await
        .unwrap()
        .unwrap()
        .into();
    source.created_at = Set((chrono::Utc::now() - chrono::Duration::minutes(30)).fixed_offset());
    let prior_updated_at =
        chrono::DateTime::parse_from_rfc3339("2020-01-01T00:00:00+00:00").unwrap();
    source.updated_at = Set(prior_updated_at);
    source.update(&db).await.unwrap();
    let reads = Arc::new(AtomicUsize::new(0));
    let reader = Arc::new(ClaimedTopicPolicy {
        tenant,
        topic,
        prior_updated_at,
        reads: reads.clone(),
    });
    let providers = ForumSettingsProviders::from_optional_readers(
        readers
            .0
            .then(|| SharedStaticModuleSettingsReader(reader.clone())),
        readers
            .1
            .then(|| SharedStaticModuleSettingsTransactionReader(reader)),
    );
    let topics = TopicService::new(db.clone(), events).with_settings_providers(providers);
    let before = checkpoint(&db, tenant, topic, reply.id).await;
    let result = topics
        .update(
            tenant,
            topic,
            author,
            UpdateTopicInput {
                locale: "en".into(),
                title: Some("must not commit".into()),
                body: None,
                channel_slugs: None,
                tags: None,
                metadata: None,
            },
        )
        .await;
    if !readers.0 {
        assert!(
            matches!(result, Err(ref error) if error.stable_code() == "FORUM_STATIC_SETTINGS_UNAVAILABLE"),
            "{result:?}"
        );
    } else if !readers.1 {
        assert!(
            matches!(result, Err(ref error) if error.stable_code() == "FORUM_TRANSACTIONAL_SETTINGS_UNAVAILABLE"),
            "{result:?}"
        );
    } else {
        assert!(
            matches!(result, Err(ForumError::Validation(ref message)) if message.contains("edit window has closed")),
            "{result:?}"
        );
    }
    assert_eq!(
        reads.load(Ordering::SeqCst),
        usize::from(readers.0 && readers.1)
    );
    assert_eq!(
        checkpoint(&db, tenant, topic, reply.id).await,
        before,
        "denied admission must roll back the claim and all domain/outbox changes"
    );
}

async fn checkpoint(
    db: &DatabaseConnection,
    tenant: Uuid,
    topic: Uuid,
    reply: Uuid,
) -> serde_json::Value {
    let source = reply_restore_checkpoint(db, tenant, topic, reply).await;
    let translations = forum_topic_translation::Entity::find()
        .filter(forum_topic_translation::Column::TenantId.eq(tenant))
        .filter(forum_topic_translation::Column::TopicId.eq(topic))
        .order_by_asc(forum_topic_translation::Column::Locale)
        .all(db)
        .await
        .unwrap();
    let revisions = forum_topic_revision::Entity::find()
        .filter(forum_topic_revision::Column::TenantId.eq(tenant))
        .filter(forum_topic_revision::Column::TopicId.eq(topic))
        .order_by_asc(forum_topic_revision::Column::Id)
        .all(db)
        .await
        .unwrap();
    serde_json::json!({ "source": source, "translations": translations, "revisions": revisions })
}
