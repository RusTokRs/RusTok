use std::sync::Arc;

use rustok_blog::{
    BlogModule, BlogPostStatus, CategoryService, CreateCategoryInput, CreatePostInput,
    PostListQuery, PostService,
};
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

async fn setup_blog_test_db() -> DatabaseConnection {
    let db_url = format!(
        "sqlite:file:blog_post_category_name_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(db_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    Database::connect(options)
        .await
        .expect("blog category-name sqlite database should connect")
}

async fn ensure_schema(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
    SysEventsMigration
        .up(&manager)
        .await
        .expect("outbox migration should apply");
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("taxonomy migration should apply");
    }
    for migration in BlogModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("blog migration should apply");
    }
}

fn event_bus(db: &DatabaseConnection) -> TransactionalEventBus {
    TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())))
}

#[tokio::test]
async fn post_category_name_projects_across_detail_and_list_paths() {
    let db = setup_blog_test_db().await;
    ensure_schema(&db).await;
    let manager = SchemaManager::new(&db);
    assert!(
        !manager
            .has_table("blog_category_translations")
            .await
            .expect("legacy translation table lookup should succeed"),
        "post Category projection contract must run after donor storage retirement"
    );

    let bus = event_bus(&db);
    let category_service = CategoryService::new(db.clone(), bus.clone());
    let post_service = PostService::new(db.clone(), bus);
    let tenant_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let category_id = category_service
        .create(
            tenant_id,
            admin.clone(),
            CreateCategoryInput {
                locale: "de".to_string(),
                name: "Nachrichten".to_string(),
                slug: Some("nachrichten".to_string()),
                description: None,
                parent_id: None,
                position: Some(0),
                settings: serde_json::json!({}),
            },
        )
        .await
        .expect("category should be created");

    let post_id = post_service
        .create_post(
            tenant_id,
            admin.clone(),
            CreatePostInput {
                locale: "de".to_string(),
                title: "Kategorie im Post".to_string(),
                content: rustok_blog::richtext::article_document_from_plain_text("Body"),
                excerpt: None,
                slug: Some("kategorie-im-post".to_string()),
                publish: true,
                tags: Vec::new(),
                category_id: Some(category_id),
                featured_image_url: None,
                seo_title: None,
                seo_description: None,
                channel_slugs: None,
                metadata: None,
            },
        )
        .await
        .expect("post should be created");

    let detail = post_service
        .get_post_with_locale_fallback(tenant_id, admin.clone(), post_id, "fr", Some("de"))
        .await
        .expect("detail should resolve Taxonomy category name through fallback");
    assert_eq!(detail.category_id, Some(category_id));
    assert_eq!(detail.category_name.as_deref(), Some("Nachrichten"));

    let listed = post_service
        .list_posts_with_locale_fallback(
            tenant_id,
            admin,
            PostListQuery {
                locale: Some("fr".to_string()),
                page: Some(1),
                per_page: Some(10),
                ..Default::default()
            },
            Some("de"),
        )
        .await
        .expect("authenticated list should resolve Taxonomy category name");
    assert_eq!(listed.items.len(), 1);
    assert_eq!(listed.items[0].category_id, Some(category_id));
    assert_eq!(
        listed.items[0].category_name.as_deref(),
        Some("Nachrichten")
    );

    let public = post_service
        .list_public_visible_with_locale_fallback(
            tenant_id,
            PostListQuery {
                status: Some(BlogPostStatus::Published),
                locale: Some("fr".to_string()),
                page: Some(1),
                per_page: Some(10),
                ..Default::default()
            },
            Some("de"),
            None,
        )
        .await
        .expect("public list should resolve Taxonomy category name");
    assert_eq!(public.items.len(), 1);
    assert_eq!(public.items[0].category_id, Some(category_id));
    assert_eq!(
        public.items[0].category_name.as_deref(),
        Some("Nachrichten")
    );
}
