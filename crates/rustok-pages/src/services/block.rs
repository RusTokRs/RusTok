use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, QueryFilter, QueryOrder, TransactionTrait,
};
use serde_json::Value;
use tracing::instrument;
use url::Url;
use uuid::Uuid;

use rustok_content::entities::node::ContentStatus;
use rustok_core::{Action, Resource, SecurityContext};
use rustok_outbox::TransactionalEventBus;

use crate::dto::*;
use crate::entities::{page, page_block};
use crate::error::{PagesError, PagesResult};
use crate::services::rbac::{can_read_non_public_pages, enforce_owned_scope, enforce_scope};

pub struct BlockService {
    db: DatabaseConnection,
}

impl BlockService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        let _ = event_bus;
        Self { db }
    }

    pub async fn create_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        input: CreateBlockInput,
    ) -> PagesResult<BlockResponse> {
        let parent_page = find_page_in_tx(txn, tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Update,
            parent_page.author_id,
        )?;
        let data = validate_and_sanitize_block_data(&input.block_type, input.data)?;
        let translations = sanitize_translations(&input.block_type, input.translations)?;
        let now = Utc::now();
        let block_id = Uuid::new_v4();

        page_block::ActiveModel {
            id: Set(block_id),
            page_id: Set(page_id),
            tenant_id: Set(tenant_id),
            block_type: Set(block_type_str(&input.block_type).to_string()),
            position: Set(input.position),
            data: Set(data.clone()),
            translations: Set(translations.clone().map(|items| serde_json::json!(items))),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(txn)
        .await?;

        Ok(BlockResponse {
            id: block_id,
            block_type: input.block_type,
            position: input.position,
            data,
            translations,
        })
    }

    pub async fn delete_all_for_page_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        page_id: Uuid,
    ) -> PagesResult<()> {
        page_block::Entity::delete_many()
            .filter(page_block::Column::TenantId.eq(tenant_id))
            .filter(page_block::Column::PageId.eq(page_id))
            .exec(txn)
            .await?;
        Ok(())
    }

    async fn list_models_for_page(
        &self,
        tenant_id: Uuid,
        page_id: Uuid,
    ) -> PagesResult<Vec<page_block::Model>> {
        Ok(page_block::Entity::find()
            .filter(page_block::Column::TenantId.eq(tenant_id))
            .filter(page_block::Column::PageId.eq(page_id))
            .order_by_asc(page_block::Column::Position)
            .all(&self.db)
            .await?)
    }

    async fn find_block(&self, tenant_id: Uuid, block_id: Uuid) -> PagesResult<page_block::Model> {
        page_block::Entity::find_by_id(block_id)
            .filter(page_block::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| PagesError::block_not_found(block_id))
    }

    async fn find_page_for_block(
        &self,
        tenant_id: Uuid,
        page_id: Uuid,
    ) -> PagesResult<page::Model> {
        page::Entity::find_by_id(page_id)
            .filter(page::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| PagesError::page_not_found(page_id))
    }

    async fn find_block_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        block_id: Uuid,
    ) -> PagesResult<page_block::Model> {
        page_block::Entity::find_by_id(block_id)
            .filter(page_block::Column::TenantId.eq(tenant_id))
            .one(txn)
            .await?
            .ok_or_else(|| PagesError::block_not_found(block_id))
    }

    fn model_to_block(model: page_block::Model) -> PagesResult<BlockResponse> {
        let block_type = block_type_from_str(&model.block_type)?;
        let translations = model
            .translations
            .and_then(|value| serde_json::from_value(value).ok());

        Ok(BlockResponse {
            id: model.id,
            block_type,
            position: model.position,
            data: model.data,
            translations,
        })
    }

    #[instrument(skip(self, input))]
    pub async fn create(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        input: CreateBlockInput,
    ) -> PagesResult<BlockResponse> {
        let txn = self.db.begin().await?;
        let block = Self::create_in_tx(&txn, tenant_id, security, page_id, input).await?;
        txn.commit().await?;
        Ok(block)
    }

    #[instrument(skip(self))]
    pub async fn list_for_page(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
    ) -> PagesResult<Vec<BlockResponse>> {
        enforce_scope(&security, Resource::Pages, Action::Read)?;
        let parent_page = self.find_page_for_block(tenant_id, page_id).await?;
        if !can_read_non_public_pages(&security)
            && storage_to_status(&parent_page.status)?
                != rustok_content::entities::node::ContentStatus::Published
        {
            return Err(PagesError::forbidden("Permission denied"));
        }
        let blocks = self.list_models_for_page(tenant_id, page_id).await?;
        let mut responses = Vec::with_capacity(blocks.len());
        for block in blocks {
            responses.push(Self::model_to_block(block)?);
        }
        Ok(responses)
    }

    #[instrument(skip(self, input))]
    pub async fn update(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        block_id: Uuid,
        input: UpdateBlockInput,
    ) -> PagesResult<BlockResponse> {
        let existing = self.find_block(tenant_id, block_id).await?;
        let parent_page = self
            .find_page_for_block(tenant_id, existing.page_id)
            .await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Update,
            parent_page.author_id,
        )?;
        let block_type = block_type_from_str(&existing.block_type)?;

        let mut data = existing.data.clone();
        let mut translations = existing
            .translations
            .clone()
            .and_then(|value| serde_json::from_value(value).ok());
        if let Some(input_data) = input.data {
            data = validate_and_sanitize_block_data(&block_type, input_data)?;
        }
        if let Some(input_translations) = input.translations {
            translations = sanitize_translations(&block_type, Some(input_translations))?;
        }

        let mut active: page_block::ActiveModel = existing.into();
        if let Some(position) = input.position {
            active.position = Set(position);
        }
        active.data = Set(data.clone());
        active.translations = Set(translations.clone().map(|items| serde_json::json!(items)));
        active.updated_at = Set(Utc::now().into());
        let block = active.update(&self.db).await?;

        Ok(BlockResponse {
            id: block.id,
            block_type,
            position: block.position,
            data,
            translations,
        })
    }

    #[instrument(skip(self))]
    pub async fn reorder(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        block_order: Vec<Uuid>,
    ) -> PagesResult<()> {
        let txn = self.db.begin().await?;
        let parent_page = find_page_in_tx(&txn, tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Update,
            parent_page.author_id,
        )?;
        for (position, block_id) in block_order.into_iter().enumerate() {
            let block = Self::find_block_in_tx(&txn, tenant_id, block_id).await?;
            if block.page_id != page_id {
                return Err(PagesError::block_not_found(block_id));
            }
            let mut active: page_block::ActiveModel = block.into();
            active.position = Set(position as i32);
            active.updated_at = Set(Utc::now().into());
            active.update(&txn).await?;
        }
        txn.commit().await?;
        Ok(())
    }

    pub async fn delete(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        block_id: Uuid,
    ) -> PagesResult<()> {
        let block = self.find_block(tenant_id, block_id).await?;
        let parent_page = self.find_page_for_block(tenant_id, block.page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Delete,
            parent_page.author_id,
        )?;
        page_block::Entity::delete_by_id(block.id)
            .exec(&self.db)
            .await?;
        Ok(())
    }

    pub async fn delete_all_for_page(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
    ) -> PagesResult<()> {
        let parent_page = self.find_page_for_block(tenant_id, page_id).await?;
        enforce_owned_scope(
            &security,
            Resource::Pages,
            Action::Delete,
            parent_page.author_id,
        )?;
        page_block::Entity::delete_many()
            .filter(page_block::Column::TenantId.eq(tenant_id))
            .filter(page_block::Column::PageId.eq(page_id))
            .exec(&self.db)
            .await?;
        Ok(())
    }
}

