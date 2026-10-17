#[path = "support/forum_public_settings.rs"]
mod forum_public_settings;
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
    ForumCategoryAudiencePolicyService, ForumError, ForumModule, ForumSettingsProviders,
    ForumTopicAudienceListService, ForumTopicAudienceReadService, ListTopicsFilter,
    SetForumCategoryAudiencePolicyInput, TopicService,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, Statement};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

fn default_public_settings() -> ForumSettingsProviders {
    ForumSettingsProviders::default()
        .with_static_reader(forum_public_settings::default_settings_reader())
}

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
        search: None,
        order: Default::default(),
        solved_only: false,
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
    let service = ForumTopicAudienceListService::new(fixture.db.clone(), fixture.event_bus.clone())
        .with_settings_providers(default_public_settings());

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

#[tokio::test]
async fn category_global_topic_counter_is_not_the_viewers_page_size_or_cursor() {
    use rustok_forum::{
        ForumCategoryAudienceReadService, ForumTopicAudiencePolicyService,
        SetForumTopicAudiencePolicyInput,
    };

    let fixture = fixture().await;
    let admin = SecurityContext::new(UserRole::Admin, Some(fixture.trusted_user_id));
    let members_topic = create_topic(
        &fixture.db,
        &fixture.event_bus,
        fixture.tenant_id,
        admin.clone(),
        fixture.public_category,
        "members-only",
    )
    .await;
    ForumTopicAudiencePolicyService::new(fixture.db.clone())
        .set(
            fixture.tenant_id,
            members_topic,
            admin,
            SetForumTopicAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    roles_any: vec![UserRole::Customer],
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("members-only audience");

    let category = ForumCategoryAudienceReadService::new(fixture.db.clone())
        .with_settings_providers(default_public_settings())
        .get_public_storefront_visible_with_locale_fallback(
            fixture.tenant_id,
            fixture.public_category,
            "en",
            None,
        )
        .await
        .expect("public category");
    assert_eq!(
        category.topic_count, 3,
        "stored aggregate includes the restricted topic"
    );

    let list = ForumTopicAudienceListService::new(fixture.db.clone(), fixture.event_bus.clone())
        .with_settings_providers(default_public_settings());
    let filter = ListTopicsFilter {
        category_id: Some(fixture.public_category),
        ..list_filter(10, None)
    };
    let public = list
        .list_public_storefront_visible_with_locale_fallback(
            fixture.tenant_id,
            filter.clone(),
            None,
            Some("web"),
        )
        .await
        .expect("public page");
    assert_eq!(public.items.len(), 2);
    assert!(public.next_cursor.is_none());
    assert!(public.items.iter().all(|item| item.id != members_topic));
    let customer = list
        .list_authenticated_storefront_visible_with_audience_context(
            fixture.tenant_id,
            SecurityContext::new(UserRole::Customer, Some(fixture.trusted_user_id)),
            read_context(fixture.tenant_id, fixture.trusted_user_id, "mixed-counts"),
            filter,
            None,
        )
        .await
        .expect("signed-in page");
    assert_eq!(customer.items.len(), 3);
    assert!(customer.items.iter().any(|item| item.id == members_topic));
    assert_eq!(category.topic_count as usize, customer.items.len());
}

async fn selected_owner_page(
    fixture: &Fixture,
    filter: ListTopicsFilter,
) -> Result<rustok_forum::ForumTopicAudiencePage, ForumError> {
    let mut context = read_context(
        fixture.tenant_id,
        fixture.low_trust_user_id,
        "selected-owner-page",
    )
    .with_deadline(Duration::from_secs(30));
    context.locale = filter.locale.clone().unwrap_or_else(|| "en".into());
    ForumTopicAudienceListService::with_audience_facts(
        fixture.db.clone(),
        fixture.event_bus.clone(),
        Arc::new(TrustFactsPort {
            low_trust_user_id: fixture.low_trust_user_id,
        }),
    )
    .list_authenticated_owner_visible_with_audience_context(
        fixture.tenant_id,
        SecurityContext::new(UserRole::Customer, Some(fixture.low_trust_user_id)),
        context,
        filter,
        Some("en"),
    )
    .await
}

async fn selection_sql(fixture: &Fixture, sql: &str, values: Vec<sea_orm::Value>) {
    fixture
        .db
        .execute_raw(Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            sql,
            values,
        ))
        .await
        .expect("selection fixture SQL should execute");
}

