#[path = "support/posting_cooldown.rs"]
mod posting_cooldown;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rustok_api::{Permission, PortActor, PortContext, PortError};
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumAudienceConstraints,
    ForumAudienceFacts, ForumAudienceFactsPort, ForumAudienceFactsRequest,
    ForumCategoryAudiencePolicyService, ForumError, ForumModule, ForumWidgetPreviewPayload,
    ForumWidgetPreviewService, PreviewForumWidgetInput, SetForumCategoryAudiencePolicyInput,
    SharedForumAudienceFactsPort, TopicService,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, Statement};
use sea_orm_migration::SchemaManager;
use serde_json::json;
use uuid::Uuid;

const TOPIC_LIST: &str = "forum.topic_list";
const TOPIC_DETAIL: &str = "forum.topic_detail";
const REPLY_STREAM: &str = "forum.reply_stream";

/// Trust facts for the widget tests: one low-trust user, everyone else is trusted.
struct TrustFactsPort {
    low_trust_user_id: Uuid,
}

#[async_trait]
impl ForumAudienceFactsPort for TrustFactsPort {
    async fn resolve_forum_audience_facts(
        &self,
        _context: PortContext,
        request: ForumAudienceFactsRequest,
    ) -> Result<ForumAudienceFacts, PortError> {
        Ok(ForumAudienceFacts {
            tenant_id: request.tenant_id,
            user_id: request.user_id,
            trust_level: request.include_trust_level.then_some(
                if request.user_id == self.low_trust_user_id {
                    1
                } else {
                    8
                },
            ),
            channel_memberships: Vec::new(),
            group_memberships: Vec::new(),
        })
    }
}

async fn setup() -> (DatabaseConnection, TransactionalEventBus) {
    let database_url = format!(
        "sqlite:file:forum_widget_preview_audience_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("widget audience SQLite database should connect");

    db.execute_unprepared(
        "CREATE TABLE IF NOT EXISTS users (
            id TEXT NOT NULL PRIMARY KEY,
            tenant_id TEXT NOT NULL
        );",
    )
    .await
    .expect("users table fixture should apply");

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
    for migration in ForumModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("forum migration should apply");
    }

    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    (db, event_bus)
}

async fn insert_user(db: &DatabaseConnection, tenant_id: Uuid, user_id: Uuid) {
    db.execute_raw(Statement::from_sql_and_values(
        sea_orm::DatabaseBackend::Sqlite,
        "INSERT INTO users (id, tenant_id) VALUES (?1, ?2)",
        vec![user_id.into(), tenant_id.into()],
    ))
    .await
    .expect("platform user fixture should insert");
}

async fn create_category(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    security: SecurityContext,
    slug: &str,
) -> Uuid {
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
        .id
}

async fn create_topic(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    security: SecurityContext,
    category_id: Uuid,
    slug: &str,
) -> Uuid {
    TopicService::new(db.clone(), event_bus.clone())
        .with_settings_providers(posting_cooldown::zero_cooldown_providers())
        .create(
            tenant_id,
            security,
            CreateTopicInput {
                locale: "en".into(),
                category_id,
                title: slug.replace('-', " "),
                slug: Some(slug.into()),
                body: rustok_api::RichTextDocument::single_paragraph("Widget audience fixture"),
                metadata: json!({}),
                tags: Vec::new(),
                channel_slugs: Some(vec!["web".into()]),
            },
        )
        .await
        .expect("topic should be created")
        .id
}

fn read_context(tenant_id: Uuid, user_id: Uuid, correlation: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(user_id.to_string()),
        "en",
        correlation,
    )
    .with_deadline(Duration::from_secs(1))
}

fn viewer_security(user_id: Uuid) -> SecurityContext {
    SecurityContext::from_permission_snapshot(
        Some(user_id),
        &[
            Permission::FORUM_TOPICS_READ,
            Permission::FORUM_REPLIES_READ,
        ],
    )
}

fn input(widget_type: &str, props: serde_json::Value) -> PreviewForumWidgetInput {
    PreviewForumWidgetInput {
        widget_type: widget_type.to_string(),
        props,
    }
}

struct Fixture {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    tenant_id: Uuid,
    trusted_user_id: Uuid,
    low_trust_user_id: Uuid,
    public_category: Uuid,
    restricted_category: Uuid,
    restricted_topic_id: Uuid,
}

