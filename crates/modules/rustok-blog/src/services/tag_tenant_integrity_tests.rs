use std::sync::Arc;

use chrono::Utc;
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use rustok_test_utils::setup_test_db;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

use crate::{CreatePostInput, PostService, entities::blog_post_tag};

async fn setup_schema() -> sea_orm::DatabaseConnection {
    let db = setup_test_db().await;
    let manager = SchemaManager::new(&db);

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
    for migration in crate::migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("blog migration should apply");
    }

    db
}

fn create_input(slug: &str, tag: &str) -> CreatePostInput {
    CreatePostInput {
        locale: "en".to_string(),
        title: format!("Post {slug}"),
        content: crate::richtext::article_document_from_plain_text("Body"),
        excerpt: None,
        slug: Some(slug.to_string()),
        publish: false,
        tags: vec![tag.to_string()],
        category_id: None,
        featured_image_url: None,
        seo_title: None,
        seo_description: None,
        channel_slugs: None,
        metadata: None,
    }
}

#[tokio::test]
async fn storage_rejects_cross_tenant_blog_post_tag_attachment() {
    let db = setup_schema().await;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let service = PostService::new(db.clone(), event_bus);

    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();
    let admin_a = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let admin_b = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let post_a = service
        .create_post(tenant_a, admin_a, create_input("tenant-a", "alpha"))
        .await
        .expect("tenant A post should be created");
    let post_b = service
        .create_post(tenant_b, admin_b, create_input("tenant-b", "beta"))
        .await
        .expect("tenant B post should be created");

    let relation_a = blog_post_tag::Entity::find()
        .filter(blog_post_tag::Column::TenantId.eq(tenant_a))
        .filter(blog_post_tag::Column::PostId.eq(post_a))
        .one(&db)
        .await
        .expect("tenant A relation query should succeed")
        .expect("tenant A relation should exist");
    let relation_b = blog_post_tag::Entity::find()
        .filter(blog_post_tag::Column::TenantId.eq(tenant_b))
        .filter(blog_post_tag::Column::PostId.eq(post_b))
        .one(&db)
        .await
        .expect("tenant B relation query should succeed")
        .expect("tenant B relation should exist");

    assert_eq!(relation_a.tenant_id, tenant_a);
    assert_eq!(relation_b.tenant_id, tenant_b);

    let error = blog_post_tag::ActiveModel {
        post_id: Set(post_a),
        tag_id: Set(relation_b.tag_id),
        tenant_id: Set(tenant_a),
        created_at: Set(Utc::now().into()),
    }
    .insert(&db)
    .await
    .expect_err("cross-tenant tag attachment must be rejected by storage");

    assert!(
        error.to_string().contains("tenant")
            || error.to_string().contains("foreign key")
            || error.to_string().contains("constraint")
    );
}
