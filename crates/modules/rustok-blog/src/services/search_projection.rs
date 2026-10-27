use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use async_trait::async_trait;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect};
use serde_json::json;
use uuid::Uuid;

use rustok_api::PLATFORM_FALLBACK_LOCALE;
use rustok_core::search_projection::{
    MAX_SEARCH_PROJECTION_PAGE_SIZE, SearchProjectionDocument, SearchProjectionPage,
    SearchProjectionSource, SearchProjectionSourceFactory,
};
use rustok_core::{Error, ModuleRuntimeExtensions, Result};
use rustok_profiles::{ProfileSummaryAudience, ProfileSummaryReader};

use crate::dto::PublishedPostCursor;
use crate::entities::{blog_post, blog_post_channel_visibility, blog_post_translation};
use crate::services::category_name_projection::{
    BlogCategoryProjectionInfo, load_category_projections_map,
};
use crate::services::post::{
    order_public_post_list, public_post_after_condition, public_post_cursor,
};
use crate::services::tag::load_post_tags_map;

pub const BLOG_SOURCE_MODULE: &str = "blog";
pub const BLOG_POST_ENTITY_TYPE: &str = "blog_post";

#[derive(Clone, Default)]
pub struct BlogSearchProjectionSourceFactory;

impl SearchProjectionSourceFactory for BlogSearchProjectionSourceFactory {
    fn source_module(&self) -> &'static str {
        BLOG_SOURCE_MODULE
    }

    fn build(&self, db: DatabaseConnection) -> Arc<dyn SearchProjectionSource> {
        Arc::new(BlogSearchProjectionSource::new(db, None))
    }

    fn build_with_extensions(
        &self,
        db: DatabaseConnection,
        extensions: &ModuleRuntimeExtensions,
    ) -> Arc<dyn SearchProjectionSource> {
        let profiles_reader = extensions.get::<Arc<dyn ProfileSummaryReader>>().cloned();
        Arc::new(BlogSearchProjectionSource::new(db, profiles_reader))
    }
}

#[derive(Clone)]
pub struct BlogSearchProjectionSource {
    db: DatabaseConnection,
    profiles_reader: Option<Arc<dyn ProfileSummaryReader>>,
}

impl BlogSearchProjectionSource {
    pub fn new(
        db: DatabaseConnection,
        profiles_reader: Option<Arc<dyn ProfileSummaryReader>>,
    ) -> Self {
        Self {
            db,
            profiles_reader,
        }
    }

