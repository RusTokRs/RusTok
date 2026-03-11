use sea_orm::{DatabaseConnection, TransactionTrait};
use tracing::instrument;
use uuid::Uuid;

use rustok_content::{BodyInput, CreateNodeInput, ListNodesFilter, NodeService, UpdateNodeInput};
use rustok_core::{prepare_content_payload, DomainEvent, SecurityContext};
use rustok_outbox::TransactionalEventBus;

use crate::constants::{reply_status, topic_status, KIND_REPLY, KIND_TOPIC};
use crate::dto::{
    CreateReplyInput, ListRepliesFilter, ReplyListItem, ReplyResponse, UpdateReplyInput,
};
use crate::error::{ForumError, ForumResult};
use crate::locale::resolve_body;

pub struct ReplyService {
    db: DatabaseConnection,
    nodes: NodeService,
    event_bus: TransactionalEventBus,
}

impl ReplyService {
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
        topic_id: Uuid,
        input: CreateReplyInput,
    ) -> ForumResult<ReplyResponse> {
        let topic_node = self.nodes.get_node(tenant_id, topic_id).await?;
        if topic_node.kind != KIND_TOPIC {
            return Err(ForumError::TopicNotFound(topic_id));
        }

        let topic_status_value = topic_node
            .metadata
            .get("forum_status")
            .and_then(|v| v.as_str())
            .unwrap_or(topic_status::OPEN);

        if topic_status_value == topic_status::CLOSED {
            return Err(ForumError::TopicClosed);
        }
        if topic_status_value == topic_status::ARCHIVED {
            return Err(ForumError::TopicArchived);
        }

        let create_format = input.content_format.as_str();
        if create_format != "rt_json_v1" && input.content.trim().is_empty() {
            return Err(ForumError::Validation(
                "Reply content cannot be empty".to_string(),
            ));
        }

        let author_id = security.user_id;
        let locale = input.locale.clone();

        let prepared_content = prepare_content_payload(
            Some(&input.content_format),
            Some(&input.content),
            input.content_json.as_ref(),
            &locale,
            "Reply content",
        )
        .map_err(ForumError::Validation)?;

        let metadata = serde_json::json!({
            "parent_reply_id": input.parent_reply_id,
            "reply_status": reply_status::APPROVED,
        });

        let txn = self.db.begin().await?;

        let reply_id = self
            .nodes
            .create_node_in_tx(
                &txn,
                tenant_id,
                security.clone(),
                CreateNodeInput {
                    kind: KIND_REPLY.to_string(),
                    status: Some(rustok_content::entities::node::ContentStatus::Published),
                    parent_id: Some(topic_id),
                    author_id,
                    category_id: None,
                    position: None,
                    depth: None,
                    reply_count: None,
                    metadata,
                    translations: Vec::new(),
                    bodies: vec![BodyInput {
                        locale: locale.clone(),
                        body: Some(prepared_content.body),
                        format: Some(prepared_content.format),
                    }],
                },
            )
            .await?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::ForumTopicReplied {
                    topic_id,
                    reply_id,
                    author_id,
                },
            )
            .await?;

        txn.commit().await?;

        let node = self.nodes.get_node(tenant_id, reply_id).await?;
        Ok(Self::node_to_reply(node, topic_id, &locale))
    }

    #[instrument(skip(self))]
    pub async fn get(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        locale: &str,
    ) -> ForumResult<ReplyResponse> {
        let node = self.nodes.get_node(tenant_id, reply_id).await?;

        if node.kind != KIND_REPLY {
            return Err(ForumError::ReplyNotFound(reply_id));
        }

        let topic_id = node.parent_id.ok_or(ForumError::ReplyNotFound(reply_id))?;
        Ok(Self::node_to_reply(node, topic_id, locale))
    }

    #[instrument(skip(self, security, input))]
    pub async fn update(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        security: SecurityContext,
        input: UpdateReplyInput,
    ) -> ForumResult<ReplyResponse> {
        let existing = self.get(tenant_id, reply_id, &input.locale).await?;
        let bodies = if input.content.is_some()
            || input.content_json.is_some()
            || input.content_format.is_some()
        {
            let prepared_content = prepare_content_payload(
                input.content_format.as_deref(),
                input.content.as_deref(),
                input.content_json.as_ref(),
                &input.locale,
                "Reply content",
            )
            .map_err(ForumError::Validation)?;
            Some(vec![BodyInput {
                locale: input.locale.clone(),
                body: Some(prepared_content.body),
                format: Some(prepared_content.format),
            }])
        } else {
            None
        };

        let node = self
            .nodes
            .update_node(
                tenant_id,
                reply_id,
                security,
                UpdateNodeInput {
                    bodies,
                    ..UpdateNodeInput::default()
                },
            )
            .await?;

        Ok(Self::node_to_reply(node, existing.topic_id, &input.locale))
    }

    #[instrument(skip(self, security))]
    pub async fn delete(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<()> {
        self.nodes
            .delete_node(tenant_id, reply_id, security)
            .await?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn list_for_topic(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        filter: ListRepliesFilter,
    ) -> ForumResult<(Vec<ReplyListItem>, u64)> {
        let locale = filter.locale.clone().unwrap_or_else(|| "en".to_string());
        let (items, total) = self
            .nodes
            .list_nodes(
                tenant_id,
                security,
                ListNodesFilter {
                    kind: Some(KIND_REPLY.to_string()),
                    status: None,
                    parent_id: Some(topic_id),
                    author_id: None,
                    locale: Some(locale.clone()),
                    page: filter.page,
                    per_page: filter.per_page,
                    include_deleted: false,
                    category_id: None,
                },
            )
            .await?;

        let node_ids: Vec<Uuid> = items.iter().map(|item| item.id).collect();

        let mut full_nodes = Vec::with_capacity(node_ids.len());
        for id in node_ids {
            match self.nodes.get_node(tenant_id, id).await {
                Ok(node) => full_nodes.push(node),
                Err(_) => continue,
            }
        }

        let replies = full_nodes
            .into_iter()
            .map(|node| {
                let resolved = resolve_body(&node.bodies, &locale);
                let metadata = &node.metadata;
                let content = resolved
                    .body
                    .and_then(|b| b.body.clone())
                    .unwrap_or_default();
                let preview: String = content.chars().take(200).collect();
                ReplyListItem {
                    id: node.id,
                    locale: locale.clone(),
                    effective_locale: resolved.effective_locale,
                    topic_id,
                    author_id: node.author_id,
                    content_preview: preview,
                    status: metadata
                        .get("reply_status")
                        .and_then(|v| v.as_str())
                        .unwrap_or(reply_status::APPROVED)
                        .to_string(),
                    parent_reply_id: metadata
                        .get("parent_reply_id")
                        .and_then(|v| v.as_str())
                        .and_then(|s| Uuid::parse_str(s).ok()),
                    created_at: node.created_at,
                }
            })
            .collect();

        Ok((replies, total))
    }

    fn node_to_reply(
        node: rustok_content::NodeResponse,
        topic_id: Uuid,
        locale: &str,
    ) -> ReplyResponse {
        let resolved = resolve_body(&node.bodies, locale);
        let metadata = node.metadata;

        let content = resolved
            .body
            .as_ref()
            .and_then(|b| b.body.clone())
            .unwrap_or_default();
        let content_format = resolved
            .body
            .as_ref()
            .map(|b| b.format.clone())
            .unwrap_or_else(|| "markdown".to_string());
        let content_json = if content_format == "rt_json_v1" {
            serde_json::from_str(&content).ok()
        } else {
            None
        };

        ReplyResponse {
            id: node.id,
            locale: locale.to_string(),
            effective_locale: resolved.effective_locale,
            topic_id,
            author_id: node.author_id,
            content,
            content_format,
            content_json,
            status: metadata
                .get("reply_status")
                .and_then(|v| v.as_str())
                .unwrap_or(reply_status::APPROVED)
                .to_string(),
            parent_reply_id: metadata
                .get("parent_reply_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok()),
            created_at: node.created_at,
            updated_at: node.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{reply_status, KIND_REPLY};
    use rustok_content::dto::{BodyResponse, NodeResponse, NodeTranslationResponse};
    use rustok_content::entities::node::ContentStatus;

    fn make_reply_node(
        topic_id: Uuid,
        author_id: Option<Uuid>,
        content: Option<&str>,
        locale: &str,
        status: &str,
        parent_reply_id: Option<Uuid>,
    ) -> NodeResponse {
        let metadata = serde_json::json!({
            "reply_status": status,
            "parent_reply_id": parent_reply_id.map(|u| u.to_string())
        });
        NodeResponse {
            id: Uuid::nil(),
            tenant_id: Uuid::nil(),
            kind: KIND_REPLY.to_string(),
            status: ContentStatus::Published,
            parent_id: Some(topic_id),
            author_id,
            category_id: None,
            position: 0,
            depth: 0,
            reply_count: 0,
            metadata,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            published_at: None,
            deleted_at: None,
            version: 1,
            translations: vec![NodeTranslationResponse {
                locale: locale.to_string(),
                title: None,
                slug: None,
                excerpt: None,
            }],
            bodies: content
                .map(|c| {
                    vec![BodyResponse {
                        locale: locale.to_string(),
                        body: Some(c.to_string()),
                        format: "markdown".to_string(),
                        updated_at: "2024-01-01T00:00:00Z".to_string(),
                    }]
                })
                .unwrap_or_default(),
        }
    }

    #[test]
    fn node_to_reply_maps_fields() {
        let topic_id = Uuid::new_v4();
        let author_id = Uuid::new_v4();
        let parent_reply_id = Uuid::new_v4();
        let node = make_reply_node(
            topic_id,
            Some(author_id),
            Some("Hello!"),
            "en",
            reply_status::APPROVED,
            Some(parent_reply_id),
        );

        let result = ReplyService::node_to_reply(node, topic_id, "en");

        assert_eq!(result.topic_id, topic_id);
        assert_eq!(result.author_id, Some(author_id));
        assert_eq!(result.content, "Hello!");
        assert_eq!(result.content_format, "markdown");
        assert!(result.content_json.is_none());
        assert_eq!(result.status, reply_status::APPROVED);
        assert_eq!(result.parent_reply_id, Some(parent_reply_id));
        assert_eq!(result.effective_locale, "en");
    }

    #[test]
    fn node_to_reply_defaults_on_missing_fields() {
        let topic_id = Uuid::new_v4();
        let node = make_reply_node(topic_id, None, None, "en", reply_status::PENDING, None);

        let result = ReplyService::node_to_reply(node, topic_id, "en");

        assert_eq!(result.content, "");
        assert_eq!(result.status, reply_status::PENDING);
        assert_eq!(result.parent_reply_id, None);
        assert!(result.author_id.is_none());
    }

    #[test]
    fn node_to_reply_extracts_rt_json_content_json() {
        let topic_id = Uuid::new_v4();
        let rich = serde_json::json!({"version":"rt_json_v1","locale":"en","doc":{"type":"doc","content":[]}});
        let node = NodeResponse {
            id: Uuid::new_v4(),
            tenant_id: Uuid::nil(),
            kind: KIND_REPLY.to_string(),
            status: ContentStatus::Published,
            parent_id: Some(topic_id),
            author_id: None,
            category_id: None,
            position: 0,
            depth: 0,
            reply_count: 0,
            metadata: serde_json::json!({"reply_status": reply_status::APPROVED}),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            published_at: None,
            deleted_at: None,
            version: 1,
            translations: vec![],
            bodies: vec![BodyResponse {
                locale: "en".to_string(),
                body: Some(rich.to_string()),
                format: "rt_json_v1".to_string(),
                updated_at: "2024-01-01T00:00:00Z".to_string(),
            }],
        };

        let result = ReplyService::node_to_reply(node, topic_id, "en");
        assert_eq!(result.content_format, "rt_json_v1");
        assert_eq!(result.content_json, Some(rich));
    }

    #[test]
    fn node_to_reply_falls_back_to_first_body_locale() {
        let topic_id = Uuid::new_v4();
        let node = make_reply_node(
            topic_id,
            None,
            Some("Hallo!"),
            "de",
            reply_status::APPROVED,
            None,
        );

        let result = ReplyService::node_to_reply(node, topic_id, "en");
        assert_eq!(result.content, "Hallo!");
        assert_eq!(result.effective_locale, "de");
    }
}
