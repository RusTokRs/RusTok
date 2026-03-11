use sea_orm::DatabaseConnection;
use tracing::instrument;
use uuid::Uuid;

use rustok_content::{
    BodyInput, CreateNodeInput, ListNodesFilter, NodeService, NodeTranslationInput, UpdateNodeInput,
};
use rustok_core::{
    normalize_content_format, prepare_content_payload, SecurityContext, CONTENT_FORMAT_RT_JSON_V1,
};
use rustok_outbox::TransactionalEventBus;

use crate::dto::*;
use crate::error::{PagesError, PagesResult};
use crate::services::BlockService;

const PAGE_KIND: &str = "page";

pub struct PageService {
    nodes: NodeService,
    blocks: BlockService,
}

impl PageService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            nodes: NodeService::new(db.clone(), event_bus.clone()),
            blocks: BlockService::new(db, event_bus),
        }
    }

    #[instrument(skip(self, input))]
    pub async fn create(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreatePageInput,
    ) -> PagesResult<PageResponse> {
        let template = input
            .template
            .clone()
            .unwrap_or_else(|| "default".to_string());

        let metadata = build_page_metadata(&template, &input.translations, None);

        let bodies = if let Some(body) = input.body {
            let format =
                normalize_content_format(body.format.as_deref()).map_err(PagesError::validation)?;
            if format == CONTENT_FORMAT_RT_JSON_V1
                && body.content_json.is_none()
                && body.content.trim().is_empty()
            {
                return Err(PagesError::validation(
                    "content_json is required for rt_json_v1 format",
                ));
            }
            let markdown_source = if body.content.trim().is_empty() {
                None
            } else {
                Some(body.content.as_str())
            };

            let prepared_body = prepare_content_payload(
                Some(&format),
                markdown_source,
                body.content_json.as_ref(),
                &body.locale,
                "Body",
            )
            .map_err(PagesError::validation)?;

            vec![BodyInput {
                locale: body.locale,
                body: Some(prepared_body.body),
                format: Some(prepared_body.format),
            }]
        } else {
            Vec::new()
        };

        let node = self
            .nodes
            .create_node(
                tenant_id,
                security.clone(),
                CreateNodeInput {
                    kind: PAGE_KIND.to_string(),
                    status: Some(if input.publish {
                        rustok_content::entities::node::ContentStatus::Published
                    } else {
                        rustok_content::entities::node::ContentStatus::Draft
                    }),
                    parent_id: None,
                    author_id: security.user_id,
                    category_id: None,
                    position: None,
                    depth: None,
                    reply_count: None,
                    metadata,
                    translations: input
                        .translations
                        .into_iter()
                        .map(|translation| NodeTranslationInput {
                            locale: translation.locale,
                            title: Some(translation.title),
                            slug: translation.slug,
                            excerpt: None,
                        })
                        .collect(),
                    bodies,
                },
            )
            .await?;

        if let Some(blocks) = input.blocks {
            for block in blocks {
                self.blocks
                    .create(tenant_id, security.clone(), node.id, block)
                    .await?;
            }
        }

        self.get(tenant_id, security, node.id).await
    }

    #[instrument(skip(self))]
    pub async fn get(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
    ) -> PagesResult<PageResponse> {
        let node = self.ensure_page_kind(tenant_id, page_id).await?;

        let blocks = self
            .blocks
            .list_for_page(tenant_id, security, page_id)
            .await?;
        Ok(node_to_page(node, blocks))
    }

    #[instrument(skip(self))]
    pub async fn get_by_slug(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        locale: &str,
        slug: &str,
    ) -> PagesResult<Option<PageResponse>> {
        let node = self
            .nodes
            .get_by_slug(tenant_id, PAGE_KIND, locale, slug)
            .await?;

        match node {
            Some(node)
                if node.status == rustok_content::entities::node::ContentStatus::Published =>
            {
                let blocks = self
                    .blocks
                    .list_for_page(tenant_id, security, node.id)
                    .await?;
                Ok(Some(node_to_page(node, blocks)))
            }
            _ => Ok(None),
        }
    }

    #[instrument(skip(self))]
    pub async fn list(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        filter: ListPagesFilter,
    ) -> PagesResult<(Vec<PageListItem>, u64)> {
        let (nodes, total) = self
            .nodes
            .list_nodes(
                tenant_id,
                security.clone(),
                ListNodesFilter {
                    kind: Some(PAGE_KIND.to_string()),
                    status: filter.status,
                    parent_id: None,
                    author_id: None,
                    category_id: None,
                    locale: filter.locale,
                    page: filter.page,
                    per_page: filter.per_page,
                    include_deleted: false,
                },
            )
            .await?;

        let mut pages = Vec::with_capacity(nodes.len());
        for node in nodes {
            let full_node = self.nodes.get_node(tenant_id, node.id).await?;
            let template = full_node
                .metadata
                .get("template")
                .and_then(|value| value.as_str())
                .unwrap_or("default")
                .to_string();

            pages.push(PageListItem {
                id: full_node.id,
                status: full_node.status,
                template,
                title: node.title,
                slug: node.slug,
                updated_at: full_node.updated_at,
            });
        }

        Ok((pages, total))
    }

    #[instrument(skip(self, input))]
    pub async fn update(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        input: UpdatePageInput,
    ) -> PagesResult<PageResponse> {
        let existing = self.ensure_page_kind(tenant_id, page_id).await?;

        let template = input.template.clone().unwrap_or_else(|| {
            existing
                .metadata
                .get("template")
                .and_then(|value| value.as_str())
                .unwrap_or("default")
                .to_string()
        });

        let metadata = build_page_metadata(
            &template,
            input.translations.as_deref().unwrap_or(&[]),
            Some(&existing.metadata),
        );

        let bodies = if let Some(body) = input.body {
            let format =
                normalize_content_format(body.format.as_deref()).map_err(PagesError::validation)?;
            if format == CONTENT_FORMAT_RT_JSON_V1
                && body.content_json.is_none()
                && body.content.trim().is_empty()
            {
                return Err(PagesError::validation(
                    "content_json is required for rt_json_v1 format",
                ));
            }
            let markdown_source = if body.content.trim().is_empty() {
                None
            } else {
                Some(body.content.as_str())
            };

            let prepared_body = prepare_content_payload(
                Some(&format),
                markdown_source,
                body.content_json.as_ref(),
                &body.locale,
                "Body",
            )
            .map_err(PagesError::validation)?;

            Some(vec![BodyInput {
                locale: body.locale,
                body: Some(prepared_body.body),
                format: Some(prepared_body.format),
            }])
        } else {
            None
        };

        self.nodes
            .update_node(
                tenant_id,
                page_id,
                security.clone(),
                UpdateNodeInput {
                    status: input.status,
                    parent_id: None,
                    author_id: None,
                    category_id: None,
                    position: None,
                    depth: None,
                    reply_count: None,
                    metadata: Some(metadata),
                    translations: input.translations.map(|translations| {
                        translations
                            .into_iter()
                            .map(|translation| NodeTranslationInput {
                                locale: translation.locale,
                                title: Some(translation.title),
                                slug: translation.slug,
                                excerpt: None,
                            })
                            .collect()
                    }),
                    bodies,
                    expected_version: None,
                },
            )
            .await?;

        self.get(tenant_id, security, page_id).await
    }

    #[instrument(skip(self))]
    pub async fn publish(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
    ) -> PagesResult<PageResponse> {
        self.ensure_page_kind(tenant_id, page_id).await?;
        self.nodes
            .publish_node(tenant_id, page_id, security.clone())
            .await?;
        self.get(tenant_id, security, page_id).await
    }

    #[instrument(skip(self))]
    pub async fn unpublish(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
    ) -> PagesResult<PageResponse> {
        self.ensure_page_kind(tenant_id, page_id).await?;
        self.nodes
            .unpublish_node(tenant_id, page_id, security.clone())
            .await?;
        self.get(tenant_id, security, page_id).await
    }

    pub async fn delete(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
    ) -> PagesResult<()> {
        self.ensure_page_kind(tenant_id, page_id).await?;
        self.blocks
            .delete_all_for_page(tenant_id, security.clone(), page_id)
            .await?;
        self.nodes.delete_node(tenant_id, page_id, security).await?;
        Ok(())
    }

    async fn ensure_page_kind(
        &self,
        tenant_id: Uuid,
        page_id: Uuid,
    ) -> PagesResult<rustok_content::dto::NodeResponse> {
        let node = self.nodes.get_node(tenant_id, page_id).await?;
        if node.kind != PAGE_KIND {
            return Err(PagesError::PageNotFound(page_id));
        }

        Ok(node)
    }
}

