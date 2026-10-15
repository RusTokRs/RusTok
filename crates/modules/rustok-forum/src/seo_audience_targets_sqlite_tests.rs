use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{
    PortError, SharedStaticModuleSettingsReader, SharedStaticModuleSettingsTransactionReader,
    StaticModuleSettingsReader, StaticModuleSettingsSnapshot,
    StaticModuleSettingsTransactionReader,
};
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseTransaction, Statement};
use sea_orm_migration::SchemaManager;

use super::*;
use crate::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumAudienceConstraints,
    ForumCategoryAudiencePolicyService, ForumModule, ForumSettingsProviders,
    ForumTopicAudiencePolicyService, SetForumCategoryAudiencePolicyInput,
    SetForumTopicAudiencePolicyInput, TopicListCursor, TopicService,
};

struct OpenForum;

#[async_trait]
impl StaticModuleSettingsReader for OpenForum {
    async fn settings(
        &self,
        _tenant: Uuid,
        slug: &str,
    ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
        assert_eq!(slug, "forum");
        Ok(Some(StaticModuleSettingsSnapshot {
            enabled: true,
            settings: serde_json::json!({
                "allow_anonymous_reading": true,
                "rate_limit_new_topic_seconds": 0,
                "rate_limit_new_reply_seconds": 0,
                "use_reactions": false,
            }),
        }))
    }
}

#[async_trait]
impl StaticModuleSettingsTransactionReader for OpenForum {
    async fn settings_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        tenant: Uuid,
        slug: &str,
    ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
        self.settings(tenant, slug).await
    }
}

fn page_request<'a>(
    tenant_id: Uuid,
    after: Option<&'a str>,
) -> SeoTargetBulkPageRequest<'a> {
    SeoTargetBulkPageRequest {
        tenant_id,
        default_locale: "en",
        locale: "en",
        after,
        limit: 1,
    }
}

