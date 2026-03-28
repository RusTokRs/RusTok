use sea_orm::{DatabaseConnection, TransactionTrait};
use tracing::instrument;
use uuid::Uuid;

use rustok_content::{
    BodyInput, CreateNodeInput, ListNodesFilter, NodeService, NodeTranslationInput,
    UpdateNodeInput, PLATFORM_FALLBACK_LOCALE,
};
use rustok_core::{prepare_content_payload, SecurityContext};
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use crate::constants::{topic_status, KIND_TOPIC};
use crate::dto::{
    CreateTopicInput, ListTopicsFilter, TopicListItem, TopicResponse, UpdateTopicInput,
};
use crate::error::{ForumError, ForumResult};
use crate::locale::{
    available_locales, resolve_body_with_fallback, resolve_translation_with_fallback,
};

pub struct TopicService {
    db: DatabaseConnection,
    nodes: NodeService,
    event_bus: TransactionalEventBus,
}

impl TopicService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            nodes: NodeService::new(db.clone(), event_bus.clone()),
            db,
            event_bus,
        }
    }

    #[instrument(skip(self, security, input))]
    pub async fn create(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreateTopicInput,
    ) -> ForumResult<TopicResponse> {
        if input.title.trim().is_empty() {
            return Err(ForumError::Validation(
                "Topic title cannot be empty".to_string(),
            ));
        }
        let create_format = input.body_format.as_str();
        if create_format != "rt_json_v1" && input.body.trim().is_empty() {
            return Err(ForumError::Validation(
                "Topic body cannot be empty".to_string(),
            ));
        }

        let author_id = security.user_id;
        let category_id = input.category_id;
        let locale = input.locale.clone();

        let prepared_body = prepare_content_payload(
            Some(&input.body_format),
            Some(&input.body),
            input.content_json.as_ref(),
            &locale,
            "Topic body",
        )
        .map_err(ForumError::Validation)?;

        let mut metadata = serde_json::json!({
            "tags": input.tags,
            "is_pinned": false,
            "is_locked": false,
            "reply_count": 0,
            "forum_status": topic_status::OPEN
        });

        if let Some(slugs) = input.channel_slugs {
            metadata["channel_visibility"] = serde_json::json!({ "allowed_channel_slugs": slugs });
        }

        let txn = self.db.begin().await?;

        let topic_id = self
            .nodes
            .create_node_in_tx(
                &txn,
                tenant_id,
                security.clone(),
                CreateNodeInput {
                    kind: KIND_TOPIC.to_string(),
                    status: Some(rustok_content::entities::node::ContentStatus::Published),
                    parent_id: Some(category_id),
                    author_id,
                    category_id: Some(category_id),
                    position: None,
                    depth: None,
                    reply_count: Some(0),
                    metadata,
                    translations: vec![NodeTranslationInput {
                        locale: locale.clone(),
                        title: Some(input.title.clone()),
                        slug: input.slug.clone(),
                        excerpt: None,
                    }],
                    bodies: vec![BodyInput {
                        locale: locale.clone(),
                        body: Some(prepared_body.body),
                        format: Some(prepared_body.format),
                    }],
                },
            )
            .await?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::ForumTopicCreated {
                    topic_id,
                    category_id,
                    author_id,
                    locale: locale.clone(),
                },
            )
            .await?;

        txn.commit().await?;

        let node = self.nodes.get_node(tenant_id, topic_id).await?;
        Ok(Self::node_to_topic(node, &locale))
    }

    #[instrument(skip(self))]
    pub async fn get(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        locale: &str,
    ) -> ForumResult<TopicResponse> {
        self.get_with_locale_fallback(tenant_id, topic_id, locale, None)
            .await
    }

    #[instrument(skip(self))]
    pub async fn get_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> ForumResult<TopicResponse> {
        let node = self.nodes.get_node(tenant_id, topic_id).await?;

        if node.kind != KIND_TOPIC {
            return Err(ForumError::TopicNotFound(topic_id));
        }

        Ok(Self::node_to_topic_with_fallback(
            node,
            locale,
            fallback_locale,
        ))
    }

    #[instrument(skip(self, security, input))]
    pub async fn update(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        input: UpdateTopicInput,
    ) -> ForumResult<TopicResponse> {
        let existing = self.get(tenant_id, topic_id, &input.locale).await?;
        let mut metadata = serde_json::json!({
            "tags": input.tags.unwrap_or(existing.tags.clone()),
            "is_pinned": existing.is_pinned,
            "is_locked": existing.is_locked,
            "reply_count": existing.reply_count,
            "forum_status": existing.status
        });

        if let Some(slugs) = input.channel_slugs {
            metadata["channel_visibility"] = serde_json::json!({ "allowed_channel_slugs": slugs });
        } else if !existing.channel_slugs.is_empty() {
            metadata["channel_visibility"] = serde_json::json!({ "allowed_channel_slugs": existing.channel_slugs });
        }

        let translations = if input.title.is_some() {
            Some(vec![NodeTranslationInput {
                locale: input.locale.clone(),
                title: Some(input.title.unwrap_or(existing.title.clone())),
                slug: Some(existing.slug.clone()),
                excerpt: None,
            }])
        } else {
            None
        };

        let bodies = if input.body.is_some()
            || input.content_json.is_some()
            || input.body_format.is_some()
        {
            let prepared_body = prepare_content_payload(
                input.body_format.as_deref(),
                input.body.as_deref(),
                input.content_json.as_ref(),
                &input.locale,
                "Topic body",
            )
            .map_err(ForumError::Validation)?;
            Some(vec![BodyInput {
                locale: input.locale.clone(),
                body: Some(prepared_body.body),
                format: Some(prepared_body.format),
            }])
        } else {
            None
        };

        let node = self
            .nodes
            .update_node(
                tenant_id,
                topic_id,
                security,
                UpdateNodeInput {
                    metadata: Some(metadata),
                    status: Some(rustok_content::entities::node::ContentStatus::Published),
                    translations,
                    bodies,
                    ..UpdateNodeInput::default()
                },
            )
            .await?;

        Ok(Self::node_to_topic(node, &input.locale))
    }

    #[instrument(skip(self, security))]
    pub async fn delete(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.nodes
            .delete_node(tenant_id, topic_id, security)
            .await?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn list(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        filter: ListTopicsFilter,
    ) -> ForumResult<(Vec<TopicListItem>, u64)> {
        self.list_with_locale_fallback(tenant_id, security, filter, None)
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn list_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        filter: ListTopicsFilter,
        fallback_locale: Option<&str>,
    ) -> ForumResult<(Vec<TopicListItem>, u64)> {
        let locale = filter
            .locale
            .clone()
            .or_else(|| fallback_locale.map(str::to_string))
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
        let (items, total) = self
            .nodes
            .list_nodes_with_locale_fallback(
                tenant_id,
                security,
                ListNodesFilter {
                    kind: Some(KIND_TOPIC.to_string()),
                    status: None,
                    parent_id: filter.category_id,
                    author_id: None,
                    locale: Some(locale.clone()),
                    page: filter.page,
                    per_page: filter.per_page,
                    include_deleted: false,
                    category_id: filter.category_id,
                },
                fallback_locale,
            )
            .await?;

        let list = items
            .into_iter()
            .map(|item| {
                let metadata = &item.metadata;
                TopicListItem {
                    id: item.id,
                    locale: locale.clone(),
                    effective_locale: item.effective_locale,
                    category_id: item.category_id.unwrap_or(Uuid::nil()),
                    author_id: item.author_id,
                    title: item.title.unwrap_or_default(),
                    slug: item.slug.unwrap_or_default(),
                    status: metadata
                        .get("forum_status")
                        .and_then(|v| v.as_str())
                        .unwrap_or(topic_status::OPEN)
                        .to_string(),
                    channel_slugs: metadata
                        .get("channel_visibility")
                        .and_then(|v| v.get("allowed_channel_slugs"))
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default(),
                    is_pinned: metadata
                        .get("is_pinned")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    is_locked: metadata
                        .get("is_locked")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    reply_count: metadata
                        .get("reply_count")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0) as i32,
                    created_at: item.created_at,
                }
            })
            .collect();

        Ok((list, total))
    }

    fn node_to_topic(node: rustok_content::NodeResponse, locale: &str) -> TopicResponse {
        Self::node_to_topic_with_fallback(node, locale, None)
    }

    fn node_to_topic_with_fallback(
        node: rustok_content::NodeResponse,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> TopicResponse {
        let resolved_tr =
            resolve_translation_with_fallback(&node.translations, locale, fallback_locale);
        let resolved_body = resolve_body_with_fallback(&node.bodies, locale, fallback_locale);
        let locales = available_locales(&node.translations);
        let metadata = node.metadata;

        let body = resolved_body
            .body
            .as_ref()
            .and_then(|b| b.body.clone())
            .unwrap_or_default();
        let body_format = resolved_body
            .body
            .as_ref()
            .map(|b| b.format.clone())
            .unwrap_or_else(|| "markdown".to_string());
        let content_json = if body_format == "rt_json_v1" {
            serde_json::from_str(&body).ok()
        } else {
            None
        };

        TopicResponse {
            id: node.id,
            requested_locale: locale.to_string(),
            locale: locale.to_string(),
            effective_locale: resolved_tr.effective_locale,
            available_locales: locales,
            category_id: node.category_id.unwrap_or(Uuid::nil()),
            author_id: node.author_id,
            title: resolved_tr
                .translation
                .and_then(|t| t.title.clone())
                .unwrap_or_default(),
            slug: resolved_tr
                .translation
                .and_then(|t| t.slug.clone())
                .unwrap_or_default(),
            body,
            body_format,
            content_json,
            status: metadata
                .get("forum_status")
                .and_then(|v| v.as_str())
                .unwrap_or(topic_status::OPEN)
                .to_string(),
            tags: metadata
                .get("tags")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            channel_slugs: metadata
                .get("channel_visibility")
                .and_then(|v| v.get("allowed_channel_slugs"))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            is_pinned: metadata
                .get("is_pinned")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            is_locked: metadata
                .get("is_locked")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            reply_count: metadata
                .get("reply_count")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32,
            created_at: node.created_at,
            updated_at: node.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{topic_status, KIND_TOPIC};
    use rustok_content::dto::{BodyResponse, NodeResponse, NodeTranslationResponse};
    use rustok_content::entities::node::ContentStatus;

    fn make_node(
        kind: &str,
        category_id: Option<Uuid>,
        author_id: Option<Uuid>,
        metadata: serde_json::Value,
        translations: Vec<NodeTranslationResponse>,
        bodies: Vec<BodyResponse>,
    ) -> NodeResponse {
        NodeResponse {
            id: Uuid::nil(),
            tenant_id: Uuid::nil(),
            kind: kind.to_string(),
            status: ContentStatus::Published,
            parent_id: category_id,
            author_id,
            category_id,
            position: 0,
            depth: 0,
            reply_count: 0,
            metadata,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            published_at: None,
            deleted_at: None,
            version: 1,
            translations,
            bodies,
        }
    }

    fn tr(locale: &str, title: &str, slug: &str) -> NodeTranslationResponse {
        NodeTranslationResponse {
            locale: locale.to_string(),
            title: Some(title.to_string()),
            slug: Some(slug.to_string()),
            excerpt: None,
        }
    }

    fn body(locale: &str, content: &str) -> BodyResponse {
        BodyResponse {
            locale: locale.to_string(),
            body: Some(content.to_string()),
            format: "markdown".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn node_to_topic_maps_fields_from_metadata() {
        let category_id = Uuid::new_v4();
        let author_id = Uuid::new_v4();
        let metadata = serde_json::json!({
            "tags": ["rust", "forum"],
            "is_pinned": true,
            "is_locked": false,
            "reply_count": 5,
            "forum_status": "open"
        });
        let node = make_node(
            KIND_TOPIC,
            Some(category_id),
            Some(author_id),
            metadata,
            vec![tr("en", "Hello World", "hello-world")],
            vec![body("en", "Body text")],
        );

        let result = TopicService::node_to_topic(node, "en");

        assert_eq!(result.title, "Hello World");
        assert_eq!(result.slug, "hello-world");
        assert_eq!(result.body, "Body text");
        assert_eq!(result.body_format, "markdown");
        assert!(result.content_json.is_none());
        assert_eq!(result.category_id, category_id);
        assert_eq!(result.author_id, Some(author_id));
        assert_eq!(result.tags, vec!["rust", "forum"]);
        assert!(result.is_pinned);
        assert!(!result.is_locked);
        assert_eq!(result.reply_count, 5);
        assert_eq!(result.status, "open");
        assert_eq!(result.effective_locale, "en");
        assert_eq!(result.available_locales, vec!["en"]);
    }

    #[test]
    fn node_to_topic_defaults_on_empty_metadata() {
        let node = make_node(
            KIND_TOPIC,
            None,
            None,
            serde_json::json!({}),
            vec![tr("en", "", "")],
            vec![],
        );

        let result = TopicService::node_to_topic(node, "en");

        assert_eq!(result.title, "");
        assert_eq!(result.body, "");
        assert_eq!(result.category_id, Uuid::nil());
        assert!(result.author_id.is_none());
        assert!(result.tags.is_empty());
        assert!(!result.is_pinned);
        assert!(!result.is_locked);
        assert_eq!(result.reply_count, 0);
        assert_eq!(result.status, topic_status::OPEN);
    }

    #[test]
    fn node_to_topic_falls_back_to_en() {
        let metadata = serde_json::json!({
            "tags": [],
            "is_pinned": false,
            "is_locked": false,
            "reply_count": 0,
            "forum_status": "open"
        });
        let node = make_node(
            KIND_TOPIC,
            None,
            None,
            metadata,
            vec![
                tr("en", "Fallback EN", "fallback-en"),
                tr("de", "Rückfall", "rueckfall"),
            ],
            vec![body("en", "English body")],
        );

        let result = TopicService::node_to_topic(node, "ru");
        assert_eq!(result.effective_locale, "en");
        assert_eq!(result.title, "Fallback EN");
    }

    #[test]
    fn node_to_topic_extracts_rt_json_content_json() {
        let rich = serde_json::json!({"version":"rt_json_v1","locale":"en","doc":{"type":"doc","content":[]}});
        let node = make_node(
            KIND_TOPIC,
            None,
            None,
            serde_json::json!({}),
            vec![tr("en", "Title", "title")],
            vec![BodyResponse {
                locale: "en".to_string(),
                body: Some(rich.to_string()),
                format: "rt_json_v1".to_string(),
                updated_at: "2024-01-01T00:00:00Z".to_string(),
            }],
        );

        let result = TopicService::node_to_topic(node, "en");
        assert_eq!(result.body_format, "rt_json_v1");
        assert_eq!(result.content_json, Some(rich));
    }

    #[test]
    fn node_to_topic_falls_back_to_first_when_no_en() {
        let metadata = serde_json::json!({
            "tags": [],
            "is_pinned": false,
            "is_locked": false,
            "reply_count": 0,
            "forum_status": "open"
        });
        let node = make_node(
            KIND_TOPIC,
            None,
            None,
            metadata,
            vec![tr("de", "Erster", "erster")],
            vec![],
        );

        let result = TopicService::node_to_topic(node, "en");
        assert_eq!(result.effective_locale, "de");
        assert_eq!(result.title, "Erster");
    }
}