fn build_page_metadata(
    template: &str,
    translations: &[PageTranslationInput],
    existing: Option<&serde_json::Value>,
) -> serde_json::Value {
    let mut metadata = existing
        .cloned()
        .filter(|value| value.is_object())
        .unwrap_or_else(|| serde_json::json!({}));

    metadata["template"] = serde_json::json!(template);

    let mut seo = serde_json::Map::new();
    for translation in translations {
        let meta_title = translation.meta_title.clone();
        let meta_description = translation.meta_description.clone();

        if meta_title.is_some() || meta_description.is_some() {
            seo.insert(
                translation.locale.clone(),
                serde_json::json!({
                    "meta_title": meta_title,
                    "meta_description": meta_description,
                }),
            );
        }
    }

    if !seo.is_empty() {
        metadata["seo"] = serde_json::Value::Object(seo);
    } else if let Some(existing) = existing {
        if let Some(existing_seo) = existing.get("seo") {
            metadata["seo"] = existing_seo.clone();
        }
    }

    metadata
}

fn node_to_page(
    node: rustok_content::dto::NodeResponse,
    blocks: Vec<BlockResponse>,
) -> PageResponse {
    let template = node
        .metadata
        .get("template")
        .and_then(|value| value.as_str())
        .unwrap_or("default")
        .to_string();

    let seo = node.metadata.get("seo").and_then(|value| value.as_object());

    let translations = node
        .translations
        .into_iter()
        .map(|translation| {
            let meta = seo
                .and_then(|map| map.get(&translation.locale))
                .and_then(|value| value.as_object());

            PageTranslationResponse {
                locale: translation.locale,
                title: translation.title,
                slug: translation.slug,
                meta_title: meta
                    .and_then(|value| value.get("meta_title"))
                    .and_then(|value| value.as_str())
                    .map(String::from),
                meta_description: meta
                    .and_then(|value| value.get("meta_description"))
                    .and_then(|value| value.as_str())
                    .map(String::from),
            }
        })
        .collect();

    let body = node.bodies.into_iter().next().map(|body| {
        let content = body.body.unwrap_or_default();
        let format = body.format;
        let content_json = if format == "rt_json_v1" {
            serde_json::from_str(&content).ok()
        } else {
            None
        };

        PageBodyResponse {
            locale: body.locale,
            content,
            format,
            content_json,
            updated_at: body.updated_at,
        }
    });

    PageResponse {
        id: node.id,
        status: node.status,
        template,
        created_at: node.created_at,
        updated_at: node.updated_at,
        published_at: node.published_at,
        translations,
        body,
        blocks,
        metadata: node.metadata,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_content::{CreateNodeInput, NodeService};
    use rustok_core::SecurityContext;
    use rustok_test_utils::{
        db::setup_test_db, helpers::admin_context, mock_transactional_event_bus,
    };
    use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};

    async fn ensure_content_schema(db: &DatabaseConnection) {
        if db.get_database_backend() != DbBackend::Sqlite {
            return;
        }

        db.execute(Statement::from_string(
            DbBackend::Sqlite,
            "CREATE TABLE IF NOT EXISTS nodes (
                id TEXT PRIMARY KEY,
                tenant_id TEXT NOT NULL,
                parent_id TEXT NULL,
                author_id TEXT NULL,
                kind TEXT NOT NULL,
                category_id TEXT NULL,
                status TEXT NOT NULL,
                position INTEGER NOT NULL,
                depth INTEGER NOT NULL,
                reply_count INTEGER NOT NULL,
                metadata TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                published_at TEXT NULL,
                deleted_at TEXT NULL,
                version INTEGER NOT NULL DEFAULT 1
            )"
            .to_string(),
        ))
        .await
        .expect("failed to create nodes test table");

        db.execute(Statement::from_string(
            DbBackend::Sqlite,
            "CREATE TABLE IF NOT EXISTS node_translations (
                id TEXT PRIMARY KEY,
                node_id TEXT NOT NULL,
                locale TEXT NOT NULL,
                title TEXT NULL,
                slug TEXT NULL,
                excerpt TEXT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY(node_id) REFERENCES nodes(id)
            )"
            .to_string(),
        ))
        .await
        .expect("failed to create node_translations test table");

        db.execute(Statement::from_string(
            DbBackend::Sqlite,
            "CREATE TABLE IF NOT EXISTS bodies (
                id TEXT PRIMARY KEY,
                node_id TEXT NOT NULL,
                locale TEXT NOT NULL,
                body TEXT NULL,
                format TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY(node_id) REFERENCES nodes(id)
            )"
            .to_string(),
        ))
        .await
        .expect("failed to create bodies test table");
    }

    async fn setup() -> (PageService, NodeService, Uuid, SecurityContext) {
        let db = setup_test_db().await;
        ensure_content_schema(&db).await;
        let event_bus = mock_transactional_event_bus();
        let page_service = PageService::new(db.clone(), event_bus.clone());
        let node_service = NodeService::new(db, event_bus);

        (page_service, node_service, Uuid::new_v4(), admin_context())
    }

    async fn create_post_node(
        node_service: &NodeService,
        tenant_id: Uuid,
        security: SecurityContext,
    ) -> rustok_content::dto::NodeResponse {
        node_service
            .create_node(
                tenant_id,
                security,
                CreateNodeInput {
                    kind: "post".to_string(),
                    status: Some(rustok_content::entities::node::ContentStatus::Draft),
                    parent_id: None,
                    author_id: None,
                    category_id: None,
                    position: Some(0),
                    depth: Some(0),
                    reply_count: Some(0),
                    metadata: serde_json::json!({}),
                    translations: vec![rustok_content::NodeTranslationInput {
                        locale: "en".to_string(),
                        title: Some("Post".to_string()),
                        slug: None,
                        excerpt: None,
                    }],
                    bodies: vec![],
                },
            )
            .await
            .expect("failed to create post node")
    }

    #[tokio::test]
    async fn publish_returns_page_not_found_for_post_id_and_keeps_status() {
        let (page_service, node_service, tenant_id, security) = setup().await;
        let post = create_post_node(&node_service, tenant_id, security.clone()).await;

        let result = page_service
            .publish(tenant_id, security.clone(), post.id)
            .await;

        assert!(matches!(result, Err(PagesError::PageNotFound(id)) if id == post.id));

        let unchanged = node_service
            .get_node(tenant_id, post.id)
            .await
            .expect("post node should remain accessible");
        assert_eq!(unchanged.status, post.status);
        assert!(unchanged.deleted_at.is_none());
    }

    #[tokio::test]
    async fn unpublish_returns_page_not_found_for_post_id_and_keeps_status() {
        let (page_service, node_service, tenant_id, security) = setup().await;
        let post = create_post_node(&node_service, tenant_id, security.clone()).await;

        let result = page_service
            .unpublish(tenant_id, security.clone(), post.id)
            .await;

        assert!(matches!(result, Err(PagesError::PageNotFound(id)) if id == post.id));

        let unchanged = node_service
            .get_node(tenant_id, post.id)
            .await
            .expect("post node should remain accessible");
        assert_eq!(unchanged.status, post.status);
        assert!(unchanged.deleted_at.is_none());
    }

    #[tokio::test]
    async fn delete_returns_page_not_found_for_post_id_and_keeps_record() {
        let (page_service, node_service, tenant_id, security) = setup().await;
        let post = create_post_node(&node_service, tenant_id, security.clone()).await;

        let result = page_service
            .delete(tenant_id, security.clone(), post.id)
            .await;

        assert!(matches!(result, Err(PagesError::PageNotFound(id)) if id == post.id));

        let unchanged = node_service
            .get_node(tenant_id, post.id)
            .await
            .expect("post node should remain accessible");
        assert_eq!(unchanged.status, post.status);
        assert!(unchanged.deleted_at.is_none());
    }
}
