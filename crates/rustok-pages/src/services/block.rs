use sea_orm::DatabaseConnection;
use serde_json::{Map, Value};
use tracing::instrument;
use url::Url;
use uuid::Uuid;

use rustok_content::{CreateNodeInput, ListNodesFilter, NodeService, UpdateNodeInput};
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;

use crate::dto::*;
use crate::error::{PagesError, PagesResult};

const BLOCK_KIND: &str = "block";

pub struct BlockService {
    nodes: NodeService,
}

impl BlockService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            nodes: NodeService::new(db, event_bus),
        }
    }

    #[instrument(skip(self, input))]
    pub async fn create(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        input: CreateBlockInput,
    ) -> PagesResult<BlockResponse> {
        let data = validate_and_sanitize_block_data(&input.block_type, input.data)?;
        let translations = sanitize_translations(&input.block_type, input.translations)?;

        let metadata = serde_json::json!({
            "block_type": input.block_type,
            "data": data,
            "translations": translations,
        });

        let node = self
            .nodes
            .create_node(
                tenant_id,
                security,
                CreateNodeInput {
                    kind: BLOCK_KIND.to_string(),
                    status: Some(rustok_content::entities::node::ContentStatus::Published),
                    parent_id: Some(page_id),
                    author_id: None,
                    category_id: None,
                    position: Some(input.position),
                    depth: None,
                    reply_count: None,
                    metadata,
                    translations: vec![],
                    bodies: vec![],
                },
            )
            .await?;

        Ok(node_to_block(node))
    }

    #[instrument(skip(self))]
    pub async fn list_for_page(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
    ) -> PagesResult<Vec<BlockResponse>> {
        let (items, _) = self
            .nodes
            .list_nodes(
                tenant_id,
                security.clone(),
                ListNodesFilter {
                    kind: Some(BLOCK_KIND.to_string()),
                    status: None,
                    parent_id: Some(page_id),
                    author_id: None,
                    category_id: None,
                    locale: None,
                    page: 1,
                    per_page: 100,
                    include_deleted: false,
                },
            )
            .await?;

        let mut blocks = Vec::with_capacity(items.len());
        for item in items {
            let node = self.nodes.get_node(tenant_id, item.id).await?;
            blocks.push(node_to_block(node));
        }

        Ok(blocks)
    }

    #[instrument(skip(self, input))]
    pub async fn update(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        block_id: Uuid,
        input: UpdateBlockInput,
    ) -> PagesResult<BlockResponse> {
        let existing = self.nodes.get_node(tenant_id, block_id).await?;
        if existing.kind != BLOCK_KIND {
            return Err(PagesError::BlockNotFound(block_id));
        }

        let block_type: BlockType = existing
            .metadata
            .get("block_type")
            .and_then(|value| serde_json::from_value(value.clone()).ok())
            .unwrap_or_default();

        let mut metadata = if existing.metadata.is_object() {
            existing.metadata
        } else {
            serde_json::json!({})
        };
        if let Some(data) = input.data {
            metadata["data"] = validate_and_sanitize_block_data(&block_type, data)?;
        }
        if let Some(translations) = input.translations {
            metadata["translations"] =
                serde_json::to_value(sanitize_translations(&block_type, Some(translations))?)
                    .map_err(|err| PagesError::validation(err.to_string()))?;
        }

        self.nodes
            .update_node(
                tenant_id,
                block_id,
                security.clone(),
                UpdateNodeInput {
                    status: None,
                    parent_id: None,
                    author_id: None,
                    category_id: None,
                    position: input.position,
                    depth: None,
                    reply_count: None,
                    metadata: Some(metadata),
                    translations: None,
                    bodies: None,
                    expected_version: None,
                },
            )
            .await?;

        let node = self.nodes.get_node(tenant_id, block_id).await?;
        Ok(node_to_block(node))
    }

    #[instrument(skip(self))]
    pub async fn reorder(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        block_order: Vec<Uuid>,
    ) -> PagesResult<()> {
        let _ = page_id;
        for (position, block_id) in block_order.into_iter().enumerate() {
            self.nodes
                .update_node(
                    tenant_id,
                    block_id,
                    security.clone(),
                    UpdateNodeInput {
                        status: None,
                        parent_id: None,
                        author_id: None,
                        category_id: None,
                        position: Some(position as i32),
                        depth: None,
                        reply_count: None,
                        metadata: None,
                        translations: None,
                        bodies: None,
                        expected_version: None,
                    },
                )
                .await?;
        }
        Ok(())
    }

    pub async fn delete(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        block_id: Uuid,
    ) -> PagesResult<()> {
        self.nodes
            .delete_node(tenant_id, block_id, security)
            .await?;
        Ok(())
    }

    pub async fn delete_all_for_page(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
    ) -> PagesResult<()> {
        let blocks = self
            .list_for_page(tenant_id, security.clone(), page_id)
            .await?;
        for block in blocks {
            self.nodes
                .delete_node(tenant_id, block.id, security.clone())
                .await?;
        }
        Ok(())
    }
}

