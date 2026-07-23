use std::sync::Arc;

use rustok_blog::{
    BlogModule, CreatePostInput, ListTagsFilter, PostService, TagService, entities::blog_post_tag,
};
use rustok_core::{MemoryTransport, MigrationSource, SecurityContext, UserRole};
use rustok_outbox::TransactionalEventBus;
use rustok_taxonomy::{
    CreateTaxonomyTermInput, TaxonomyModule, TaxonomyScopeType, TaxonomyService, TaxonomyTermKind,
    entities::taxonomy_term,
};
use sea_orm::{
    ColumnTrait, ConnectOptions, Database, DatabaseConnection, EntityTrait, QueryFilter,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup_blog_test_db() -> DatabaseConnection {
    let db_url = format!(
        "sqlite:file:blog_taxonomy_tags_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut opts = ConnectOptions::new(db_url);
    opts.max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);

    Database::connect(opts)
        .await
        .expect("failed to connect blog sqlite database")
}

async fn setup() -> (
    DatabaseConnection,
    TransactionalEventBus,
    tokio::sync::broadcast::Receiver<rustok_events::EventEnvelope>,
    Uuid,
) {
    let db = setup_blog_test_db().await;
    let schema = SchemaManager::new(&db);
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("taxonomy migration should apply");
    }
    for migration in BlogModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("blog migration should apply");
    }

    let transport = MemoryTransport::new();
    let receiver = transport.subscribe();
    let event_bus = TransactionalEventBus::new(Arc::new(transport));
    (db, event_bus, receiver, Uuid::new_v4())
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

#[tokio::test]
async fn post_tags_create_blog_scoped_taxonomy_terms_and_usage_counts() {
    let (db, event_bus, _events, tenant_id) = setup().await;
    let post_service = PostService::new(db.clone(), event_bus);
    let tag_service = TagService::new(db.clone());
    let security = admin();

    let post_id = post_service
        .create_post(
            tenant_id,
            security.clone(),
            CreatePostInput {
                locale: "en".to_string(),
                title: "Tagged post".to_string(),
                body: "Body".to_string(),
                body_format: "markdown".to_string(),
                content_json: None,
                content: None,
                excerpt: None,
                slug: Some("tagged-post".to_string()),
                publish: true,
                tags: vec![
                    "rust".to_string(),
                    "backend".to_string(),
                    "rust".to_string(),
                ],
                category_id: None,
                featured_image_url: None,
                seo_title: None,
                seo_description: None,
                channel_slugs: None,
                metadata: None,
            },
        )
        .await
        .expect("post should be created");

    let post = post_service
        .get_post(tenant_id, security.clone(), post_id, "en")
        .await
        .expect("post should load");
    assert_eq!(post.tags.len(), 2);
    assert!(post.tags.contains(&"rust".to_string()));
    assert!(post.tags.contains(&"backend".to_string()));

    let post_tags = blog_post_tag::Entity::find()
        .filter(blog_post_tag::Column::PostId.eq(post_id))
        .all(&db)
        .await
        .expect("post tag relations should load");
    assert_eq!(post_tags.len(), 2);

    let terms = taxonomy_term::Entity::find()
        .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
        .all(&db)
        .await
        .expect("taxonomy terms should load");
    assert_eq!(terms.len(), 2);
    assert!(
        terms
            .iter()
            .all(|term| term.scope_type == TaxonomyScopeType::Module)
    );
    assert!(terms.iter().all(|term| term.scope_value == "blog"));

    let (tags, total) = tag_service
        .list_tags(
            tenant_id,
            security,
            ListTagsFilter {
                locale: Some("en".to_string()),
                page: 1,
                per_page: 10,
            },
        )
        .await
        .expect("blog tags should list");
    assert_eq!(total, 2);
    assert!(tags.iter().all(|item| item.use_count == 1));
}

#[tokio::test]
async fn post_tag_sync_reuses_existing_global_taxonomy_term() {
    let (db, event_bus, _events, tenant_id) = setup().await;
    let post_service = PostService::new(db.clone(), event_bus);
    let taxonomy_service = TaxonomyService::new(db.clone());
    let security = admin();

    let global_rust_term_id = taxonomy_service
        .create_term(
            tenant_id,
            security.clone(),
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

    let post_id = post_service
        .create_post(
            tenant_id,
            security,
            CreatePostInput {
                locale: "en".to_string(),
                title: "Global tag reuse".to_string(),
                body: "Body".to_string(),
                body_format: "markdown".to_string(),
                content_json: None,
                content: None,
                excerpt: None,
                slug: Some("global-tag-reuse".to_string()),
                publish: true,
                tags: vec!["rust".to_string(), "backend".to_string()],
                category_id: None,
                featured_image_url: None,
                seo_title: None,
                seo_description: None,
                channel_slugs: None,
                metadata: None,
            },
        )
        .await
        .expect("post should be created");

    let attached_term_ids = blog_post_tag::Entity::find()
        .filter(blog_post_tag::Column::PostId.eq(post_id))
        .all(&db)
        .await
        .expect("blog post tags should load")
        .into_iter()
        .map(|row| row.tag_id)
        .collect::<Vec<_>>();
    assert!(attached_term_ids.contains(&global_rust_term_id));

    let blog_scoped_terms = taxonomy_term::Entity::find()
        .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Module))
        .filter(taxonomy_term::Column::ScopeValue.eq("blog"))
        .all(&db)
        .await
        .expect("blog-scoped terms should load");
    assert_eq!(blog_scoped_terms.len(), 1);
    assert_eq!(blog_scoped_terms[0].canonical_key, "backend");
}
