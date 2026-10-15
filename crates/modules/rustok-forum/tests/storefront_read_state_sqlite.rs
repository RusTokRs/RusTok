#[path = "support/posting_cooldown.rs"]
mod posting_cooldown;

use std::sync::Arc;
use std::time::Duration;

use rustok_api::{PortActor, PortContext};
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput, ForumAudienceConstraints,
    ForumModule, ForumStorefrontReadStateService, ForumTopicAudiencePolicyService, ListTopicsFilter,
    ReplyService, SetForumTopicAudiencePolicyInput, TopicService,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup() -> (DatabaseConnection, TransactionalEventBus, Uuid) {
    let db_url = format!(
        "sqlite:file:forum_storefront_read_state_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(db_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("forum storefront read-state sqlite database should connect");
    let schema = SchemaManager::new(&db);
    for migration in OutboxModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("outbox migration should apply");
    }
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&schema)
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
    for migration in ForumModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("forum migration should apply");
    }
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    (db, event_bus, Uuid::new_v4())
}

fn topic_filter(category_id: Option<Uuid>, per_page: u64) -> ListTopicsFilter {
    ListTopicsFilter {
        category_id,
        status: None,
        locale: Some("en".into()),
        after: None,
        per_page,
    }
}

fn read_context(tenant_id: Uuid, reader_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(reader_id.to_string()),
        "en",
        "storefront-unread-page",
    )
    .with_deadline(Duration::from_secs(30))
}

