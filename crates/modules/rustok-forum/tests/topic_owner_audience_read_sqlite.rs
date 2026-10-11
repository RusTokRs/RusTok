#[path = "support/posting_cooldown.rs"]
mod posting_cooldown;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rustok_api::{PortActor, PortContext, PortError};
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumAudienceConstraints,
    ForumAudienceFacts, ForumAudienceFactsPort, ForumAudienceFactsRequest,
    ForumCategoryAudiencePolicyService, ForumError, ForumModule, ForumTopicAudienceListService,
    ForumTopicAudienceReadService, ListTopicsFilter, SetForumCategoryAudiencePolicyInput,
    TopicService,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, Statement};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

/// Trust facts for the owner-read tests: one low-trust user, everyone else is trusted.
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
        "sqlite:file:forum_topic_owner_audience_read_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("owner audience read SQLite database should connect");

    db.execute_unprepared(
        r#"
        CREATE TABLE users (
            id TEXT NOT NULL PRIMARY KEY,
            tenant_id TEXT NOT NULL
        )
        "#,
    )
    .await
    .expect("SQLite platform user fixture should be created");

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
                body: rustok_api::RichTextDocument::single_paragraph("Owner audience fixture"),
                metadata: serde_json::json!({}),
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

fn list_filter(per_page: u64, after: Option<String>) -> ListTopicsFilter {
    ListTopicsFilter {
        category_id: None,
        status: None,
        locale: Some("en".into()),
        after,
        per_page,
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
    for slug in ["restricted-one", "restricted-two"] {
        create_topic(
            &db,
            &event_bus,
            tenant_id,
            admin.clone(),
            restricted_category,
            slug,
        )
        .await;
    }

    Fixture {
        db,
        event_bus,
        tenant_id,
        trusted_user_id,
        low_trust_user_id,
        public_category,
        restricted_category,
    }
}

#[tokio::test]
async fn owner_selected_topic_read_denies_restricted_topic_as_not_found() {
    let fixture = fixture().await;
    let restricted_topic_id = {
        let admin = SecurityContext::new(UserRole::Admin, None);
        let service = TopicService::new(fixture.db.clone(), fixture.event_bus.clone())
            .with_settings_providers(posting_cooldown::zero_cooldown_providers());
        service
            .list(fixture.tenant_id, admin, list_filter(50, None))
            .await
            .expect("admin list should resolve")
            .items
            .into_iter()
            .find(|item| item.category_id == fixture.restricted_category)
            .expect("restricted fixture topic should exist")
            .id
    };

    let service = ForumTopicAudienceReadService::with_audience_facts(
        fixture.db.clone(),
        fixture.event_bus.clone(),
        Arc::new(TrustFactsPort {
            low_trust_user_id: fixture.low_trust_user_id,
        }),
    );

    let low_security = SecurityContext::new(UserRole::Customer, Some(fixture.low_trust_user_id));
    let denied = service
        .get_authenticated_owner_visible_with_audience_context(
            fixture.tenant_id,
            low_security,
            read_context(
                fixture.tenant_id,
                fixture.low_trust_user_id,
                "low-owner-read",
            ),
            restricted_topic_id,
            Some("en"),
        )
        .await;
    assert!(
        matches!(denied, Err(ForumError::TopicNotFound(id)) if id == restricted_topic_id),
        "owner read must deny a topic whose category audience the viewer does not satisfy"
    );

    let trusted_security = SecurityContext::new(UserRole::Customer, Some(fixture.trusted_user_id));
    let allowed = service
        .get_authenticated_owner_visible_with_audience_context(
            fixture.tenant_id,
            trusted_security,
            read_context(
                fixture.tenant_id,
                fixture.trusted_user_id,
                "trusted-owner-read",
            ),
            restricted_topic_id,
            Some("en"),
        )
        .await
        .expect("trusted owner read should resolve");
    assert_eq!(allowed.id, restricted_topic_id);
    assert_eq!(allowed.category_id, fixture.restricted_category);
}

#[tokio::test]
async fn owner_topic_list_applies_audience_before_pagination() {
    let fixture = fixture().await;
    let service = ForumTopicAudienceListService::with_audience_facts(
        fixture.db.clone(),
        fixture.event_bus.clone(),
        Arc::new(TrustFactsPort {
            low_trust_user_id: fixture.low_trust_user_id,
        }),
    );

    let low_security = SecurityContext::new(UserRole::Customer, Some(fixture.low_trust_user_id));
    let low_page = service
        .list_authenticated_owner_visible_with_audience_context(
            fixture.tenant_id,
            low_security,
            read_context(
                fixture.tenant_id,
                fixture.low_trust_user_id,
                "low-owner-list",
            ),
            list_filter(2, None),
            Some("en"),
        )
        .await
        .expect("low-trust owner list should resolve");
    assert_eq!(
        low_page.items.len(),
        2,
        "only the public topics are visible"
    );
    assert!(
        low_page
            .items
            .iter()
            .all(|item| item.category_id == fixture.public_category),
        "restricted topics must not appear in the owner list"
    );
    assert!(
        low_page.next_cursor.is_none(),
        "no further visible topic exists, so there must be no next cursor"
    );

    let trusted_security = SecurityContext::new(UserRole::Customer, Some(fixture.trusted_user_id));
    let first = service
        .list_authenticated_owner_visible_with_audience_context(
            fixture.tenant_id,
            trusted_security.clone(),
            read_context(
                fixture.tenant_id,
                fixture.trusted_user_id,
                "trusted-owner-list-1",
            ),
            list_filter(3, None),
            Some("en"),
        )
        .await
        .expect("trusted owner first page should resolve");
    assert_eq!(first.items.len(), 3);
    let cursor = first
        .next_cursor
        .clone()
        .expect("four visible topics with page size three must produce a cursor");

    let second = service
        .list_authenticated_owner_visible_with_audience_context(
            fixture.tenant_id,
            trusted_security,
            read_context(
                fixture.tenant_id,
                fixture.trusted_user_id,
                "trusted-owner-list-2",
            ),
            list_filter(3, Some(cursor)),
            Some("en"),
        )
        .await
        .expect("trusted owner second page should resolve");
    assert_eq!(
        second.items.len(),
        1,
        "the last page holds the remaining topic"
    );
    assert!(second.next_cursor.is_none());

    let mut seen: Vec<Uuid> = first
        .items
        .iter()
        .chain(second.items.iter())
        .map(|item| item.id)
        .collect();
    seen.sort();
    seen.dedup();
    assert_eq!(
        seen.len(),
        4,
        "pages must cover every visible topic exactly once"
    );
}

#[tokio::test]
async fn public_storefront_topic_list_excludes_audience_restricted_topics() {
    let fixture = fixture().await;
    let service = ForumTopicAudienceListService::new(fixture.db.clone(), fixture.event_bus.clone());

    let page = service
        .list_public_storefront_visible_with_locale_fallback(
            fixture.tenant_id,
            list_filter(10, None),
            Some("en"),
            Some("web"),
        )
        .await
        .expect("public storefront list should resolve");

    assert_eq!(
        page.items.len(),
        2,
        "public sitemap and storefront listings must hide audience-restricted topics"
    );
    assert!(
        page.items
            .iter()
            .all(|item| item.category_id == fixture.public_category),
        "only topics in the public category may be listed for anonymous viewers"
    );
}