#[tokio::test]
async fn search_refills_across_bounded_candidate_batches_without_hidden_matches_or_false_cursors() {
    let fixture = fixture().await;
    selection_sql(
        &fixture,
        "UPDATE forum_topic_translations SET title = 'НАЙТИ нужную тему' WHERE tenant_id = ?1",
        vec![fixture.tenant_id.into()],
    )
    .await;
    // Every initial candidate batch is irrelevant; four older matches include two hidden ones.
    for n in 0..105 {
        create_topic(
            &fixture.db,
            &fixture.event_bus,
            fixture.tenant_id,
            SecurityContext::new(UserRole::Admin, None),
            fixture.public_category,
            &format!("newer-noise-{n}"),
        )
        .await;
    }
    let filter = ListTopicsFilter {
        search: Some("  найти  ".into()),
        ..list_filter(1, None)
    };
    let first = selected_owner_page(&fixture, filter.clone())
        .await
        .expect("sparse search first page");
    assert_eq!(first.items.len(), 1);
    assert_eq!(first.items[0].category_id, fixture.public_category);
    let cursor = first
        .next_cursor
        .clone()
        .expect("another authorized match exists");
    assert_eq!(
        rustok_forum::TopicListCursor::decode(&cursor)
            .expect("cursor")
            .id,
        first.items[0].id
    );
    let second = selected_owner_page(
        &fixture,
        ListTopicsFilter {
            after: Some(cursor),
            ..filter
        },
    )
    .await
    .expect("sparse search continuation");
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.items[0].category_id, fixture.public_category);
    assert_ne!(first.items[0].id, second.items[0].id);
    assert!(
        second.next_cursor.is_none(),
        "hidden matches must not manufacture another page"
    );
}