fn node_to_block(node: rustok_content::dto::NodeResponse) -> BlockResponse {
    let block_type: BlockType = node
        .metadata
        .get("block_type")
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_default();

    let data = node
        .metadata
        .get("data")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));

    let translations = node
        .metadata
        .get("translations")
        .and_then(|value| serde_json::from_value(value.clone()).ok());

    BlockResponse {
        id: node.id,
        block_type,
        position: node.position,
        data,
        translations,
    }
}

fn sanitize_translations(
    block_type: &BlockType,
    translations: Option<Vec<BlockTranslationInput>>,
) -> PagesResult<Option<Vec<BlockTranslationInput>>> {
    translations
        .map(|items| {
            items
                .into_iter()
                .map(|item| {
                    let data = validate_and_sanitize_block_data(block_type, item.data)?;
                    Ok(BlockTranslationInput {
                        locale: item.locale,
                        data,
                    })
                })
                .collect::<PagesResult<Vec<_>>>()
        })
        .transpose()
}

fn validate_and_sanitize_block_data(block_type: &BlockType, data: Value) -> PagesResult<Value> {
    let payload = BlockPayload::from_block_type(block_type, data)
        .map_err(|err| PagesError::validation(format!("Invalid block payload: {err}")))?;

    sanitize_payload(payload)?.into_value().map_err(|err| {
        PagesError::validation(format!("Failed to encode sanitized block payload: {err}"))
    })
}

