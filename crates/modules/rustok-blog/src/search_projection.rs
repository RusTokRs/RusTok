use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, QueryFilter,
    QuerySelect, Statement,
};
use serde_json::json;
use uuid::Uuid;

use rustok_api::PLATFORM_FALLBACK_LOCALE;
use rustok_core::search_projection::{
    MAX_SEARCH_PROJECTION_PAGE_SIZE, SearchProjectionDocument, SearchProjectionPage,
    SearchProjectionSource, SearchProjectionSourceFactory,
};
use rustok_core::{Error, ModuleRuntimeExtensions, Result};
use rustok_profiles_api::ProfileSummaryReader;

use crate::dto::PublishedPostCursor;
use crate::entities::{blog_post, blog_post_channel_visibility, blog_post_translation};
use crate::services::category_name_projection::load_category_projections_map;
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
        let profiles_reader = extensions
            .get::<Arc<dyn ProfileSummaryReader>>()
            .cloned();
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

        // 1. Load translations
        let translations = blog_post_translation::Entity::find()
            .filter(blog_post_translation::Column::PostId.is_in(post_ids.clone()))
            .all(&self.db)
            .await
            .map_err(Error::Database)?;

        let mut translations_by_post: HashMap<Uuid, Vec<blog_post_translation::Model>> =
            HashMap::new();
        for translation in translations {
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

        // 3. Load categories
        let category_ids: Vec<Uuid> = posts
            .iter()
            .filter_map(|p| p.category_id)
            .collect();

        // 4. Load authors
        let author_ids: Vec<Uuid> = posts.iter().map(|p| p.author_id).collect();
        let author_names = self.load_author_names(tenant_id, &author_ids).await?;

        let mut documents = Vec::new();

        for post in posts {
            let Some(post_translations) = translations_by_post.get(&post.id) else {
                continue;
            };

            let post_channels = channels_by_post.get(&post.id).cloned().unwrap_or_default();
            let author_name = author_names.get(&post.author_id).cloned();

            for translation in post_translations {
                let locale = &translation.locale;
                let fallback_locale = Some(PLATFORM_FALLBACK_LOCALE);

                // Category projection for this specific locale
                let category_info = if let Some(cat_id) = post.category_id {
                    let cat_map = load_category_projections_map(
                        &self.db,
                        tenant_id,
                        &category_ids,
                        locale,
                        fallback_locale,
                    )
                    .await
                    .map_err(|e| Error::External(e.to_string()))?;
                    cat_map.get(&cat_id).cloned()
                } else {
                    None
                };

                let category_name = category_info.as_ref().map(|c| c.name.clone());
                let category_slug = category_info.as_ref().map(|c| c.slug.clone());

                // Tags for this specific post and locale
                let tags_map = load_post_tags_map(
                    &self.db,
                    tenant_id,
                    &[post.id],
                    locale,
                    fallback_locale,
                )
                .await
                .map_err(|e| Error::External(e.to_string()))?;
                let post_tags = tags_map.get(&post.id).cloned().unwrap_or_default();

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

                documents.push(SearchProjectionDocument {
                    document_key: format!("blog_post:{}:{}", post.id, translation.locale),
                    tenant_id,
                    document_id: post.id,
                    source_module: BLOG_SOURCE_MODULE.to_string(),
                    entity_type: BLOG_POST_ENTITY_TYPE.to_string(),
                    locale: translation.locale.clone(),
                    status: post.status.clone(),
                    is_public: post.status == "published",
                    title: translation.title.clone(),
                    subtitle: category_name.clone(),
                    slug: Some(post.slug.clone()),
                    handle: None,
                    body: translation.excerpt.clone().unwrap_or_default(),
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

        if let Some(reader) = &self.profiles_reader {
            let summaries = reader
                .find_profile_summaries(
                    tenant_id,
                    author_ids,
                    None,
                    Some(PLATFORM_FALLBACK_LOCALE),
                    rustok_profiles_api::ProfileSummaryAudience::Anonymous,
                )
                .await
                .unwrap_or_default();
            let mut result = HashMap::new();
            for (user_id, s) in summaries {
                let name = if s.display_name.trim().is_empty() {
                    s.handle
                } else {
                    s.display_name
                };
                result.insert(user_id, name);
            }
            return Ok(result);
        }

        if self.db.get_database_backend() == DbBackend::Postgres {
            let stmt = Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT id, name
                FROM users
                WHERE tenant_id = $1
                  AND id = ANY($2)
                  AND LOWER(status::text) = 'active'
                "#,
                vec![tenant_id.into(), author_ids.to_vec().into()],
            );
            if let Ok(rows) = self.db.query_all_raw(stmt).await {
                let mut map = HashMap::new();
                for row in rows {
                    if let (Ok(id), Ok(name)) = (
                        row.try_get::<Uuid>("", "id"),
                        row.try_get::<String>("", "name"),
                    ) {
                        map.insert(id, name);
                    }
                }
                return Ok(map);
            }
        }

        Ok(HashMap::new())
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
        let posts = select
            .limit(limit as u64)
            .all(&self.db)
            .await
            .map_err(Error::Database)?;

        let next_cursor = if posts.len() == limit {
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
