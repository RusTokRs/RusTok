use sea_orm::DatabaseConnection;
use tracing::instrument;
use uuid::Uuid;

use rustok_content::{
    available_locales_from, resolve_by_locale_with_fallback, BodyInput, CreateNodeInput,
    ListNodesFilter, NodeService, NodeTranslationInput, UpdateNodeInput,
};
use rustok_core::{
    normalize_content_format, prepare_content_payload, SecurityContext, CONTENT_FORMAT_GRAPESJS_V1,
    CONTENT_FORMAT_RT_JSON_V1,
};
use rustok_outbox::TransactionalEventBus;

use crate::dto::*;
use crate::error::{PagesError, PagesResult};
use crate::services::BlockService;

const PAGE_KIND: &str = "page";
const CHANNEL_VISIBILITY_KEY: &str = "channel_visibility";
const ALLOWED_CHANNEL_SLUGS_KEY: &str = "allowed_channel_slugs";

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

        let metadata = build_page_metadata(
            &template,
            &input.translations,
            input.channel_slugs.as_deref(),
            None,
        );

        let bodies = if let Some(body) = input.body {
            let format =
                normalize_content_format(body.format.as_deref()).map_err(PagesError::validation)?;
            if body_requires_json_payload(&format)
                && body.content_json.is_none()
                && body.content.trim().is_empty()
            {
                return Err(PagesError::validation(format!(
                    "content_json is required for {format} format"
                )));
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
    pub async fn get_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> PagesResult<PageResponse> {
        let node = self.ensure_page_kind(tenant_id, page_id).await?;

        let blocks = self
            .blocks
            .list_for_page(tenant_id, security, page_id)
            .await?;
        Ok(node_to_page_with_locale(
            node,
            blocks,
            locale,
            fallback_locale,
        ))
    }

    #[instrument(skip(self))]
    pub async fn get_by_slug(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        locale: &str,
        slug: &str,
    ) -> PagesResult<Option<PageResponse>> {
        self.get_by_slug_with_locale_fallback(tenant_id, security, locale, slug, None)
            .await
    }

    #[instrument(skip(self))]
    pub async fn get_by_slug_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        locale: &str,
        slug: &str,
        fallback_locale: Option<&str>,
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
                Ok(Some(node_to_page_with_locale(
                    node,
                    blocks,
                    locale,
                    fallback_locale,
                )))
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
                channel_slugs: extract_channel_slugs(&full_node.metadata),
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
            input.channel_slugs.as_deref(),
            Some(&existing.metadata),
        );

        let bodies = if let Some(body) = input.body {
            let format =
                normalize_content_format(body.format.as_deref()).map_err(PagesError::validation)?;
            if body_requires_json_payload(&format)
                && body.content_json.is_none()
                && body.content.trim().is_empty()
            {
                return Err(PagesError::validation(format!(
                    "content_json is required for {format} format"
                )));
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
    channel_slugs: Option<&[String]>,
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

    if let Some(channel_slugs) = channel_slugs {
        let normalized = normalize_channel_slugs(channel_slugs);
        if normalized.is_empty() {
            if let Some(object) = metadata.as_object_mut() {
                object.remove(CHANNEL_VISIBILITY_KEY);
            }
        } else {
            metadata[CHANNEL_VISIBILITY_KEY] = serde_json::json!({
                ALLOWED_CHANNEL_SLUGS_KEY: normalized,
            });
        }
    }

    metadata
}

pub(crate) fn extract_channel_slugs(metadata: &serde_json::Value) -> Vec<String> {
    metadata
        .get(CHANNEL_VISIBILITY_KEY)
        .and_then(|value| value.get(ALLOWED_CHANNEL_SLUGS_KEY))
        .and_then(|value| value.as_array())
        .map(|items| {
            normalize_channel_slugs(
                &items
                    .iter()
                    .filter_map(|item| item.as_str().map(ToOwned::to_owned))
                    .collect::<Vec<_>>(),
            )
        })
        .unwrap_or_default()
}

pub(crate) fn is_page_visible_for_channel(
    metadata: &serde_json::Value,
    channel_slug: Option<&str>,
) -> bool {
    let allowed_channel_slugs = extract_channel_slugs(metadata);
    if allowed_channel_slugs.is_empty() {
        return true;
    }

    let Some(channel_slug) = channel_slug else {
        return false;
    };

    let normalized = channel_slug.trim().to_ascii_lowercase();
    !normalized.is_empty() && allowed_channel_slugs.iter().any(|item| item == &normalized)
}

fn normalize_channel_slugs(channel_slugs: &[String]) -> Vec<String> {
    let mut normalized = channel_slugs
        .iter()
        .map(|item| item.trim().to_ascii_lowercase())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn node_to_page(
    node: rustok_content::dto::NodeResponse,
    blocks: Vec<BlockResponse>,
) -> PageResponse {
    node_to_page_internal(node, blocks, None, None)
}

fn node_to_page_with_locale(
    node: rustok_content::dto::NodeResponse,
    blocks: Vec<BlockResponse>,
    requested_locale: &str,
    fallback_locale: Option<&str>,
) -> PageResponse {
    node_to_page_internal(node, blocks, Some(requested_locale), fallback_locale)
}

fn node_to_page_internal(
    node: rustok_content::dto::NodeResponse,
    blocks: Vec<BlockResponse>,
    requested_locale: Option<&str>,
    fallback_locale: Option<&str>,
) -> PageResponse {
    let template = node
        .metadata
        .get("template")
        .and_then(|value| value.as_str())
        .unwrap_or("default")
        .to_string();

    let seo = node.metadata.get("seo").and_then(|value| value.as_object());

    let available_locales =
        available_locales_from(&node.translations, |translation| &translation.locale);

    let selected_translation = requested_locale.and_then(|locale| {
        resolve_by_locale_with_fallback(
            &node.translations,
            locale,
            fallback_locale,
            |translation| &translation.locale,
        )
        .item
        .map(|translation| page_translation_response(translation, seo))
    });

    let effective_locale = requested_locale.map(|locale| {
        let resolved_translation = resolve_by_locale_with_fallback(
            &node.translations,
            locale,
            fallback_locale,
            |translation| &translation.locale,
        );
        if resolved_translation.item.is_some() {
            return resolved_translation.effective_locale;
        }

        let resolved_body =
            resolve_by_locale_with_fallback(&node.bodies, locale, fallback_locale, |body| {
                &body.locale
            });
        resolved_body.effective_locale
    });

    let translations = node
        .translations
        .into_iter()
        .map(|translation| page_translation_response(&translation, seo))
        .collect();

    let body = if let Some(locale) = requested_locale {
        resolve_by_locale_with_fallback(&node.bodies, locale, fallback_locale, |body| &body.locale)
            .item
            .map(page_body_response)
    } else {
        node.bodies.first().map(page_body_response)
    };

    PageResponse {
        id: node.id,
        status: node.status,
        requested_locale: requested_locale.map(str::to_string),
        effective_locale,
        available_locales,
        template,
        created_at: node.created_at,
        updated_at: node.updated_at,
        published_at: node.published_at,
        translation: selected_translation,
        translations,
        body,
        channel_slugs: extract_channel_slugs(&node.metadata),
        blocks,
        metadata: node.metadata,
    }
}

fn page_translation_response(
    translation: &rustok_content::dto::NodeTranslationResponse,
    seo: Option<&serde_json::Map<String, serde_json::Value>>,
) -> PageTranslationResponse {
    let meta = seo
        .and_then(|map| map.get(&translation.locale))
        .and_then(|value| value.as_object());

    PageTranslationResponse {
        locale: translation.locale.clone(),
        title: translation.title.clone(),
        slug: translation.slug.clone(),
        meta_title: meta
            .and_then(|value| value.get("meta_title"))
            .and_then(|value| value.as_str())
            .map(String::from),
        meta_description: meta
            .and_then(|value| value.get("meta_description"))
            .and_then(|value| value.as_str())
            .map(String::from),
    }
}

fn page_body_response(body: &rustok_content::dto::BodyResponse) -> PageBodyResponse {
    let content = body.body.clone().unwrap_or_default();
    let format = body.format.clone();
    let content_json =
        if format == CONTENT_FORMAT_RT_JSON_V1 || format == CONTENT_FORMAT_GRAPESJS_V1 {
            serde_json::from_str(&content).ok()
        } else {
            None
        };

    PageBodyResponse {
        locale: body.locale.clone(),
        content,
        format,
        content_json,
        updated_at: body.updated_at.clone(),
    }
}

fn body_requires_json_payload(format: &str) -> bool {
    format == CONTENT_FORMAT_RT_JSON_V1 || format == CONTENT_FORMAT_GRAPESJS_V1
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_content::dto::{BodyResponse, NodeResponse, NodeTranslationResponse};
    use rustok_content::entities::node::ContentStatus;

    fn make_page_node(
        translations: Vec<NodeTranslationResponse>,
        bodies: Vec<BodyResponse>,
    ) -> NodeResponse {
        NodeResponse {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            kind: PAGE_KIND.to_string(),
            status: ContentStatus::Published,
            parent_id: None,
            author_id: None,
            category_id: None,
            position: 0,
            depth: 0,
            reply_count: 0,
            metadata: serde_json::json!({ "template": "landing" }),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            published_at: None,
            deleted_at: None,
            version: 1,
            translations,
            bodies,
        }
    }

    fn translation(locale: &str, title: &str, slug: &str) -> NodeTranslationResponse {
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

    fn grapes_body(locale: &str, content_json: serde_json::Value) -> BodyResponse {
        BodyResponse {
            locale: locale.to_string(),
            body: Some(content_json.to_string()),
            format: CONTENT_FORMAT_GRAPESJS_V1.to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn node_to_page_with_locale_tracks_requested_and_effective_locale() {
        let node = make_page_node(
            vec![
                translation("en", "About", "about"),
                translation("ru", "О нас", "o-nas"),
            ],
            vec![body("en", "English body")],
        );

        let page = node_to_page_with_locale(node, vec![], "ru", Some("en"));

        assert_eq!(page.requested_locale.as_deref(), Some("ru"));
        assert_eq!(page.effective_locale.as_deref(), Some("ru"));
        assert_eq!(
            page.translation.and_then(|translation| translation.title),
            Some("О нас".to_string())
        );
        assert_eq!(
            page.body.and_then(|body| Some(body.locale)),
            Some("en".to_string())
        );
    }

    #[test]
    fn node_to_page_with_locale_falls_back_for_translation_and_body() {
        let node = make_page_node(
            vec![translation("en", "About", "about")],
            vec![body("en", "English body")],
        );

        let page = node_to_page_with_locale(node, vec![], "ru", Some("en"));

        assert_eq!(page.requested_locale.as_deref(), Some("ru"));
        assert_eq!(page.effective_locale.as_deref(), Some("en"));
        assert_eq!(
            page.translation.and_then(|translation| translation.title),
            Some("About".to_string())
        );
        assert_eq!(
            page.body
                .and_then(|selected_body| Some(selected_body.locale)),
            Some("en".to_string())
        );
    }

    #[test]
    fn page_body_response_parses_grapesjs_content_json() {
        let node = make_page_node(
            vec![translation("en", "Landing", "landing")],
            vec![grapes_body(
                "en",
                serde_json::json!({
                    "pages": [],
                    "styles": [],
                }),
            )],
        );

        let page = node_to_page(node, vec![]);
        let body = page.body.expect("page body");

        assert_eq!(body.format, CONTENT_FORMAT_GRAPESJS_V1);
        assert_eq!(
            body.content_json,
            Some(serde_json::json!({
                "pages": [],
                "styles": [],
            }))
        );
    }

    #[test]
    fn build_page_metadata_normalizes_channel_slugs() {
        let metadata = build_page_metadata(
            "default",
            &[],
            Some(&[" Web ".to_string(), "mobile".to_string(), "web".to_string()]),
            None,
        );

        assert_eq!(
            extract_channel_slugs(&metadata),
            vec!["mobile".to_string(), "web".to_string()]
        );
    }

    #[test]
    fn page_visibility_respects_channel_allowlist() {
        let metadata = serde_json::json!({
            "channel_visibility": {
                "allowed_channel_slugs": ["web", "mobile"]
            }
        });

        assert!(is_page_visible_for_channel(&metadata, Some("web")));
        assert!(!is_page_visible_for_channel(&metadata, Some("blog")));
        assert!(!is_page_visible_for_channel(&metadata, None));
    }
}