fn sanitize_payload(payload: BlockPayload) -> PagesResult<BlockPayload> {
    match payload {
        BlockPayload::Hero(mut data) => {
            trim_required(&mut data.title, "hero.title")?;
            trim_optional(&mut data.subtitle);
            trim_optional(&mut data.cta_label);
            sanitize_optional_http_url(
                &mut data.background_image_url,
                "hero.background_image_url",
            )?;
            sanitize_optional_http_url(&mut data.cta_url, "hero.cta_url")?;
            Ok(BlockPayload::Hero(data))
        }
        BlockPayload::Text(mut data) => {
            trim_required(&mut data.text, "text.text")?;
            Ok(BlockPayload::Text(data))
        }
        BlockPayload::Image(mut data) => {
            trim_required(&mut data.src, "image.src")?;
            enforce_allowed_url(&data.src, false, "image.src")?;
            trim_optional(&mut data.alt);
            trim_optional(&mut data.caption);
            Ok(BlockPayload::Image(data))
        }
        BlockPayload::Gallery(mut data) => {
            if data.images.is_empty() {
                return Err(PagesError::validation("gallery.images must not be empty"));
            }
            for image in &mut data.images {
                trim_required(&mut image.src, "gallery.images[].src")?;
                enforce_allowed_url(&image.src, false, "gallery.images[].src")?;
                trim_optional(&mut image.alt);
                trim_optional(&mut image.caption);
            }
            Ok(BlockPayload::Gallery(data))
        }
        BlockPayload::Cta(mut data) => {
            trim_required(&mut data.title, "cta.title")?;
            trim_required(&mut data.button_label, "cta.button_label")?;
            trim_required(&mut data.button_url, "cta.button_url")?;
            enforce_allowed_url(&data.button_url, false, "cta.button_url")?;
            trim_optional(&mut data.description);
            Ok(BlockPayload::Cta(data))
        }
        BlockPayload::Features(mut data) => {
            trim_optional(&mut data.title);
            if data.items.is_empty() {
                return Err(PagesError::validation("features.items must not be empty"));
            }
            for item in &mut data.items {
                trim_required(&mut item.title, "features.items[].title")?;
                trim_optional(&mut item.description);
                trim_optional(&mut item.icon);
            }
            Ok(BlockPayload::Features(data))
        }
        BlockPayload::Testimonials(mut data) => {
            trim_optional(&mut data.title);
            if data.items.is_empty() {
                return Err(PagesError::validation(
                    "testimonials.items must not be empty",
                ));
            }
            for item in &mut data.items {
                trim_required(&mut item.quote, "testimonials.items[].quote")?;
                trim_required(&mut item.author, "testimonials.items[].author")?;
                trim_optional(&mut item.role);
            }
            Ok(BlockPayload::Testimonials(data))
        }
        BlockPayload::Pricing(mut data) => {
            trim_optional(&mut data.title);
            if data.plans.is_empty() {
                return Err(PagesError::validation("pricing.plans must not be empty"));
            }
            for plan in &mut data.plans {
                trim_required(&mut plan.name, "pricing.plans[].name")?;
                trim_required(&mut plan.price, "pricing.plans[].price")?;
                trim_optional(&mut plan.period);
                if plan.features.is_empty() {
                    return Err(PagesError::validation(
                        "pricing.plans[].features must not be empty",
                    ));
                }
                for feature in &mut plan.features {
                    trim_required(feature, "pricing.plans[].features[]")?;
                }
                trim_optional(&mut plan.cta_label);
                sanitize_optional_http_url(&mut plan.cta_url, "pricing.plans[].cta_url")?;
            }
            Ok(BlockPayload::Pricing(data))
        }
        BlockPayload::Faq(mut data) => {
            trim_optional(&mut data.title);
            if data.items.is_empty() {
                return Err(PagesError::validation("faq.items must not be empty"));
            }
            for item in &mut data.items {
                trim_required(&mut item.question, "faq.items[].question")?;
                trim_required(&mut item.answer, "faq.items[].answer")?;
            }
            Ok(BlockPayload::Faq(data))
        }
        BlockPayload::Contact(mut data) => {
            trim_optional(&mut data.title);
            trim_optional(&mut data.description);
            trim_optional(&mut data.email);
            trim_optional(&mut data.phone);
            trim_optional(&mut data.address);
            Ok(BlockPayload::Contact(data))
        }
        BlockPayload::ProductGrid(mut data) => {
            trim_optional(&mut data.title);
            if data.product_ids.is_empty() {
                return Err(PagesError::validation(
                    "product_grid.product_ids must not be empty",
                ));
            }
            Ok(BlockPayload::ProductGrid(data))
        }
        BlockPayload::Newsletter(mut data) => {
            trim_optional(&mut data.title);
            trim_optional(&mut data.description);
            trim_optional(&mut data.submit_label);
            Ok(BlockPayload::Newsletter(data))
        }
        BlockPayload::Video(mut data) => {
            data.provider = data.provider.trim().to_lowercase();
            trim_required(&mut data.url, "video.url")?;
            if !is_allowed_embed(&data.provider, &data.url) {
                return Err(PagesError::validation(
                    "video.provider/video.url is not allowed by embed policy",
                ));
            }
            trim_optional(&mut data.title);
            Ok(BlockPayload::Video(data))
        }
        BlockPayload::Html(mut data) => {
            data.html = sanitize_html_fragment(&data.html)?;
            Ok(BlockPayload::Html(data))
        }
        BlockPayload::Spacer(data) => Ok(BlockPayload::Spacer(data)),
    }
}

fn trim_required(field: &mut String, name: &str) -> PagesResult<()> {
    *field = field.trim().to_string();
    if field.is_empty() {
        return Err(PagesError::validation(format!("{name} must not be empty")));
    }
    Ok(())
}

fn trim_optional(field: &mut Option<String>) {
    if let Some(value) = field {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            *field = None;
        } else {
            *value = trimmed.to_string();
        }
    }
}

fn sanitize_optional_http_url(field: &mut Option<String>, name: &str) -> PagesResult<()> {
    if let Some(url) = field {
        *url = url.trim().to_string();
        enforce_allowed_url(url, false, name)?;
    }
    Ok(())
}

fn enforce_allowed_url(raw: &str, allow_mailto: bool, field_name: &str) -> PagesResult<()> {
    if !is_allowed_url(raw, allow_mailto) {
        return Err(PagesError::validation(format!(
            "{field_name} uses forbidden URL scheme or format"
        )));
    }
    Ok(())
}

fn is_allowed_url(raw: &str, allow_mailto: bool) -> bool {
    let Ok(url) = Url::parse(raw) else {
        return false;
    };

    match url.scheme() {
        "http" | "https" => true,
        "mailto" => allow_mailto,
        _ => false,
    }
}

fn is_allowed_embed(provider: &str, raw: &str) -> bool {
    let Ok(url) = Url::parse(raw) else {
        return false;
    };
    if url.scheme() != "https" {
        return false;
    }

    let host = url.host_str().unwrap_or_default();
    match provider {
        "youtube" => matches!(host, "youtube.com" | "www.youtube.com" | "youtu.be"),
        "vimeo" => matches!(host, "vimeo.com" | "player.vimeo.com"),
        _ => false,
    }
}

