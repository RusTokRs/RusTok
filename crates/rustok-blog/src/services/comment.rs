use sea_orm::{DatabaseConnection, TransactionTrait};
use tracing::instrument;
use uuid::Uuid;

use rustok_content::{
    BodyInput, CreateNodeInput, ListNodesFilter, NodeService, NodeTranslationInput, UpdateNodeInput,
};
use rustok_core::{prepare_content_payload, DomainEvent, SecurityContext};
use rustok_outbox::TransactionalEventBus;

use crate::dto::{
    CommentListItem, CommentResponse, CreateCommentInput, ListCommentsFilter, UpdateCommentInput,
};
use crate::error::{BlogError, BlogResult};
use crate::locale::resolve_body;
use crate::state_machine::CommentStatus;

const KIND_POST: &str = "post";
const KIND_COMMENT: &str = "comment";
const DEFAULT_COMMENT_TITLE: &str = "Comment";

pub struct CommentService {
    db: DatabaseConnection,
    nodes: NodeService,
    event_bus: TransactionalEventBus,
}

impl CommentService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            nodes: NodeService::new(db.clone(), event_bus.clone()),
            db,
            event_bus,
        }
    }

    #[instrument(skip(self, security, input))]
    pub async fn create_comment(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        post_id: Uuid,
        input: CreateCommentInput,
    ) -> BlogResult<CommentResponse> {
        let post_node = self
            .nodes
            .get_node(tenant_id, post_id)
            .await
            .map_err(BlogError::from)?;
        if post_node.kind != KIND_POST {
            return Err(BlogError::post_not_found(post_id));
        }

        let create_format = input.content_format.as_str();
        if create_format != "rt_json_v1" && input.content.trim().is_empty() {
            return Err(BlogError::validation("Comment content cannot be empty"));
        }

        let locale = input.locale.clone();
        let content = input.content;
        let prepared_content = prepare_content_payload(
            Some(&input.content_format),
            Some(&content),
            input.content_json.as_ref(),
            &locale,
            "Comment content",
        )
        .map_err(BlogError::validation)?;
        let translation_title = Self::build_comment_translation_title(&content);
        let metadata = serde_json::json!({
            "parent_comment_id": input.parent_comment_id,
            "comment_status": CommentStatus::Pending,
        });

        let txn = self.db.begin().await.map_err(BlogError::from)?;

        let comment_id = self
            .nodes
            .create_node_in_tx(
                &txn,
                tenant_id,
                security.clone(),
                CreateNodeInput {
                    kind: KIND_COMMENT.to_string(),
                    status: Some(rustok_content::entities::node::ContentStatus::Published),
                    parent_id: Some(post_id),
                    author_id: security.user_id,
                    category_id: None,
                    position: None,
                    depth: None,
                    reply_count: None,
                    metadata,
                    translations: vec![NodeTranslationInput {
                        locale: locale.clone(),
                        title: Some(translation_title),
                        slug: None,
                        excerpt: None,
                    }],
                    bodies: vec![BodyInput {
                        locale: locale.clone(),
                        body: Some(prepared_content.body),
                        format: Some(prepared_content.format),
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
                DomainEvent::NodeUpdated {
                    node_id: post_id,
                    kind: KIND_POST.to_string(),
                },
            )
            .await
            .map_err(BlogError::from)?;

        txn.commit().await.map_err(BlogError::from)?;

        let node = self
            .nodes
            .get_node(tenant_id, comment_id)
            .await
            .map_err(BlogError::from)?;
        Ok(Self::node_to_comment(node, post_id, &locale))
    }

    #[instrument(skip(self))]
    pub async fn get_comment(
        &self,
        tenant_id: Uuid,
        comment_id: Uuid,
        locale: &str,
    ) -> BlogResult<CommentResponse> {
        let node = self
            .nodes
            .get_node(tenant_id, comment_id)
            .await
            .map_err(BlogError::from)?;

        if node.kind != KIND_COMMENT {
            return Err(BlogError::comment_not_found(comment_id));
        }

        let post_id = node
            .parent_id
            .ok_or_else(|| BlogError::comment_not_found(comment_id))?;
        Ok(Self::node_to_comment(node, post_id, locale))
    }

    #[instrument(skip(self, security, input))]
    pub async fn update_comment(
        &self,
        tenant_id: Uuid,
        comment_id: Uuid,
        security: SecurityContext,
        input: UpdateCommentInput,
    ) -> BlogResult<CommentResponse> {
        let existing = self
            .get_comment(tenant_id, comment_id, &input.locale)
            .await?;
        let bodies = if input.content.is_some()
            || input.content_json.is_some()
            || input.content_format.is_some()
        {
            let prepared_content = prepare_content_payload(
                input.content_format.as_deref(),
                input.content.as_deref(),
                input.content_json.as_ref(),
                &input.locale,
                "Comment content",
            )
            .map_err(BlogError::validation)?;
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
                comment_id,
                security,
                UpdateNodeInput {
                    bodies,
                    ..UpdateNodeInput::default()
                },
            )
            .await
            .map_err(BlogError::from)?;

        Ok(Self::node_to_comment(node, existing.post_id, &input.locale))
    }

    #[instrument(skip(self, security))]
    pub async fn delete_comment(
        &self,
        tenant_id: Uuid,
        comment_id: Uuid,
        security: SecurityContext,
    ) -> BlogResult<()> {
        self.nodes
            .delete_node(tenant_id, comment_id, security)
            .await
            .map_err(BlogError::from)?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn list_for_post(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        post_id: Uuid,
        filter: ListCommentsFilter,
    ) -> BlogResult<(Vec<CommentListItem>, u64)> {
        let locale = filter.locale.clone().unwrap_or_else(|| "en".to_string());
        let (items, total) = self
            .nodes
            .list_nodes(
                tenant_id,
                security,
                ListNodesFilter {
                    kind: Some(KIND_COMMENT.to_string()),
                    status: None,
                    parent_id: Some(post_id),
                    author_id: None,
                    locale: Some(locale.clone()),
                    page: filter.page,
                    per_page: filter.per_page,
                    include_deleted: false,
                    category_id: None,
                },
            )
            .await
            .map_err(BlogError::from)?;

        let mut full_nodes = Vec::with_capacity(items.len());
        for id in items.into_iter().map(|item| item.id) {
            if let Ok(node) = self.nodes.get_node(tenant_id, id).await {
                full_nodes.push(node);
            }
        }

        let comments = full_nodes
            .into_iter()
            .map(|node| {
                let resolved = resolve_body(&node.bodies, &locale);
                let content = resolved
                    .body
                    .and_then(|b| b.body.clone())
                    .unwrap_or_default();
                let preview: String = content.chars().take(200).collect();
                CommentListItem {
                    id: node.id,
                    locale: locale.clone(),
                    effective_locale: resolved.effective_locale,
                    post_id,
                    author_id: node.author_id,
                    content_preview: preview,
                    status: node
                        .metadata
                        .get("comment_status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("pending")
                        .to_string(),
                    parent_comment_id: node
                        .metadata
                        .get("parent_comment_id")
                        .and_then(|v| v.as_str())
                        .and_then(|s| Uuid::parse_str(s).ok()),
                    created_at: node.created_at,
                }
            })
            .collect();

        Ok((comments, total))
    }

    fn node_to_comment(
        node: rustok_content::NodeResponse,
        post_id: Uuid,
        locale: &str,
    ) -> CommentResponse {
        let resolved = resolve_body(&node.bodies, locale);

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

        CommentResponse {
            id: node.id,
            locale: locale.to_string(),
            effective_locale: resolved.effective_locale,
            post_id,
            author_id: node.author_id,
            content,
            content_format,
            content_json,
            status: node
                .metadata
                .get("comment_status")
                .and_then(|v| v.as_str())
                .unwrap_or("pending")
                .to_string(),
            parent_comment_id: node
                .metadata
                .get("parent_comment_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok()),
            created_at: node.created_at,
            updated_at: node.updated_at,
        }
    }

    fn build_comment_translation_title(content: &str) -> String {
        let normalized = content.split_whitespace().collect::<Vec<_>>().join(" ");
        let preview: String = normalized.chars().take(80).collect();
        if preview.is_empty() {
            DEFAULT_COMMENT_TITLE.to_string()
        } else {
            preview
        }
    }
}

#[cfg(test)]
mod rich_content_tests {
    use super::*;
    use rustok_content::dto::{BodyResponse, NodeResponse};
    use rustok_content::entities::node::ContentStatus;

    #[test]
    fn node_to_comment_extracts_rt_json_content_json() {
        let rich = serde_json::json!({"version":"rt_json_v1","locale":"en","doc":{"type":"doc","content":[]}});
        let node = NodeResponse {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            kind: KIND_COMMENT.to_string(),
            status: ContentStatus::Published,
            parent_id: Some(Uuid::new_v4()),
            author_id: None,
            category_id: None,
            position: 0,
            depth: 0,
            reply_count: 0,
            metadata: serde_json::json!({}),
            created_at: "2024-01-01T00:00:00Z".into(),
            updated_at: "2024-01-01T00:00:00Z".into(),
            published_at: None,
            deleted_at: None,
            version: 1,
            translations: vec![],
            bodies: vec![BodyResponse {
                locale: "en".into(),
                body: Some(rich.to_string()),
                format: "rt_json_v1".into(),
                updated_at: "2024-01-01T00:00:00Z".into(),
            }],
        };

        let result = CommentService::node_to_comment(node, Uuid::new_v4(), "en");
        assert_eq!(result.content_format, "rt_json_v1");
        assert_eq!(result.content_json, Some(rich));
    }
}