/// Public category with two topics; a category restricted to trust level 5 with two topics.
async fn fixture() -> Fixture {
    let (db, event_bus) = setup().await;
    let tenant_id = Uuid::new_v4();
    let admin_user_id = Uuid::new_v4();
    let trusted_user_id = Uuid::new_v4();
    let low_trust_user_id = Uuid::new_v4();
    for user_id in [admin_user_id, trusted_user_id, low_trust_user_id] {
        insert_user(&db, tenant_id, user_id).await;
    }

    let admin = SecurityContext::new(UserRole::Admin, Some(admin_user_id));
    let public_category = create_category(&db, tenant_id, admin.clone(), "public-area").await;
    let restricted_category = create_category(&db, tenant_id, admin.clone(), "trusted-area").await;

    ForumCategoryAudiencePolicyService::new(db.clone())
        .set(
            tenant_id,
            restricted_category,
            admin.clone(),
            SetForumCategoryAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    minimum_trust_level: Some(5),
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("restricted category trust layer should persist");

    for slug in ["public-one", "public-two"] {
        create_topic(
            &db,
            &event_bus,
            tenant_id,
            admin.clone(),
            public_category,
            slug,
        )
        .await;
    }
    let mut restricted_topic_id = None;
    for slug in ["restricted-one", "restricted-two"] {
        let id = create_topic(
            &db,
            &event_bus,
            tenant_id,
            admin.clone(),
            restricted_category,
            slug,
        )
        .await;
        restricted_topic_id.get_or_insert(id);
    }

    Fixture {
        db,
        event_bus,
        tenant_id,
        trusted_user_id,
        low_trust_user_id,
        public_category,
        restricted_category,
        restricted_topic_id: restricted_topic_id.expect("restricted fixture topic should exist"),
    }
}

fn preview_service(fixture: &Fixture) -> ForumWidgetPreviewService {
    let facts: SharedForumAudienceFactsPort = Arc::new(TrustFactsPort {
        low_trust_user_id: fixture.low_trust_user_id,
    });
    ForumWidgetPreviewService::new(fixture.db.clone(), fixture.event_bus.clone(), Some(facts))
}

#[tokio::test]
async fn widget_topic_list_counts_and_pages_only_owner_visible_topics() {
    let fixture = fixture().await;
    let service = preview_service(&fixture);

    let low = service
        .preview(
            fixture.tenant_id,
            viewer_security(fixture.low_trust_user_id),
            read_context(
                fixture.tenant_id,
                fixture.low_trust_user_id,
                "low-widget-list",
            ),
            Some("en"),
            input(
                TOPIC_LIST,
                json!({ "page": 1, "per_page": 10, "include_pinned": true, "sort": "newest" }),
            ),
        )
        .await
        .expect("low-trust topic list preview should resolve");
    let Some(ForumWidgetPreviewPayload::TopicList(low_list)) = low.payload else {
        panic!("topic list preview must return a topic list payload");
    };
    assert_eq!(low_list.total, 2, "restricted topics must not be counted");
    assert_eq!(low_list.items.len(), 2);
    assert!(
        low_list
            .items
            .iter()
            .all(|item| item.category_id == fixture.public_category),
        "restricted topics must not appear in the widget list"
    );

    let trusted = service
        .preview(
            fixture.tenant_id,
            viewer_security(fixture.trusted_user_id),
            read_context(
                fixture.tenant_id,
                fixture.trusted_user_id,
                "trusted-widget-list",
            ),
            Some("en"),
            input(
                TOPIC_LIST,
                json!({ "page": 2, "per_page": 3, "include_pinned": true, "sort": "newest" }),
            ),
        )
        .await
        .expect("trusted topic list preview should resolve");
    let Some(ForumWidgetPreviewPayload::TopicList(trusted_list)) = trusted.payload else {
        panic!("topic list preview must return a topic list payload");
    };
    assert_eq!(trusted_list.total, 4, "trusted viewer reads every topic");
    assert_eq!(
        trusted_list.items.len(),
        1,
        "page two of four visible topics at size three holds one topic"
    );
}

#[tokio::test]
async fn widget_topic_detail_denies_restricted_topic_as_not_found() {
    let fixture = fixture().await;
    let service = preview_service(&fixture);
    let restricted = fixture.restricted_topic_id;

    let denied = service
        .preview(
            fixture.tenant_id,
            viewer_security(fixture.low_trust_user_id),
            read_context(
                fixture.tenant_id,
                fixture.low_trust_user_id,
                "low-widget-detail",
            ),
            Some("en"),
            input(
                TOPIC_DETAIL,
                json!({ "topic_id": restricted.to_string(), "include_replies": true }),
            ),
        )
        .await;
    assert!(
        matches!(denied, Err(ForumError::TopicNotFound(id)) if id == restricted),
        "topic detail preview must deny a topic whose category audience the viewer does not satisfy"
    );

    let allowed = service
        .preview(
            fixture.tenant_id,
            viewer_security(fixture.trusted_user_id),
            read_context(
                fixture.tenant_id,
                fixture.trusted_user_id,
                "trusted-widget-detail",
            ),
            Some("en"),
            input(
                TOPIC_DETAIL,
                json!({ "topic_id": restricted.to_string(), "include_replies": true }),
            ),
        )
        .await
        .expect("trusted topic detail preview should resolve");
    let Some(ForumWidgetPreviewPayload::TopicDetail(detail)) = allowed.payload else {
        panic!("topic detail preview must return a topic detail payload");
    };
    assert_eq!(detail.topic.id, restricted);
    assert_eq!(detail.topic.category_id, fixture.restricted_category);
}

#[tokio::test]
async fn widget_reply_stream_denies_replies_of_restricted_topic() {
    let fixture = fixture().await;
    let service = preview_service(&fixture);
    let restricted = fixture.restricted_topic_id;

    let denied = service
        .preview(
            fixture.tenant_id,
            viewer_security(fixture.low_trust_user_id),
            read_context(
                fixture.tenant_id,
                fixture.low_trust_user_id,
                "low-widget-replies",
            ),
            Some("en"),
            input(
                REPLY_STREAM,
                json!({
                    "topic_id": restricted.to_string(),
                    "per_page": 20,
                    "approved_only": true
                }),
            ),
        )
        .await;
    assert!(
        matches!(denied, Err(ForumError::TopicNotFound(id)) if id == restricted),
        "reply preview must authorise through the parent topic audience"
    );
}

#[tokio::test]
async fn widget_preview_rejects_audience_context_for_another_actor() {
    let fixture = fixture().await;
    let service = preview_service(&fixture);

    let mismatched = service
        .preview(
            fixture.tenant_id,
            viewer_security(fixture.trusted_user_id),
            read_context(
                fixture.tenant_id,
                fixture.low_trust_user_id,
                "mismatched-widget",
            ),
            Some("en"),
            input(
                TOPIC_LIST,
                json!({ "page": 1, "per_page": 10, "include_pinned": true, "sort": "newest" }),
            ),
        )
        .await;
    assert!(
        matches!(mismatched, Err(ForumError::Validation(_))),
        "a preview must not run with an audience context for a different user"
    );
}