    async fn project_posts(
        &self,
        tenant_id: Uuid,
        posts: &[blog_post::Model],
    ) -> Result<Vec<SearchProjectionDocument>> {
        if posts.is_empty() {
            return Ok(Vec::new());
        }

        let post_ids = posts.iter().map(|p| p.id).collect::<Vec<_>>();

        // 1. Load translations. The translation table carries no tenant
        // column; tenant isolation is inherited from the caller, which only
        // ever passes posts already filtered by `tenant_id` (see
        // `list_public_documents`), so these post ids cannot cross tenants.
        let translations = blog_post_translation::Entity::find()
            .filter(blog_post_translation::Column::PostId.is_in(post_ids.clone()))
            .all(&self.db)
            .await
            .map_err(Error::Database)?;

        let mut translations_by_post: HashMap<Uuid, Vec<blog_post_translation::Model>> =
            HashMap::new();
        let mut locales = BTreeSet::new();
        for translation in translations {
            locales.insert(translation.locale.clone());
            translations_by_post
                .entry(translation.post_id)
                .or_default()
                .push(translation);
        }

        // 2. Load channel visibility
        let channel_visibilities = blog_post_channel_visibility::Entity::find()
            .filter(blog_post_channel_visibility::Column::TenantId.eq(tenant_id))
            .filter(blog_post_channel_visibility::Column::PostId.is_in(post_ids.clone()))
            .all(&self.db)
            .await
            .map_err(Error::Database)?;

        let mut channels_by_post: HashMap<Uuid, Vec<String>> = HashMap::new();
        for vis in channel_visibilities {
            channels_by_post
                .entry(vis.post_id)
                .or_default()
                .push(vis.channel_slug);
        }

        // 3. Collect the Category identities of the whole page
        let category_ids: Vec<Uuid> = posts.iter().filter_map(|p| p.category_id).collect();

        // 4. Load authors
        let author_ids: Vec<Uuid> = posts.iter().map(|p| p.author_id).collect();
        let author_names = self.load_author_names(tenant_id, &author_ids).await?;

        // 5. Load the localized Category and Tag projections once per locale.
        //    Reading them per (post x locale) multiplied the Taxonomy and Tag
        //    queries by the page size.
        let fallback_locale = Some(PLATFORM_FALLBACK_LOCALE);
        let mut category_maps_by_locale: HashMap<
            String,
            HashMap<Uuid, BlogCategoryProjectionInfo>,
        > = HashMap::new();
        let mut tag_maps_by_locale: HashMap<String, HashMap<Uuid, Vec<String>>> = HashMap::new();
        for locale in &locales {
            let category_map = load_category_projections_map(
                &self.db,
                tenant_id,
                &category_ids,
                locale,
                fallback_locale,
            )
            .await
            .map_err(|error| Error::External(error.to_string()))?;
            category_maps_by_locale.insert(locale.clone(), category_map);

            let tag_map =
                load_post_tags_map(&self.db, tenant_id, &post_ids, locale, fallback_locale)
                    .await
                    .map_err(|error| Error::External(error.to_string()))?;
            tag_maps_by_locale.insert(locale.clone(), tag_map);
        }

        let mut documents = Vec::new();

        for post in posts {
            let Some(post_translations) = translations_by_post.get(&post.id) else {
                continue;
            };

            let post_channels = channels_by_post.get(&post.id).cloned().unwrap_or_default();
            let author_name = author_names.get(&post.author_id).cloned();

            for translation in post_translations {
                let locale = &translation.locale;

                // Category projection for this specific locale
                let category_info = post.category_id.and_then(|cat_id| {
                    category_maps_by_locale
                        .get(locale)
                        .and_then(|cat_map| cat_map.get(&cat_id))
                        .cloned()
                });

                let category_name = category_info.as_ref().map(|c| c.name.clone());
                let category_slug = category_info.as_ref().map(|c| c.slug.clone());

                // Tags for this specific post and locale
                let post_tags = tag_maps_by_locale
                    .get(locale)
                    .and_then(|tags_map| tags_map.get(&post.id))
                    .cloned()
                    .unwrap_or_default();

                let keywords_text = [
                    category_name.as_deref().unwrap_or(""),
                    author_name.as_deref().unwrap_or(""),
                    translation.seo_title.as_deref().unwrap_or(""),
                    translation.seo_description.as_deref().unwrap_or(""),
                    &post_tags.join(" "),
                ]
                .iter()
                .filter(|s| !s.trim().is_empty())
                .copied()
                .collect::<Vec<_>>()
                .join(" ");

                // The searchable body is the article's plain text, not the
                // excerpt: indexing only the excerpt made full-text search
                // blind to everything written in the article itself. A stored
                // body that no longer satisfies the article contract degrades
                // to the excerpt with a warning instead of aborting the page.
                let body_text = match crate::richtext::project_stored_article(&translation.body) {
                    Ok((_, text)) => text,
                    Err(error) => {
                        tracing::warn!(
                            tenant_id = %tenant_id,
                            post_id = %post.id,
                            locale = %translation.locale,
                            %error,
                            "Blog Search projection could not project a stored article body; indexing the excerpt instead"
                        );
                        translation.excerpt.clone().unwrap_or_default()
                    }
                };

                documents.push(SearchProjectionDocument {
                    document_key: format!("blog_post:{}:{}", post.id, translation.locale),
                    tenant_id,
                    document_id: post.id,
                    source_module: BLOG_SOURCE_MODULE.to_string(),
                    entity_type: BLOG_POST_ENTITY_TYPE.to_string(),
                    locale: translation.locale.clone(),
                    status: post.status.clone(),
                    is_public: post.status == "published" && post.published_at.is_some(),
                    title: translation.title.clone(),
                    subtitle: category_name.clone(),
                    slug: Some(post.slug.clone()),
                    handle: None,
                    body: body_text,
                    keywords_text,
                    facets: json!({
                        "has_category": post.category_id.is_some(),
                        "has_tags": !post_tags.is_empty(),
                        "has_channels": !post_channels.is_empty(),
                        "channel_slugs": post_channels,
                    }),
                    payload: json!({
                        "slug": post.slug,
                        "excerpt": translation.excerpt,
                        "seo_title": translation.seo_title,
                        "seo_description": translation.seo_description,
                        "featured_image_url": post.featured_image_url,
                        "category_id": post.category_id,
                        "category_name": category_name,
                        "category_slug": category_slug,
                        "author_id": post.author_id,
                        "author_name": author_name,
                        "tags": post_tags,
                        "channel_slugs": post_channels,
                        "comment_count": post.comment_count,
                        "version": post.version,
                        "published_at": post.published_at,
                        "archived_at": post.archived_at,
                    }),
                    published_at: post.published_at.map(Into::into),
                    created_at: post.created_at.into(),
                    updated_at: post.updated_at.max(translation.updated_at).into(),
                });
            }
        }

        Ok(documents)
    }