async fn find_page_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
) -> PagesResult<page::Model> {
    page::Entity::find_by_id(page_id)
        .filter(page::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await?
        .ok_or_else(|| PagesError::page_not_found(page_id))
}

fn storage_to_status(status: &str) -> PagesResult<ContentStatus> {
    Ok(match status {
        "draft" => ContentStatus::Draft,
        "published" => ContentStatus::Published,
        "archived" => ContentStatus::Archived,
        other => {
            return Err(PagesError::validation(format!(
                "Unknown page status: {other}"
            )))
        }
    })
}

fn block_type_str(block_type: &BlockType) -> &'static str {
    match block_type {
        BlockType::Hero => "hero",
        BlockType::Text => "text",
        BlockType::Image => "image",
        BlockType::Gallery => "gallery",
        BlockType::Cta => "cta",
        BlockType::Features => "features",
        BlockType::Testimonials => "testimonials",
        BlockType::Pricing => "pricing",
        BlockType::Faq => "faq",
        BlockType::Contact => "contact",
        BlockType::ProductGrid => "product_grid",
        BlockType::Newsletter => "newsletter",
        BlockType::Video => "video",
        BlockType::Html => "html",
        BlockType::Spacer => "spacer",
    }
}

fn block_type_from_str(value: &str) -> PagesResult<BlockType> {
    Ok(match value {
        "hero" => BlockType::Hero,
        "text" => BlockType::Text,
        "image" => BlockType::Image,
        "gallery" => BlockType::Gallery,
        "cta" => BlockType::Cta,
        "features" => BlockType::Features,
        "testimonials" => BlockType::Testimonials,
        "pricing" => BlockType::Pricing,
        "faq" => BlockType::Faq,
        "contact" => BlockType::Contact,
        "product_grid" => BlockType::ProductGrid,
        "newsletter" => BlockType::Newsletter,
        "video" => BlockType::Video,
        "html" => BlockType::Html,
        "spacer" => BlockType::Spacer,
        other => {
            return Err(PagesError::validation(format!(
                "Unknown block type in storage: {other}"
            )))
        }
    })
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