#[tokio::test]
async fn top_ranks_the_entire_authorized_selection_then_uses_latest_ties() {
    let fixture = fixture().await;
    let visible = selected_owner_page(&fixture, list_filter(20, None))
        .await
        .expect("visible topics")
        .items;
    let leader = visible[1].id;
    selection_sql(
        &fixture,
        "UPDATE forum_topics SET reply_count = 1000 WHERE category_id = ?1 AND tenant_id = ?2",
        vec![fixture.restricted_category.into(), fixture.tenant_id.into()],
    )
    .await;
    selection_sql(&fixture, "UPDATE forum_topics SET reply_count = 1, is_pinned = 1 WHERE category_id = ?1 AND tenant_id = ?2",
        vec![fixture.public_category.into(), fixture.tenant_id.into()]).await;
    selection_sql(&fixture, "UPDATE forum_topics SET reply_count = 50, is_pinned = 0, updated_at = ?3 WHERE id = ?1 AND tenant_id = ?2",
        vec![leader.into(), fixture.tenant_id.into(), chrono::DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z").unwrap().into()]).await;
    let filter = ListTopicsFilter {
        order: rustok_forum::ForumTopicListOrder::Top,
        ..list_filter(1, None)
    };
    let first = selected_owner_page(&fixture, filter.clone())
        .await
        .expect("top page");
    assert_eq!(
        first.items[0].id, leader,
        "reply count outranks pinning/recency"
    );
    let second = selected_owner_page(
        &fixture,
        ListTopicsFilter {
            after: first.next_cursor,
            ..filter.clone()
        },
    )
    .await
    .expect("top continuation");
    assert_eq!(second.items.len(), 1);
    assert_ne!(second.items[0].id, leader);
    assert!(second.next_cursor.is_none());

    selection_sql(&fixture, "UPDATE forum_topics SET reply_count = 50, is_pinned = 0, last_reply_at = NULL, updated_at = ?2 WHERE tenant_id = ?1",
        vec![fixture.tenant_id.into(), chrono::DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z").unwrap().into()]).await;
    let mut expected: Vec<_> = visible.iter().map(|topic| topic.id).collect();
    expected.sort_by(|a, b| b.cmp(a));
    let tied = selected_owner_page(&fixture, filter.clone())
        .await
        .expect("tie first");
    assert_eq!(tied.items[0].id, expected[0]);
    let tied_last = selected_owner_page(
        &fixture,
        ListTopicsFilter {
            after: tied.next_cursor,
            ..filter
        },
    )
    .await
    .expect("tie continuation");
    assert_eq!(tied_last.items[0].id, expected[1]);
    assert!(tied_last.next_cursor.is_none());
}

#[tokio::test]
async fn solved_selection_pages_the_canonical_solution_relation_not_a_loaded_subset() {
    let fixture = fixture().await;
    let visible = selected_owner_page(&fixture, list_filter(20, None))
        .await
        .expect("visible topics");
    for topic in &visible.items {
        let admin = SecurityContext::new(UserRole::Admin, None);
        let reply = rustok_forum::ReplyService::new(fixture.db.clone(), fixture.event_bus.clone())
            .with_settings_providers(posting_cooldown::zero_cooldown_providers())
            .create(
                fixture.tenant_id,
                admin.clone(),
                topic.id,
                rustok_forum::CreateReplyInput {
                    locale: "en".into(),
                    content: rustok_api::RichTextDocument::single_paragraph("Accepted answer"),
                    parent_reply_id: None,
                },
            )
            .await
            .expect("solution reply");
        rustok_forum::ModerationService::new(fixture.db.clone(), fixture.event_bus.clone())
            .mark_solution(fixture.tenant_id, topic.id, reply.id, admin)
            .await
            .expect("canonical solution");
    }
    create_topic(
        &fixture.db,
        &fixture.event_bus,
        fixture.tenant_id,
        SecurityContext::new(UserRole::Admin, None),
        fixture.public_category,
        "newest-unsolved",
    )
    .await;
    let filter = ListTopicsFilter {
        solved_only: true,
        ..list_filter(1, None)
    };
    let first = selected_owner_page(&fixture, filter.clone())
        .await
        .expect("solved page");
    assert_eq!(first.items.len(), 1);
    assert!(first.items[0].solution_reply_id.is_some());
    let second = selected_owner_page(
        &fixture,
        ListTopicsFilter {
            after: first.next_cursor.clone(),
            ..filter
        },
    )
    .await
    .expect("solved continuation");
    assert_eq!(second.items.len(), 1);
    assert!(second.items[0].solution_reply_id.is_some());
    assert_ne!(first.items[0].id, second.items[0].id);
    assert!(second.next_cursor.is_none());
}

#[tokio::test]
async fn search_matches_only_the_displayed_locale_title_or_slug_including_fallback() {
    let fixture = fixture().await;
    let topic = selected_owner_page(&fixture, list_filter(1, None))
        .await
        .expect("topic")
        .items[0]
        .id;
    selection_sql(&fixture, "UPDATE forum_topic_translations SET title = 'Привет ЁЖ' WHERE topic_id = ?1 AND tenant_id = ?2",
        vec![topic.into(), fixture.tenant_id.into()]).await;
    selection_sql(&fixture,
        "INSERT INTO forum_topic_translations (id, topic_id, tenant_id, locale, title, slug, body, created_at, updated_at) SELECT ?1, topic_id, tenant_id, 'fr', 'Bonjour spécifique', 'fr-localized-only', body, created_at, updated_at FROM forum_topic_translations WHERE topic_id = ?2 AND tenant_id = ?3 AND locale = 'en'",
        vec![Uuid::new_v4().into(), topic.into(), fixture.tenant_id.into()]).await;
    for (locale, search, expected) in [
        ("en", "spécifique", false),
        ("en", "fr-localized-only", false),
        ("fr", "SPÉCIFIQUE", true),
        ("fr", "fr-localized", true),
        ("de", " Ёж ", true),
    ] {
        let page = selected_owner_page(
            &fixture,
            ListTopicsFilter {
                search: Some(search.into()),
                locale: Some(locale.into()),
                ..list_filter(20, None)
            },
        )
        .await
        .expect("localized selection");
        assert_eq!(
            page.items.len(),
            usize::from(expected),
            "locale={locale}, search={search}"
        );
        if expected {
            assert_eq!(page.items[0].id, topic);
        }
        if locale == "de" {
            assert_eq!(page.items[0].effective_locale, "en");
        }
    }
}

#[tokio::test]
async fn selection_cursors_reject_changed_query_order_solution_category_locale_viewer_and_tenant() {
    let mut fixture = fixture().await;
    let filter = ListTopicsFilter {
        search: Some("public".into()),
        ..list_filter(1, None)
    };
    let first = selected_owner_page(&fixture, filter.clone())
        .await
        .expect("selection");
    let cursor = first.next_cursor.expect("two public matches");
    let continuation = ListTopicsFilter {
        after: Some(cursor),
        ..filter
    };
    for changed in [
        ListTopicsFilter {
            search: Some("another".into()),
            ..continuation.clone()
        },
        ListTopicsFilter {
            order: rustok_forum::ForumTopicListOrder::Top,
            ..continuation.clone()
        },
        ListTopicsFilter {
            solved_only: true,
            ..continuation.clone()
        },
        ListTopicsFilter {
            category_id: Some(fixture.public_category),
            ..continuation.clone()
        },
        ListTopicsFilter {
            locale: Some("fr".into()),
            ..continuation.clone()
        },
    ] {
        assert!(matches!(
            selected_owner_page(&fixture, changed).await,
            Err(ForumError::Validation(_))
        ));
    }
    let resized = selected_owner_page(
        &fixture,
        ListTopicsFilter {
            per_page: 20,
            ..continuation.clone()
        },
    )
    .await
    .expect("page size is not a selection dimension");
    assert_eq!(resized.items.len(), 1);
    let user = fixture.low_trust_user_id;
    fixture.low_trust_user_id = fixture.trusted_user_id;
    assert!(matches!(
        selected_owner_page(&fixture, continuation.clone()).await,
        Err(ForumError::Validation(_))
    ));
    fixture.low_trust_user_id = user;
    fixture.tenant_id = Uuid::new_v4();
    assert!(matches!(
        selected_owner_page(&fixture, continuation).await,
        Err(ForumError::Validation(_))
    ));
}

#[tokio::test]
async fn selection_read_failures_do_not_become_successful_empty_pages() {
    let fixture = fixture().await;
    fixture
        .db
        .clone()
        .close()
        .await
        .expect("close fixture database");
    assert!(
        selected_owner_page(
            &fixture,
            ListTopicsFilter {
                search: Some("public".into()),
                ..list_filter(1, None)
            }
        )
        .await
        .is_err()
    );
}
