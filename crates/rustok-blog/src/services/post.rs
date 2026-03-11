use sea_orm::{DatabaseConnection, TransactionTrait};
use tracing::instrument;
use uuid::Uuid;

use rustok_content::{
    BodyInput, CreateNodeInput, ListNodesFilter, NodeService, NodeTranslationInput, UpdateNodeInput,
};
use rustok_core::{prepare_content_payload, DomainEvent, SecurityContext, CONTENT_FORMAT_MARKDOWN};
use rustok_outbox::TransactionalEventBus;
use serde_json::Value;

use crate::dto::{
    CreatePostInput, PostListQuery, PostListResponse, PostResponse, PostSummary, UpdatePostInput,
};
use crate::error::{BlogError, BlogResult};
use crate::locale::{available_locales, resolve_body, resolve_translation};
use crate::state_machine::BlogPostStatus;

const KIND_POST: &str = "post";

pub struct PostService {
    db: DatabaseConnection,
    nodes: NodeService,
    event_bus: TransactionalEventBus,
}

impl PostService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            nodes: NodeService::new(db.clone(), event_bus.clone()),
            db,
            event_bus,
        }
    }

    #[instrument(skip(self, security, input))]
    pub async fn create_post(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreatePostInput,
    ) -> BlogResult<Uuid> {
        if input.title.trim().is_empty() {
            return Err(BlogError::validation("Title cannot be empty"));
        }
        if input.title.len() > 512 {
            return Err(BlogError::validation("Title cannot exceed 512 characters"));
        }
        if input.locale.trim().is_empty() {
            return Err(BlogError::validation("Locale cannot be empty"));
        }
        if input.tags.len() > 20 {
            return Err(BlogError::validation("Cannot have more than 20 tags"));
        }

        let create_format = input.body_format.as_deref().unwrap_or("markdown");
        if create_format != "rt_json_v1" && input.body.trim().is_empty() {
            return Err(BlogError::validation("Body cannot be empty"));
        }

        let author_id = security.user_id.ok_or(BlogError::AuthorRequired)?;
        let locale = input.locale.clone();

        let prepared_body = prepare_content_payload(
            Some(&input.body_format),
            Some(&input.body),
            input.content_json.as_ref(),
            &locale,
            "Body",
        )
        .map_err(BlogError::validation)?;

        let mut metadata = input.metadata.unwrap_or_else(|| serde_json::json!({}));
        if let Value::Object(map) = &mut metadata {
            map.insert("tags".to_string(), serde_json::json!(input.tags));
            if let Some(cat_id) = input.category_id {
                map.insert("category_id".to_string(), serde_json::json!(cat_id));
            }
            if let Some(url) = &input.featured_image_url {
                map.insert("featured_image_url".to_string(), serde_json::json!(url));
            }
            if let Some(seo_title) = &input.seo_title {
                map.insert("seo_title".to_string(), serde_json::json!(seo_title));
            }
            if let Some(seo_desc) = &input.seo_description {
                map.insert("seo_description".to_string(), serde_json::json!(seo_desc));
            }
        } else {
            metadata = serde_json::json!({
                "tags": input.tags,
                "category_id": input.category_id,
                "featured_image_url": input.featured_image_url,
                "seo_title": input.seo_title,
                "seo_description": input.seo_description,
            });
        }

        let status = if input.publish {
            rustok_content::entities::node::ContentStatus::Published
        } else {
            rustok_content::entities::node::ContentStatus::Draft
        };

        let txn = self.db.begin().await.map_err(BlogError::from)?;

        let post_id = self
            .nodes
            .create_node_in_tx(
                &txn,
                tenant_id,
                security.clone(),
                CreateNodeInput {
                    kind: KIND_POST.to_string(),
                    status: Some(status),
                    parent_id: None,
                    author_id: Some(author_id),
                    category_id: input.category_id,
                    position: None,
                    depth: None,
                    reply_count: Some(0),
                    metadata,
                    translations: vec![NodeTranslationInput {
                        locale: locale.clone(),
                        title: Some(input.title),
                        slug: input.slug,
                        excerpt: input.excerpt,
                    }],
                    bodies: vec![BodyInput {
                        locale: locale.clone(),
                        body: Some(prepared_body.body),
                        format: Some(prepared_body.format),
                    }],
                },
            )
            .await
            .map_err(BlogError::from)?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::BlogPostCreated {
                    post_id,
                    author_id: Some(author_id),
                    locale,
                },
            )
            .await
            .map_err(BlogError::from)?;

        txn.commit().await.map_err(BlogError::from)?;

        Ok(post_id)
    }

    #[instrument(skip(self, security, input))]
    pub async fn update_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        security: SecurityContext,
        input: UpdatePostInput,
    ) -> BlogResult<()> {
        let locale = input.locale.clone().unwrap_or_else(|| "en".to_string());
        let mut update = UpdateNodeInput::default();

        if input.title.is_some() || input.slug.is_some() || input.excerpt.is_some() {
            update.translations = Some(vec![NodeTranslationInput {
                locale: locale.clone(),
                title: input.title,
                slug: input.slug,
                excerpt: input.excerpt,
            }]);
        }

        if input.body.is_some() || input.content_json.is_some() || input.body_format.is_some() {
            let prepared_body = prepare_content_payload(
                input.body_format.as_deref(),
                input.body.as_deref(),
                input.content_json.as_ref(),
                &locale,
                "Body",
            )
            .map_err(BlogError::validation)?;
            update.bodies = Some(vec![BodyInput {
                locale: locale.clone(),
                body: Some(prepared_body.body),
                format: Some(prepared_body.format),
            }]);
        }

        if input.tags.is_some()
            || input.category_id.is_some()
            || input.metadata.is_some()
            || input.featured_image_url.is_some()
            || input.seo_title.is_some()
            || input.seo_description.is_some()
        {
            let mut metadata = input.metadata.unwrap_or_else(|| serde_json::json!({}));
            if let Value::Object(map) = &mut metadata {
                if let Some(tags) = input.tags {
                    map.insert("tags".to_string(), serde_json::json!(tags));
                }
                if let Some(cat_id) = input.category_id {
                    map.insert("category_id".to_string(), serde_json::json!(cat_id));
                }
                if let Some(url) = input.featured_image_url {
                    map.insert("featured_image_url".to_string(), serde_json::json!(url));
                }
                if let Some(seo_title) = input.seo_title {
                    map.insert("seo_title".to_string(), serde_json::json!(seo_title));
                }
                if let Some(seo_desc) = input.seo_description {
                    map.insert("seo_description".to_string(), serde_json::json!(seo_desc));
                }
            }
            update.metadata = Some(metadata);
        }

        if let Some(version) = input.version {
            update.expected_version = Some(version);
        }

        let txn = self.db.begin().await.map_err(BlogError::from)?;

        self.nodes
            .update_node_in_tx(&txn, tenant_id, post_id, security.clone(), update)
            .await
            .map_err(BlogError::from)?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::BlogPostUpdated { post_id, locale },
            )
            .await
            .map_err(BlogError::from)?;

        txn.commit().await.map_err(BlogError::from)?;

        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn publish_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        security: SecurityContext,
    ) -> BlogResult<()> {
        let node = self
            .nodes
            .get_node(tenant_id, post_id)
            .await
            .map_err(BlogError::from)?;
        let author_id = node.author_id;

        let txn = self.db.begin().await.map_err(BlogError::from)?;

        self.nodes
            .publish_node_in_tx(&txn, tenant_id, post_id, security.clone())
            .await
            .map_err(BlogError::from)?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::BlogPostPublished { post_id, author_id },
            )
            .await
            .map_err(BlogError::from)?;

        txn.commit().await.map_err(BlogError::from)?;

        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn unpublish_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        security: SecurityContext,
    ) -> BlogResult<()> {
        let txn = self.db.begin().await.map_err(BlogError::from)?;

        self.nodes
            .unpublish_node_in_tx(&txn, tenant_id, post_id, security.clone())
            .await
            .map_err(BlogError::from)?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::BlogPostUnpublished { post_id },
            )
            .await
            .map_err(BlogError::from)?;

        txn.commit().await.map_err(BlogError::from)?;

        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn archive_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        security: SecurityContext,
        reason: Option<String>,
    ) -> BlogResult<()> {
        let txn = self.db.begin().await.map_err(BlogError::from)?;

        self.nodes
            .archive_node_in_tx(&txn, tenant_id, post_id, security.clone())
            .await
            .map_err(BlogError::from)?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::BlogPostArchived {
                    post_id,
                    reason: reason.clone(),
                },
            )
            .await
            .map_err(BlogError::from)?;

        txn.commit().await.map_err(BlogError::from)?;

        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn delete_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        security: SecurityContext,
    ) -> BlogResult<()> {
        let node = self
            .nodes
            .get_node(tenant_id, post_id)
            .await
            .map_err(BlogError::from)?;
        let status = map_content_status(node.status.clone());
        if status == BlogPostStatus::Published {
            return Err(BlogError::CannotDeletePublished);
        }

        let txn = self.db.begin().await.map_err(BlogError::from)?;

        self.nodes
            .delete_node_in_tx(&txn, tenant_id, post_id, security.clone())
            .await
            .map_err(BlogError::from)?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::BlogPostDeleted { post_id },
            )
            .await
            .map_err(BlogError::from)?;

        txn.commit().await.map_err(BlogError::from)?;

        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn get_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        locale: &str,
    ) -> BlogResult<PostResponse> {
        let node = self
            .nodes
            .get_node(tenant_id, post_id)
            .await
            .map_err(BlogError::from)?;

        let tr = resolve_translation(&node.translations, locale);
        let br = resolve_body(&node.bodies, locale);
        let all_locales = available_locales(&node.translations);

        let translation = tr.translation;
        let body_resp = br.body;

        let tags = node
            .metadata
            .get("tags")
            .and_then(|t| t.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let category_id = node
            .metadata
            .get("category_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok());

        let featured_image_url = node
            .metadata
            .get("featured_image_url")
            .and_then(|v| v.as_str())
            .map(String::from);

        let seo_title = node
            .metadata
            .get("seo_title")
            .and_then(|v| v.as_str())
            .map(String::from);

        let seo_description = node
            .metadata
            .get("seo_description")
            .and_then(|v| v.as_str())
            .map(String::from);

        let body = body_resp.and_then(|b| b.body.clone()).unwrap_or_default();
        let body_format = body_resp
            .map(|b| b.format.clone())
            .unwrap_or_else(|| "markdown".to_string());
        let content_json = if body_format == "rt_json_v1" {
            serde_json::from_str(&body).ok()
        } else {
            None
        };

        Ok(PostResponse {
            id: node.id,
            tenant_id: node.tenant_id,
            author_id: node.author_id.unwrap_or_default(),
            title: translation
                .and_then(|t| t.title.clone())
                .unwrap_or_default(),
            slug: translation.and_then(|t| t.slug.clone()).unwrap_or_default(),
            locale: locale.to_string(),
            effective_locale: tr.effective_locale,
            available_locales: all_locales,
            body: body_resp.and_then(|b| b.body.clone()).unwrap_or_default(),
            body_format: body_resp
                .map(|b| b.format.clone())
                .unwrap_or_else(|| CONTENT_FORMAT_MARKDOWN.to_string()),
            excerpt: translation.and_then(|t| t.excerpt.clone()),
            status: map_content_status(node.status),
            category_id,
            category_name: None,
            tags,
            featured_image_url,
            seo_title,
            seo_description,
            metadata: node.metadata,
            comment_count: node.reply_count as i64,
            view_count: 0,
            created_at: node
                .created_at
                .parse()
                .unwrap_or_else(|_| chrono::Utc::now()),
            updated_at: node
                .updated_at
                .parse()
                .unwrap_or_else(|_| chrono::Utc::now()),
            published_at: node.published_at.and_then(|p| p.parse().ok()),
            version: node.version,
        })
    }

    #[instrument(skip(self, security))]
    pub async fn list_posts(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        query: PostListQuery,
    ) -> BlogResult<PostListResponse> {
        let locale = query.locale.clone().unwrap_or_else(|| "en".to_string());

        let filter = ListNodesFilter {
            kind: Some(KIND_POST.to_string()),
            status: query.status.map(map_blog_status_to_content),
            locale: query.locale.clone(),
            author_id: query.author_id,
            category_id: query.category_id,
            page: query.page() as u64,
            per_page: query.per_page() as u64,
            ..Default::default()
        };

        let (node_list, total) = self
            .nodes
            .list_nodes(tenant_id, security.clone(), filter)
            .await
            .map_err(BlogError::from)?;

        let mut items = Vec::with_capacity(node_list.len());
        for item in node_list {
            let tags = item
                .metadata
                .get("tags")
                .and_then(|t| t.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            let category_id = item.category_id.or_else(|| {
                item.metadata
                    .get("category_id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| Uuid::parse_str(s).ok())
            });

            let featured_image_url = item
                .metadata
                .get("featured_image_url")
                .and_then(|v| v.as_str())
                .map(String::from);

            items.push(PostSummary {
                id: item.id,
                title: item.title.unwrap_or_default(),
                slug: item.slug.unwrap_or_default(),
                locale: locale.clone(),
                effective_locale: locale.clone(),
                excerpt: item.excerpt,
                status: map_content_status(item.status),
                author_id: item.author_id.unwrap_or_default(),
                author_name: None,
                category_id,
                category_name: None,
                tags,
                featured_image_url,
                comment_count: 0,
                published_at: item.published_at.and_then(|p| p.parse().ok()),
                created_at: item
                    .created_at
                    .parse()
                    .unwrap_or_else(|_| chrono::Utc::now()),
            });
        }

        Ok(PostListResponse::new(items, total, &query))
    }

    pub async fn get_posts_by_tag(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        tag: String,
        page: u32,
        per_page: u32,
    ) -> BlogResult<PostListResponse> {
        let query = PostListQuery {
            tag: Some(tag),
            page: Some(page),
            per_page: Some(per_page),
            ..Default::default()
        };
        self.list_posts(tenant_id, security, query).await
    }

    pub async fn get_posts_by_category(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        category_id: Uuid,
        page: u32,
        per_page: u32,
    ) -> BlogResult<PostListResponse> {
        let query = PostListQuery {
            category_id: Some(category_id),
            page: Some(page),
            per_page: Some(per_page),
            ..Default::default()
        };
        self.list_posts(tenant_id, security, query).await
    }

    pub async fn get_posts_by_author(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        author_id: Uuid,
        page: u32,
        per_page: u32,
    ) -> BlogResult<PostListResponse> {
        let query = PostListQuery {
            author_id: Some(author_id),
            page: Some(page),
            per_page: Some(per_page),
            ..Default::default()
        };
        self.list_posts(tenant_id, security, query).await
    }
}

fn map_content_status(status: rustok_content::entities::node::ContentStatus) -> BlogPostStatus {
    match status {
        rustok_content::entities::node::ContentStatus::Draft => BlogPostStatus::Draft,
        rustok_content::entities::node::ContentStatus::Published => BlogPostStatus::Published,
        rustok_content::entities::node::ContentStatus::Archived => BlogPostStatus::Archived,
    }
}

fn map_blog_status_to_content(
    status: BlogPostStatus,
) -> rustok_content::entities::node::ContentStatus {
    match status {
        BlogPostStatus::Draft => rustok_content::entities::node::ContentStatus::Draft,
        BlogPostStatus::Published => rustok_content::entities::node::ContentStatus::Published,
        BlogPostStatus::Archived => rustok_content::entities::node::ContentStatus::Archived,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_roundtrip_draft() {
        let s = map_content_status(rustok_content::entities::node::ContentStatus::Draft);
        assert_eq!(s, BlogPostStatus::Draft);
        let back = map_blog_status_to_content(s);
        assert!(matches!(
            back,
            rustok_content::entities::node::ContentStatus::Draft
        ));
    }

    #[test]
    fn status_roundtrip_published() {
        let s = map_content_status(rustok_content::entities::node::ContentStatus::Published);
        assert_eq!(s, BlogPostStatus::Published);
        let back = map_blog_status_to_content(s);
        assert!(matches!(
            back,
            rustok_content::entities::node::ContentStatus::Published
        ));
    }

    #[test]
    fn status_roundtrip_archived() {
        let s = map_content_status(rustok_content::entities::node::ContentStatus::Archived);
        assert_eq!(s, BlogPostStatus::Archived);
        let back = map_blog_status_to_content(s);
        assert!(matches!(
            back,
            rustok_content::entities::node::ContentStatus::Archived
        ));
    }

    #[test]
    fn post_list_query_defaults() {
        let query = PostListQuery::default();
        assert_eq!(query.page(), 1);
        assert_eq!(query.per_page(), 20);
        assert_eq!(query.offset(), 0);
    }

    #[test]
    fn post_list_query_pagination() {
        let query = PostListQuery {
            page: Some(3),
            per_page: Some(10),
            ..Default::default()
        };
        assert_eq!(query.page(), 3);
        assert_eq!(query.per_page(), 10);
        assert_eq!(query.offset(), 20);
    }

    #[test]
    fn post_list_query_clamps_bounds() {
        let query = PostListQuery {
            page: Some(0),
            per_page: Some(200),
            ..Default::default()
        };
        assert_eq!(query.page(), 1);
        assert_eq!(query.per_page(), 100);
    }

    #[test]
    fn create_post_input_has_new_fields() {
        let input = CreatePostInput {
            locale: "ru".to_string(),
            title: "Заголовок".to_string(),
            body: "Тело поста".to_string(),
            excerpt: Some("Краткое содержание".to_string()),
            slug: Some("zagolovok".to_string()),
            publish: false,
            tags: vec!["rust".to_string()],
            category_id: None,
            featured_image_url: Some("https://cdn.example.com/img.jpg".to_string()),
            seo_title: Some("SEO заголовок".to_string()),
            seo_description: Some("SEO описание".to_string()),
            metadata: None,
        };
        assert_eq!(input.locale, "ru");
        assert!(input.featured_image_url.is_some());
        assert!(input.seo_title.is_some());
        assert!(input.seo_description.is_some());
    }

    #[test]
    fn update_post_input_defaults_to_none() {
        let input = UpdatePostInput::default();
        assert!(input.locale.is_none());
        assert!(input.title.is_none());
        assert!(input.featured_image_url.is_none());
        assert!(input.seo_title.is_none());
        assert!(input.seo_description.is_none());
        assert!(input.version.is_none());
    }
}
