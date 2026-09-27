use std::collections::HashMap;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use rustok_blog::{
    CategoryService as BlogCategoryService, CreateCategoryInput, CreatePostInput, PostService,
};
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;

use crate::model::{BlogPostStarter, TaxonomyCategoryStarter};

/// Imports blog categories using the canonical Blog CategoryService (which synchronizes with Taxonomy).
pub async fn import_blog_categories(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    security: &SecurityContext,
    categories: &[TaxonomyCategoryStarter],
    locale: &str,
) -> Result<(HashMap<String, Uuid>, usize, usize), Box<dyn std::error::Error + Send + Sync>> {
    let cat_service = BlogCategoryService::new(db.clone(), event_bus.clone());
    let mut slug_to_id = HashMap::new();
    let mut created = 0;
    let mut skipped = 0;

    for (pos, cat) in categories.iter().enumerate() {
        // Idempotency: check if category already exists in taxonomy terms
        let existing = rustok_taxonomy::entities::taxonomy_term::Entity::find()
            .filter(rustok_taxonomy::entities::taxonomy_term::Column::TenantId.eq(tenant_id))
            .filter(rustok_taxonomy::entities::taxonomy_term::Column::CanonicalKey.eq(&cat.slug))
            .one(db)
            .await?;

        if let Some(term) = existing {
            tracing::info!(slug = %cat.slug, "Blog category already exists, reusing ID");
            slug_to_id.insert(cat.slug.clone(), term.id);
            skipped += 1;
            continue;
        }

        let id = cat_service
            .create(
                tenant_id,
                security.clone(),
                CreateCategoryInput {
                    locale: locale.to_string(),
                    name: cat.name.clone(),
                    slug: Some(cat.slug.clone()),
                    description: cat.description.clone(),
                    parent_id: None,
                    position: Some(pos as i32 + 1),
                    settings: serde_json::json!({}),
                },
            )
            .await?;

        slug_to_id.insert(cat.slug.clone(), id);
        created += 1;
    }

    Ok((slug_to_id, created, skipped))
}

/// Imports blog posts with rich-text content, tags, and category relations.
pub async fn import_blog_posts(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    security: &SecurityContext,
    posts: &[BlogPostStarter],
    category_map: &HashMap<String, Uuid>,
    locale: &str,
) -> Result<(usize, usize), Box<dyn std::error::Error + Send + Sync>> {
    let post_service = PostService::new(db.clone(), event_bus.clone());
    let mut created = 0;
    let mut skipped = 0;

    for post in posts {
        // Idempotency: check by slug through canonical PostService query
        if post_service
            .get_post_by_slug(tenant_id, security.clone(), &post.slug, locale)
            .await
            .is_ok()
        {
            tracing::info!(slug = %post.slug, "Blog post already exists, skipping");
            skipped += 1;
            continue;
        }

        let category_id = post
            .category_slug
            .as_ref()
            .and_then(|slug| category_map.get(slug).copied());

        let content = rustok_blog::richtext::article_document_from_plain_text(&post.content_markdown);

        post_service
            .create_post(
                tenant_id,
                security.clone(),
                CreatePostInput {
                    locale: locale.to_string(),
                    title: post.title.clone(),
                    content,
                    excerpt: post.excerpt.clone(),
                    slug: Some(post.slug.clone()),
                    publish: post.publish,
                    tags: post.tags.clone(),
                    category_id,
                    featured_image_url: post.featured_image_url.clone(),
                    seo_title: Some(post.title.clone()),
                    seo_description: post.excerpt.clone(),
                    channel_slugs: None,
                    metadata: None,
                },
            )
            .await?;

        created += 1;
    }

    Ok((created, skipped))
}