async fn create_category(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    security: SecurityContext,
    slug: &str,
) -> rustok_forum::CategoryResponse {
    CategoryService::new(db.clone())
        .create(
            tenant_id,
            security,
            CreateCategoryInput {
                locale: "en".into(),
                name: slug.replace('-', " "),
                slug: slug.into(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await
        .expect("category should be created")
}

#[tokio::test]
async fn visible_topic_summary_and_current_mark_share_owner_policy() {
    let (db, event_bus, tenant_id) = setup().await;
    let author = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let reader = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let anonymous = SecurityContext::new(UserRole::Customer, None);

    let category = create_category(&db, tenant_id, author.clone(), "storefront-read-state").await;
    let topic = TopicService::new(db.clone(), event_bus.clone())
        .with_settings_providers(posting_cooldown::zero_cooldown_providers())
        .create(
            tenant_id,
            author.clone(),
            CreateTopicInput {
                locale: "en".into(),
                category_id: category.id,
                title: "Visible storefront topic".into(),
                slug: Some("visible-storefront-topic".into()),
                body: rustok_api::RichTextDocument::single_paragraph("Opening body"),
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created");
    ReplyService::new(db.clone(), event_bus.clone())
        .with_settings_providers(posting_cooldown::zero_cooldown_providers())
        .create(
            tenant_id,
            author,
            topic.id,
            CreateReplyInput {
                locale: "en".into(),
                content: rustok_api::RichTextDocument::single_paragraph("Approved reply"),
                parent_reply_id: None,
            },
        )
        .await
        .expect("reply should be created");

    let service = ForumStorefrontReadStateService::new(db, event_bus);
    let unseen = service
        .list_topics_with_unread(
            tenant_id,
            reader.clone(),
            topic_filter(Some(category.id), 20),
            Some("en"),
            None,
        )
        .await
        .expect("visible topic page should summarize");
    assert_eq!(unseen.items.len(), 1);
    assert_eq!(unseen.items[0].topic.id, topic.id);
    assert!(unseen.items[0].is_unread);
    assert!(unseen.items[0].unread_count > 0 || unseen.items[0].has_unread_topic_revision);

    let state = service
        .mark_topic_read_current_visible(
            tenant_id,
            topic.id,
            reader.clone(),
            "en",
            Some("en"),
            None,
        )
        .await
        .expect("current visible topic should be marked read");
    assert!(state.explicit);

    let read = service
        .list_topics_with_unread(
            tenant_id,
            reader,
            topic_filter(Some(category.id), 20),
            Some("en"),
            None,
        )
        .await
        .expect("marked topic should summarize");
    assert_eq!(read.items.len(), 1);
    assert!(!read.items[0].is_unread);
    assert_eq!(read.items[0].unread_count, 0);
    assert!(!read.items[0].has_unread_topic_revision);

    assert!(
        service
            .list_topics_with_unread(
                tenant_id,
                anonymous.clone(),
                topic_filter(Some(category.id), 20),
                Some("en"),
                None,
            )
            .await
            .is_err()
    );
    assert!(
        service
            .mark_topic_read_current_visible(
                tenant_id,
                topic.id,
                anonymous,
                "en",
                Some("en"),
                None,
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn channel_restricted_topic_cannot_be_enriched_or_marked_outside_its_channel() {
    let (db, event_bus, tenant_id) = setup().await;
    let author = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let reader = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let category = create_category(&db, tenant_id, author.clone(), "restricted-channel").await;
    let topic = TopicService::new(db.clone(), event_bus.clone())
        .with_settings_providers(posting_cooldown::zero_cooldown_providers())
        .create(
            tenant_id,
            author,
            CreateTopicInput {
                locale: "en".into(),
                category_id: category.id,
                title: "Mobile only".into(),
                slug: Some("mobile-only".into()),
                body: rustok_api::RichTextDocument::single_paragraph("Restricted body"),
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: Some(vec!["mobile".into()]),
            },
        )
        .await
        .expect("restricted topic should be created");
    let service = ForumStorefrontReadStateService::new(db, event_bus);

    let public_page = service
        .list_topics_with_unread(
            tenant_id,
            reader.clone(),
            topic_filter(Some(category.id), 20),
            Some("en"),
            None,
        )
        .await
        .expect("nonmatching storefront scope should still return a page");
    assert!(public_page.items.is_empty());
    assert!(
        service
            .mark_topic_read_current_visible(
                tenant_id,
                topic.id,
                reader.clone(),
                "en",
                Some("en"),
                None,
            )
            .await
            .is_err()
    );

    let mobile_page = service
        .list_topics_with_unread(
            tenant_id,
            reader.clone(),
            topic_filter(Some(category.id), 20),
            Some("en"),
            Some("mobile"),
        )
        .await
        .expect("matching channel should expose the topic");
    assert_eq!(mobile_page.items.len(), 1);
    assert_eq!(mobile_page.items[0].topic.id, topic.id);
    service
        .mark_topic_read_current_visible(
            tenant_id,
            topic.id,
            reader,
            "en",
            Some("en"),
            Some("mobile"),
        )
        .await
        .expect("matching channel should permit the read-state mutation");
}

#[tokio::test]
async fn storefront_unread_page_rejects_unbounded_requests_before_querying() {
    let (db, event_bus, tenant_id) = setup().await;
    let reader = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let service = ForumStorefrontReadStateService::new(db, event_bus);
    let error = service
        .list_topics_with_unread(tenant_id, reader.clone(), topic_filter(None, 101), Some("en"), None)
        .await
        .expect_err("unbounded storefront unread page should fail");
    assert!(error.to_string().contains("between 1 and 100"));
    let reader_id = reader.user_id.expect("reader id");
    let error = service
        .list_topics_with_unread_audience_visible(
            tenant_id, reader, read_context(tenant_id, reader_id),
            topic_filter(None, 101), Some("en"), true,
        )
        .await
        .expect_err("unbounded audience-aware unread page should fail");
    assert!(error.to_string().contains("between 1 and 100"));
}

#[tokio::test]
async fn unread_only_skips_read_and_hidden_topics_without_sparse_pages_or_raw_cursors() {
    let (db, event_bus, tenant_id) = setup().await;
    let author = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let reader_id = Uuid::new_v4();
    let reader = SecurityContext::new(UserRole::Customer, Some(reader_id));
    let category = create_category(&db, tenant_id, author.clone(), "unread-only").await;

    let writer = TopicService::new(db.clone(), event_bus.clone())
        .with_settings_providers(posting_cooldown::zero_cooldown_providers());
    for index in 0..5 {
        let slug = format!("unread-page-{index}");
        let topic = writer
            .create(
                tenant_id,
                author.clone(),
                CreateTopicInput {
                    locale: "en".into(),
                    category_id: category.id,
                    title: slug.clone(),
                    slug: Some(slug),
                    body: rustok_api::RichTextDocument::single_paragraph("Unseen opening post"),
                    metadata: serde_json::json!({}),
                    tags: vec![],
                    channel_slugs: None,
                },
            )
            .await
            .expect("storefront test topic should be created");
        if index == 4 {
            ForumTopicAudiencePolicyService::new(db.clone())
                .set(
                    tenant_id,
                    topic.id,
                    author.clone(),
                    SetForumTopicAudiencePolicyInput {
                        constraints: ForumAudienceConstraints {
                            deny_user_ids: vec![reader_id],
                            ..ForumAudienceConstraints::default()
                        },
                    },
                )
                .await
                .expect("hidden topic policy should persist");
        }
    }

    let service = ForumStorefrontReadStateService::new(db, event_bus);
    let all = service
        .list_topics_with_unread_audience_visible(
            tenant_id, reader.clone(), read_context(tenant_id, reader_id),
            topic_filter(Some(category.id), 20), Some("en"), false,
        )
        .await
        .expect("personalized feed should load");
    assert_eq!(all.items.len(), 4, "hidden audience topic must not be enriched");
    assert!(all.items.iter().all(|item| item.is_unread));

    // Read the first two topics in actual keyset order. An unread-only request
    // for one result must pass the now-empty first candidate batch and continue.
    for item in &all.items[..2] {
        service
            .mark_topic_read_current_audience_visible(
                tenant_id, item.topic.id, reader.clone(),
                read_context(tenant_id, reader_id), Some("en"),
            )
            .await
            .expect("visible topic should be marked read");
    }
    let after_mark = service
        .list_topics_with_unread_audience_visible(
            tenant_id, reader.clone(), read_context(tenant_id, reader_id),
            topic_filter(Some(category.id), 20), Some("en"), false,
        )
        .await
        .expect("read-state badges should remain on the ordinary feed");
    assert_eq!(after_mark.items.len(), 4);
    assert_eq!(after_mark.items.iter().filter(|item| item.is_unread).count(), 2);

    let first = service
        .list_topics_with_unread_audience_visible(
            tenant_id, reader.clone(), read_context(tenant_id, reader_id),
            topic_filter(Some(category.id), 1), Some("en"), true,
        )
        .await
        .expect("first unread-only page should skip the read-only batch");
    assert_eq!(first.items.len(), 1);
    assert_eq!(first.items[0].topic.id, all.items[2].topic.id);
    let cursor = first.next_cursor.expect("one more unread topic exists");
    assert_eq!(rustok_forum::TopicListCursor::decode(&cursor).expect("valid cursor").id,
        first.items[0].topic.id, "cursor must point to the returned unread topic");

    let second = service
        .list_topics_with_unread_audience_visible(
            tenant_id, reader, read_context(tenant_id, reader_id),
            ListTopicsFilter { after: Some(cursor), ..topic_filter(Some(category.id), 1) },
            Some("en"), true,
        )
        .await
        .expect("second unread-only page should load");
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.items[0].topic.id, all.items[3].topic.id);
    assert!(second.next_cursor.is_none());
}
