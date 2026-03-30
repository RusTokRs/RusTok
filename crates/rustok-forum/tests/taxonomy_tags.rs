use std::sync::Arc;

use rustok_core::{MemoryTransport, MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    entities::{forum_topic, forum_topic_tag},
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumModule, TopicService,
};
use rustok_outbox::TransactionalEventBus;
use rustok_taxonomy::{
    entities::taxonomy_term, CreateTaxonomyTermInput, TaxonomyModule, TaxonomyScopeType,
    TaxonomyService, TaxonomyTermKind,
};
use sea_orm::{
    ColumnTrait, ConnectOptions, Database, DatabaseConnection, EntityTrait, QueryFilter,
};
use sea_orm_migration::SchemaManager;
use tokio::sync::broadcast;
use uuid::Uuid;

async fn setup_forum_test_db() -> DatabaseConnection {
    let db_url = format!(
        "sqlite:file:forum_taxonomy_tags_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut opts = ConnectOptions::new(db_url);
    opts.max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);

    Database::connect(opts)
        .await
        .expect("failed to connect forum sqlite database")
}

async fn setup() -> (
    DatabaseConnection,
    TransactionalEventBus,
    broadcast::Receiver<rustok_events::EventEnvelope>,
    Uuid,
) {
    let db = setup_forum_test_db().await;
    let schema = SchemaManager::new(&db);
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

    let transport = MemoryTransport::new();
    let receiver = transport.subscribe();
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    (db, event_bus, receiver, Uuid::new_v4())
}

async fn create_category(
    service: &CategoryService,
    tenant_id: Uuid,
    security: SecurityContext,
) -> rustok_forum::CategoryResponse {
    service
        .create(
            tenant_id,
            security,
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "General".to_string(),
                slug: "general".to_string(),
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
async fn topic_tags_are_synced_into_forum_topic_tags_without_legacy_json() {
    let (db, event_bus, _events, tenant_id) = setup().await;
    let category_service = CategoryService::new(db.clone());
    let topic_service = TopicService::new(db.clone(), event_bus);
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let category = create_category(&category_service, tenant_id, admin.clone()).await;
    let topic = topic_service
        .create(
            tenant_id,
            admin,
            CreateTopicInput {
                locale: "en".to_string(),
                category_id: category.id,
                title: "Tagged topic".to_string(),
                slug: None,
                body: "Body".to_string(),
                body_format: "markdown".to_string(),
                content_json: None,
                tags: vec![
                    "rust".to_string(),
                    "backend".to_string(),
                    "rust".to_string(),
                ],
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created");

    assert_eq!(topic.tags, vec!["backend".to_string(), "rust".to_string()]);

    let topic_tags = forum_topic_tag::Entity::find()
        .filter(forum_topic_tag::Column::TopicId.eq(topic.id))
        .all(&db)
        .await
        .expect("topic tags should load");
    assert_eq!(topic_tags.len(), 2);

    let terms = taxonomy_term::Entity::find()
        .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
        .all(&db)
        .await
        .expect("taxonomy terms should load");
    assert_eq!(terms.len(), 2);
    assert!(terms
        .iter()
        .all(|term| term.scope_type == TaxonomyScopeType::Module));
    assert!(terms.iter().all(|term| term.scope_value == "forum"));

    let stored_topic = forum_topic::Entity::find_by_id(topic.id)
        .one(&db)
        .await
        .expect("topic row should load")
        .expect("topic row must exist");
    assert_eq!(stored_topic.id, topic.id);
}

#[tokio::test]
async fn topic_tag_sync_reuses_existing_global_taxonomy_term() {
    let (db, event_bus, _events, tenant_id) = setup().await;
    let category_service = CategoryService::new(db.clone());
    let topic_service = TopicService::new(db.clone(), event_bus);
    let taxonomy_service = TaxonomyService::new(db.clone());
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let global_rust_term_id = taxonomy_service
        .create_term(
            tenant_id,
            admin.clone(),
            CreateTaxonomyTermInput {
                kind: TaxonomyTermKind::Tag,
                scope_type: TaxonomyScopeType::Global,
                scope_value: None,
                locale: "en".to_string(),
                name: "rust".to_string(),
                slug: None,
                canonical_key: None,
                description: None,
                aliases: vec![],
            },
        )
        .await
        .expect("global term should be created");

    let category = create_category(&category_service, tenant_id, admin.clone()).await;
    let topic = topic_service
        .create(
            tenant_id,
            admin,
            CreateTopicInput {
                locale: "en".to_string(),
                category_id: category.id,
                title: "Global tag reuse".to_string(),
                slug: None,
                body: "Body".to_string(),
                body_format: "markdown".to_string(),
                content_json: None,
                tags: vec!["rust".to_string(), "backend".to_string()],
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created");

    assert_eq!(topic.tags, vec!["backend".to_string(), "rust".to_string()]);

    let topic_tag_term_ids = forum_topic_tag::Entity::find()
        .filter(forum_topic_tag::Column::TopicId.eq(topic.id))
        .all(&db)
        .await
        .expect("topic tags should load")
        .into_iter()
        .map(|row| row.term_id)
        .collect::<Vec<_>>();
    assert!(topic_tag_term_ids.contains(&global_rust_term_id));

    let forum_scoped_terms = taxonomy_term::Entity::find()
        .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Module))
        .filter(taxonomy_term::Column::ScopeValue.eq("forum"))
        .all(&db)
        .await
        .expect("forum-scoped terms should load");
    assert_eq!(forum_scoped_terms.len(), 1);
    assert_eq!(forum_scoped_terms[0].canonical_key, "backend");
}