fn sanitize_html_fragment(raw: &str) -> PagesResult<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(PagesError::validation("html.html must not be empty"));
    }

    let lowered = trimmed.to_ascii_lowercase();
    let forbidden = ["<script", "<iframe", "<object", "<embed", "javascript:"];
    if forbidden.iter().any(|needle| lowered.contains(needle)) {
        return Err(PagesError::validation(
            "html.html contains forbidden tags/protocols",
        ));
    }

    if has_inline_event_handler(trimmed) {
        return Err(PagesError::validation(
            "html.html contains forbidden inline event handlers",
        ));
    }

    Ok(trimmed.to_string())
}

fn has_inline_event_handler(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i + 3 < bytes.len() {
        if bytes[i].is_ascii_whitespace()
            && (bytes[i + 1] == b'o' || bytes[i + 1] == b'O')
            && (bytes[i + 2] == b'n' || bytes[i + 2] == b'N')
        {
            let mut j = i + 3;
            let mut has_name = false;
            while j < bytes.len() && bytes[j].is_ascii_alphabetic() {
                has_name = true;
                j += 1;
            }
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if has_name && j < bytes.len() && bytes[j] == b'=' {
                return true;
            }
        }
        i += 1;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hero_payload_validates_and_normalizes() {
        let value = validate_and_sanitize_block_data(
            &BlockType::Hero,
            json!({
                "title": "  Welcome  ",
                "subtitle": "  subtitle ",
                "background_image_url": "https://cdn.example.com/bg.png",
                "cta_label": " Start ",
                "cta_url": "https://example.com/signup"
            }),
        )
        .expect("hero payload should pass");

        assert_eq!(value["title"], "Welcome");
        assert_eq!(value["subtitle"], "subtitle");
        assert_eq!(value["cta_label"], "Start");
    }

    #[test]
    fn hero_payload_rejects_unknown_field() {
        let err = validate_and_sanitize_block_data(
            &BlockType::Hero,
            json!({
                "title": "Welcome",
                "subtitle": "subtitle",
                "danger": true
            }),
        )
        .expect_err("unknown field must be rejected");

        assert!(matches!(err, PagesError::Validation(_)));
    }

    #[test]
    fn image_payload_rejects_forbidden_url_scheme() {
        let err = validate_and_sanitize_block_data(
            &BlockType::Image,
            json!({"src": "javascript:alert(1)", "alt": "x"}),
        )
        .expect_err("javascript URL must be rejected");

        assert!(matches!(err, PagesError::Validation(_)));
    }

    #[test]
    fn video_payload_allows_only_whitelisted_embed_hosts() {
        let err = validate_and_sanitize_block_data(
            &BlockType::Video,
            json!({"provider": "youtube", "url": "https://evil.example/watch?v=1"}),
        )
        .expect_err("non-whitelisted domain must be rejected");

        assert!(matches!(err, PagesError::Validation(_)));
    }

    #[test]
    fn video_payload_accepts_vimeo() {
        let value = validate_and_sanitize_block_data(
            &BlockType::Video,
            json!({"provider": "VIMEO", "url": "https://vimeo.com/123"}),
        )
        .expect("vimeo must pass policy");

        assert_eq!(value["provider"], "vimeo");
    }

    #[test]
    fn html_payload_rejects_script_and_handlers() {
        let script_err = validate_and_sanitize_block_data(
            &BlockType::Html,
            json!({"html": "<div><script>alert(1)</script></div>"}),
        )
        .expect_err("script tags must be rejected");
        assert!(matches!(script_err, PagesError::Validation(_)));

        let handler_err = validate_and_sanitize_block_data(
            &BlockType::Html,
            json!({"html": "<div onclick=\"alert(1)\">x</div>"}),
        )
        .expect_err("inline event handlers must be rejected");
        assert!(matches!(handler_err, PagesError::Validation(_)));
    }

    #[test]
    fn html_payload_accepts_safe_markup() {
        let value = validate_and_sanitize_block_data(
            &BlockType::Html,
            json!({"html": " <div><p>Hello</p></div> "}),
        )
        .expect("safe html should pass");

        assert_eq!(value["html"], "<div><p>Hello</p></div>");
    }

    #[test]
    fn translations_are_sanitized_with_the_same_policy() {
        let result = sanitize_translations(
            &BlockType::Image,
            Some(vec![BlockTranslationInput {
                locale: "en".into(),
                data: json!({"src": "https://img.example/a.png", "alt": "  Pic "}),
            }]),
        )
        .expect("translation should pass");

        let first = result.unwrap().pop().unwrap();
        assert_eq!(first.data["alt"], "Pic");
    }
}