#[tokio::test]
async fn open_forum_seo_loads_topics_and_pages_only_public_owner_candidates() {
    let database_url = format!(
        "sqlite:file:forum_public_seo_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(database_url);
    options.max_connections(5).min_connections(1).sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("SEO fixture database");
    db.execute_unprepared(
        "CREATE TABLE users (id TEXT NOT NULL PRIMARY KEY, tenant_id TEXT NOT NULL)",
    )
    .await
    .expect("platform users fixture");
    let schema = SchemaManager::new(&db);
    for migration in OutboxModule.migrations() {
        migration.up(&schema).await.expect("outbox migration");
    }
    for migration in TaxonomyModule.migrations() {
        migration.up(&schema).await.expect("taxonomy migration");
    }
    for migration in ForumModule.migrations() {
        migration.up(&schema).await.expect("forum migration");
    }

    let tenant = Uuid::new_v4();
    let admin_id = Uuid::new_v4();
    db.execute_raw(Statement::from_sql_and_values(
        sea_orm::DatabaseBackend::Sqlite,
        "INSERT INTO users (id, tenant_id) VALUES (?1, ?2)",
        vec![admin_id.into(), tenant.into()],
    ))
    .await
    .expect("SEO admin fixture");
    let admin = SecurityContext::new(UserRole::Admin, Some(admin_id));
    let reader = SharedStaticModuleSettingsReader(Arc::new(OpenForum));
    let settings = ForumSettingsProviders::default().with_static_readers(
        reader.clone(),
        SharedStaticModuleSettingsTransactionReader(Arc::new(OpenForum)),
    );
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));

    let mut categories = Vec::new();
    for number in 0..4 {
        let category = CategoryService::new(db.clone())
            .create(
                tenant,
                admin.clone(),
                CreateCategoryInput {
                    locale: "en".into(),
                    name: format!("Category {number}"),
                    slug: format!("category-{number}"),
                    description: None,
                    icon: None,
                    color: None,
                    parent_id: None,
                    position: Some(0),
                    moderated: false,
                },
            )
            .await
            .expect("SEO category fixture");
        categories.push(category.id);
    }
    categories.sort(); // Category SEO keysets are ordered by id.
    let hidden_category = categories[1];
    ForumCategoryAudiencePolicyService::new(db.clone())
        .set(
            tenant,
            hidden_category,
            admin.clone(),
            SetForumCategoryAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    roles_any: vec![UserRole::Customer],
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("category audience fixture");

    for number in 0..3 {
        TopicService::new(db.clone(), event_bus.clone())
            .with_settings_providers(settings.clone())
            .create(
                tenant,
                admin.clone(),
                CreateTopicInput {
                    locale: "en".into(),
                    category_id: categories[0],
                    title: format!("Topic {number}"),
                    slug: Some(format!("topic-{number}")),
                    body: rustok_api::RichTextDocument::single_paragraph("Public SEO fixture"),
                    metadata: serde_json::json!({}),
                    tags: Vec::new(),
                    channel_slugs: Some(vec!["web".into()]),
                },
            )
            .await
            .expect("SEO topic fixture");
    }
    let ordered_topics = ForumTopicAudienceListService::new(db.clone(), event_bus.clone())
        .with_settings_providers(settings)
        .list_public_storefront_visible_with_locale_fallback(
            tenant,
            ListTopicsFilter {
                category_id: None,
                status: None,
                locale: Some("en".into()),
                after: None,
                per_page: 10,
            },
            Some("en"),
            None,
        )
        .await
        .expect("unrestricted SEO topic list");
    assert_eq!(ordered_topics.items.len(), 3);
    let hidden_topic = ordered_topics.items[1].id;
    ForumTopicAudiencePolicyService::new(db.clone())
        .set(
            tenant,
            hidden_topic,
            admin,
            SetForumTopicAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    roles_any: vec![UserRole::Customer],
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("topic audience fixture");
    let mut runtime = SeoTargetRuntimeContext {
        db,
        event_bus,
        static_settings_reader: Some(reader),
    };

    let visible_topic = ForumTopicSeoTargetProvider
        .load_target(
            &runtime,
            SeoTargetLoadRequest {
                tenant_id: tenant,
                default_locale: "en",
                locale: "en",
                target_id: ordered_topics.items[0].id,
                scope: SeoTargetLoadScope::PublicRoute,
                channel_slug: None,
            },
        )
        .await
        .expect("public topic SEO load must receive settings");
    assert!(visible_topic.is_some());
    assert!(ForumTopicSeoTargetProvider
        .load_target(
            &runtime,
            SeoTargetLoadRequest {
                tenant_id: tenant,
                default_locale: "en",
                locale: "en",
                target_id: hidden_topic,
                scope: SeoTargetLoadScope::PublicRoute,
                channel_slug: None,
            },
        )
        .await
        .expect("hidden topic SEO load")
        .is_none());
    assert!(ForumCategorySeoTargetProvider
        .load_target(
            &runtime,
            SeoTargetLoadRequest {
                tenant_id: tenant,
                default_locale: "en",
                locale: "en",
                target_id: hidden_category,
                scope: SeoTargetLoadScope::PublicRoute,
                channel_slug: None,
            },
        )
        .await
        .expect("hidden category SEO load")
        .is_none());

    let topic_sitemap = ForumTopicSeoTargetProvider
        .sitemap_candidates(
            &runtime,
            SeoTargetSitemapRequest {
                tenant_id: tenant,
                default_locale: "en",
            },
        )
        .await
        .expect("public topic sitemap must receive settings");
    assert_eq!(topic_sitemap.len(), 2);
    assert!(!topic_sitemap
        .iter()
        .any(|candidate| candidate.target_id == hidden_topic));
    let category_sitemap = ForumCategorySeoTargetProvider
        .sitemap_candidates(
            &runtime,
            SeoTargetSitemapRequest {
                tenant_id: tenant,
                default_locale: "en",
            },
        )
        .await
        .expect("public category sitemap");
    assert_eq!(category_sitemap.len(), 3);
    assert!(!category_sitemap
        .iter()
        .any(|candidate| candidate.target_id == hidden_category));

    let first_category_page = ForumCategorySeoTargetProvider
        .list_bulk_summaries_page(&runtime, page_request(tenant, None))
        .await
        .expect("first public category SEO page");
    assert_eq!(first_category_page.items.len(), 1);
    assert_eq!(first_category_page.items[0].target_id, categories[0]);
    assert_eq!(
        first_category_page
            .next_cursor
            .as_deref()
            .map(Uuid::parse_str)
            .transpose()
            .unwrap(),
        Some(categories[0])
    );
    let second_category_page = ForumCategorySeoTargetProvider
        .list_bulk_summaries_page(
            &runtime,
            page_request(tenant, first_category_page.next_cursor.as_deref()),
        )
        .await
        .expect("second public category SEO page");
    assert_eq!(second_category_page.items.len(), 1);
    assert_eq!(second_category_page.items[0].target_id, categories[2]);
    assert_eq!(
        second_category_page
            .next_cursor
            .as_deref()
            .map(Uuid::parse_str)
            .transpose()
            .unwrap(),
        Some(categories[2])
    );
    let last_category_page = ForumCategorySeoTargetProvider
        .list_bulk_summaries_page(
            &runtime,
            page_request(tenant, second_category_page.next_cursor.as_deref()),
        )
        .await
        .expect("last public category SEO page");
    assert_eq!(last_category_page.items.len(), 1);
    assert_eq!(last_category_page.items[0].target_id, categories[3]);
    assert!(last_category_page.next_cursor.is_none());

    let first_topic_page = ForumTopicSeoTargetProvider
        .list_bulk_summaries_page(&runtime, page_request(tenant, None))
        .await
        .expect("first public topic SEO page");
    assert_eq!(first_topic_page.items.len(), 1);
    assert_eq!(first_topic_page.items[0].target_id, ordered_topics.items[0].id);
    assert_eq!(
        TopicListCursor::decode(first_topic_page.next_cursor.as_deref().expect("public topic cursor"))
            .expect("topic cursor")
            .id,
        ordered_topics.items[0].id
    );
    let last_topic_page = ForumTopicSeoTargetProvider
        .list_bulk_summaries_page(
            &runtime,
            page_request(tenant, first_topic_page.next_cursor.as_deref()),
        )
        .await
        .expect("last public topic SEO page");
    assert_eq!(last_topic_page.items.len(), 1);
    assert_eq!(last_topic_page.items[0].target_id, ordered_topics.items[2].id);
    assert!(last_topic_page.next_cursor.is_none());

    runtime.static_settings_reader = None;
    assert!(ForumTopicSeoTargetProvider
        .sitemap_candidates(
            &runtime,
            SeoTargetSitemapRequest {
                tenant_id: tenant,
                default_locale: "en",
            },
        )
        .await
        .is_err());
    assert!(ForumTopicSeoTargetProvider
        .load_target(
            &runtime,
            SeoTargetLoadRequest {
                tenant_id: tenant,
                default_locale: "en",
                locale: "en",
                target_id: ordered_topics.items[0].id,
                scope: SeoTargetLoadScope::PublicRoute,
                channel_slug: None,
            },
        )
        .await
        .is_err());
    assert!(ForumTopicSeoTargetProvider
        .load_target(
            &runtime,
            SeoTargetLoadRequest {
                tenant_id: tenant,
                default_locale: "en",
                locale: "en",
                target_id: hidden_topic,
                scope: SeoTargetLoadScope::Authoring,
                channel_slug: None,
            },
        )
        .await
        .expect("authoring SEO must not depend on a public settings reader")
        .is_some());
}
