use std::collections::HashMap;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use rustok_api::Patch;
use rustok_blog::{
    ports::{blog_post, blog_post_translation},
    CategoryService as BlogCategoryService, CreateCategoryInput, CreatePostInput, PostService,
    UpdatePostInput,
};
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;

use crate::error::StarterResult;
use crate::model::{BlogPostStarter, TaxonomyCategoryStarter};

/// Imports blog categories using the canonical Blog CategoryService (which synchronizes with Taxonomy).
pub async fn import_blog_categories(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    security: &SecurityContext,
    categories: &[TaxonomyCategoryStarter],
    locale: &str,
) -> StarterResult<(HashMap<String, Uuid>, usize, usize)> {
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

/// Imports blog posts with rich-text content, tags, category relations, and multi-locale translations.
pub async fn import_blog_posts(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    security: &SecurityContext,
    posts: &[BlogPostStarter],
    category_map: &HashMap<String, Uuid>,
    locale: &str,
) -> StarterResult<(usize, usize)> {
    let post_service = PostService::new(db.clone(), event_bus.clone());
    let mut created = 0;
    let mut skipped = 0;

    for post in posts {
        // Idempotency: check by slug in tenant scope
        let existing = blog_post::Entity::find()
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .filter(blog_post::Column::Slug.eq(&post.slug))
            .one(db)
            .await?;

        let (post_id, mut current_version) = if let Some(existing) = existing {
            tracing::info!(slug = %post.slug, "Blog post already exists, skipping creation");
            skipped += 1;
            (existing.id, existing.version)
        } else {
            let category_id = post
                .category_slug
                .as_ref()
                .and_then(|slug| category_map.get(slug).copied());

            let content = rustok_blog::richtext::article_document_from_plain_text(&post.content_markdown);

            let new_id = post_service
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
            (new_id, 1)
        };

        // Import additional translations declared on the post
        for (trans_locale, translation) in &post.translations {
            if trans_locale == locale {
                continue;
            }

            let trans_exists = blog_post_translation::Entity::find()
                .filter(blog_post_translation::Column::PostId.eq(post_id))
                .filter(blog_post_translation::Column::Locale.eq(trans_locale.as_str()))
                .one(db)
                .await?;

            if trans_exists.is_none() {
                let content = rustok_blog::richtext::article_document_from_plain_text(&translation.content_markdown);
                let update_input = UpdatePostInput {
                    locale: Some(trans_locale.clone()),
                    title: Some(translation.title.clone()),
                    content: Some(content),
                    excerpt: match translation.excerpt.clone() {
                        Some(e) => Patch::Set(e),
                        None => Patch::Keep,
                    },
                    slug: None,
                    tags: None,
                    category_id: Patch::Keep,
                    featured_image_url: Patch::Keep,
                    seo_title: Patch::Set(translation.title.clone()),
                    seo_description: match translation.excerpt.clone() {
                        Some(e) => Patch::Set(e),
                        None => Patch::Keep,
                    },
                    channel_slugs: None,
                    metadata: None,
                    version: current_version,
                };

                if let Err(e) = post_service
                    .update_post(tenant_id, post_id, security.clone(), update_input)
                    .await
                {
                    tracing::warn!(error = ?e, locale = %trans_locale, slug = %post.slug, "Failed to apply blog post translation");
                } else {
                    current_version += 1;
                }
            }
        }
    }

    Ok((created, skipped))
}