    async fn load_author_names(
        &self,
        tenant_id: Uuid,
        author_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, String>> {
        if author_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let Some(reader) = &self.profiles_reader else {
            // The Profiles owner port is the only sanctioned source of public author
            // names. Reading the platform `users` table directly would publish the
            // account name of a profile whose visibility forbids it, because the
            // Profiles privacy decision is not applied there.
            tracing::warn!(
                tenant_id = %tenant_id,
                author_count = author_ids.len(),
                "Blog Search projection has no Profiles owner port; public author names are omitted",
            );
            return Ok(HashMap::new());
        };

        let summaries = match reader
            .find_profile_summaries(
                tenant_id,
                author_ids,
                None,
                Some(PLATFORM_FALLBACK_LOCALE),
                ProfileSummaryAudience::Anonymous,
            )
            .await
        {
            Ok(summaries) => summaries,
            Err(error) => {
                // A failing Profiles reader must not abort the whole projection, but
                // the degradation has to be visible in the platform log.
                tracing::warn!(
                    tenant_id = %tenant_id,
                    author_count = author_ids.len(),
                    error = %error,
                    "Blog Search projection could not resolve public author names",
                );
                return Ok(HashMap::new());
            }
        };

        let mut result = HashMap::new();
        for (user_id, summary) in summaries {
            let name = if summary.display_name.trim().is_empty() {
                summary.handle
            } else {
                summary.display_name
            };
            result.insert(user_id, name);
        }
        Ok(result)
    }
}

#[async_trait]
impl SearchProjectionSource for BlogSearchProjectionSource {
    fn source_module(&self) -> &'static str {
        BLOG_SOURCE_MODULE
    }

    async fn list_public_documents(
        &self,
        tenant_id: Uuid,
        after: Option<String>,
        limit: usize,
    ) -> Result<SearchProjectionPage> {
        if !(1..=MAX_SEARCH_PROJECTION_PAGE_SIZE).contains(&limit) {
            return Err(Error::Validation(format!(
                "Blog Search projection page size must be between 1 and {MAX_SEARCH_PROJECTION_PAGE_SIZE}"
            )));
        }

        let mut select = blog_post::Entity::find()
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .filter(blog_post::Column::Status.eq("published"));

        if let Some(token) = after.as_deref() {
            let cursor = PublishedPostCursor::decode(token)
                .map_err(|e| Error::Validation(format!("Invalid blog post cursor: {e}")))?;
            select = select.filter(public_post_after_condition(&cursor));
        }

        select = order_public_post_list(select);
        // Fetch one extra row: `posts.len() == limit` alone cannot tell a
        // full-but-final page from a page with a successor, and used to emit
        // a spurious cursor that resolved to an empty page.
        let mut posts = select
            .limit(limit as u64 + 1)
            .all(&self.db)
            .await
            .map_err(Error::Database)?;

        let has_next_page = posts.len() > limit;
        posts.truncate(limit);
        let next_cursor = if has_next_page {
            posts
                .last()
                .and_then(|p| public_post_cursor(p).ok())
                .map(|c| c.encode())
        } else {
            None
        };

        let documents = self.project_posts(tenant_id, &posts).await?;

        Ok(SearchProjectionPage {
            documents,
            next_cursor,
        })
    }

    async fn load_public_entity(
        &self,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
    ) -> Result<Vec<SearchProjectionDocument>> {
        if entity_type != BLOG_POST_ENTITY_TYPE {
            return Ok(Vec::new());
        }

        let post = blog_post::Entity::find_by_id(entity_id)
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .filter(blog_post::Column::Status.eq("published"))
            .one(&self.db)
            .await
            .map_err(Error::Database)?;

        let Some(post) = post else {
            return Ok(Vec::new());
        };

        self.project_posts(tenant_id, &[post]).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factory_exposes_blog_module() {
        let factory = BlogSearchProjectionSourceFactory;
        assert_eq!(factory.source_module(), "blog");
    }
}
