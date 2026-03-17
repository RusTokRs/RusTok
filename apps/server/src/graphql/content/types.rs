use async_graphql::{Enum, InputObject, SimpleObject};
use uuid::Uuid;

use rustok_content::dto;
use rustok_content::{available_locales_from, resolve_by_locale_with_fallback};

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlContentStatus {
    Draft,
    Published,
    Archived,
}

impl From<rustok_content::entities::node::ContentStatus> for GqlContentStatus {
    fn from(status: rustok_content::entities::node::ContentStatus) -> Self {
        match status {
            rustok_content::entities::node::ContentStatus::Draft => GqlContentStatus::Draft,
            rustok_content::entities::node::ContentStatus::Published => GqlContentStatus::Published,
            rustok_content::entities::node::ContentStatus::Archived => GqlContentStatus::Archived,
        }
    }
}

impl From<GqlContentStatus> for rustok_content::entities::node::ContentStatus {
    fn from(status: GqlContentStatus) -> Self {
        match status {
            GqlContentStatus::Draft => rustok_content::entities::node::ContentStatus::Draft,
            GqlContentStatus::Published => rustok_content::entities::node::ContentStatus::Published,
            GqlContentStatus::Archived => rustok_content::entities::node::ContentStatus::Archived,
        }
    }
}

#[derive(SimpleObject)]
pub struct GqlNode {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub kind: String,
    pub status: GqlContentStatus,
    pub requested_locale: Option<String>,
    pub effective_locale: Option<String>,
    pub available_locales: Vec<String>,
    pub parent_id: Option<Uuid>,
    pub author_id: Option<Uuid>,
    pub category_id: Option<Uuid>,
    pub position: i32,
    pub depth: i32,
    pub reply_count: i32,
    pub created_at: String,
    pub updated_at: String,
    pub published_at: Option<String>,
    pub translation: Option<GqlNodeTranslation>,
    pub body: Option<GqlBody>,
    pub translations: Vec<GqlNodeTranslation>,
    pub bodies: Vec<GqlBody>,
}

#[derive(SimpleObject)]
pub struct GqlNodeTranslation {
    pub locale: String,
    pub title: Option<String>,
    pub slug: Option<String>,
    pub excerpt: Option<String>,
}

#[derive(SimpleObject)]
pub struct GqlBody {
    pub locale: String,
    pub body: Option<String>,
    pub format: String,
    pub updated_at: String,
}

#[derive(SimpleObject)]
pub struct GqlNodeListItem {
    pub id: Uuid,
    pub kind: String,
    pub status: GqlContentStatus,
    pub effective_locale: String,
    pub title: Option<String>,
    pub slug: Option<String>,
    pub excerpt: Option<String>,
    pub author_id: Option<Uuid>,
    pub created_at: String,
    pub published_at: Option<String>,
}

#[derive(SimpleObject)]
pub struct GqlNodeList {
    pub items: Vec<GqlNodeListItem>,
    pub total: u64,
}

#[derive(InputObject)]
pub struct CreateNodeInput {
    pub parent_id: Option<Uuid>,
    pub author_id: Option<Uuid>,
    pub kind: String,
    pub category_id: Option<Uuid>,
    pub status: Option<GqlContentStatus>,
    pub position: Option<i32>,
    pub depth: Option<i32>,
    pub reply_count: Option<i32>,
    pub metadata: Option<serde_json::Value>,
    pub translations: Vec<NodeTranslationInput>,
    pub bodies: Vec<BodyInput>,
}

#[derive(InputObject)]
pub struct UpdateNodeInput {
    pub parent_id: Option<Uuid>,
    pub author_id: Option<Uuid>,
    pub category_id: Option<Uuid>,
    pub status: Option<GqlContentStatus>,
    pub position: Option<i32>,
    pub depth: Option<i32>,
    pub reply_count: Option<i32>,
    pub expected_version: Option<i32>,
    pub translations: Option<Vec<NodeTranslationInput>>,
    pub bodies: Option<Vec<BodyInput>>,
}

#[derive(InputObject)]
pub struct NodeTranslationInput {
    pub locale: String,
    pub title: Option<String>,
    pub slug: Option<String>,
    pub excerpt: Option<String>,
}

#[derive(InputObject)]
pub struct BodyInput {
    pub locale: String,
    pub body: Option<String>,
    pub format: Option<String>,
}

#[derive(InputObject)]
pub struct NodesFilter {
    pub kind: Option<String>,
    pub status: Option<GqlContentStatus>,
    pub parent_id: Option<Uuid>,
    pub author_id: Option<Uuid>,
    pub locale: Option<String>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

impl From<dto::NodeResponse> for GqlNode {
    fn from(node: dto::NodeResponse) -> Self {
        Self::from_node_with_locale(node, None, None)
    }
}

impl GqlNode {
    pub fn from_node_with_locale(
        node: dto::NodeResponse,
        requested_locale: Option<&str>,
        fallback_locale: Option<&str>,
    ) -> Self {
        let selected_translation = requested_locale.and_then(|locale| {
            resolve_by_locale_with_fallback(&node.translations, locale, fallback_locale, |tr| {
                &tr.locale
            })
            .item
            .cloned()
            .map(Into::into)
        });

        let selected_body = requested_locale.and_then(|locale| {
            resolve_by_locale_with_fallback(&node.bodies, locale, fallback_locale, |body| {
                &body.locale
            })
            .item
            .cloned()
            .map(Into::into)
        });

        let effective_locale = requested_locale.map(|locale| {
            let resolved_translation = resolve_by_locale_with_fallback(
                &node.translations,
                locale,
                fallback_locale,
                |tr| &tr.locale,
            );
            if resolved_translation.item.is_some() {
                return resolved_translation.effective_locale;
            }

            resolve_by_locale_with_fallback(&node.bodies, locale, fallback_locale, |body| {
                &body.locale
            })
            .effective_locale
        });

        Self {
            id: node.id,
            tenant_id: node.tenant_id,
            kind: node.kind,
            status: node.status.into(),
            requested_locale: requested_locale.map(str::to_string),
            effective_locale,
            available_locales: available_locales_from(&node.translations, |tr| &tr.locale),
            parent_id: node.parent_id,
            author_id: node.author_id,
            category_id: node.category_id,
            position: node.position,
            depth: node.depth,
            reply_count: node.reply_count,
            created_at: node.created_at,
            updated_at: node.updated_at,
            published_at: node.published_at,
            translation: selected_translation,
            body: selected_body,
            translations: node.translations.into_iter().map(Into::into).collect(),
            bodies: node.bodies.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<dto::NodeTranslationResponse> for GqlNodeTranslation {
    fn from(t: dto::NodeTranslationResponse) -> Self {
        Self {
            locale: t.locale,
            title: t.title,
            slug: t.slug,
            excerpt: t.excerpt,
        }
    }
}

impl From<dto::BodyResponse> for GqlBody {
    fn from(b: dto::BodyResponse) -> Self {
        Self {
            locale: b.locale,
            body: b.body,
            format: b.format,
            updated_at: b.updated_at,
        }
    }
}

impl From<dto::NodeListItem> for GqlNodeListItem {
    fn from(item: dto::NodeListItem) -> Self {
        Self {
            id: item.id,
            kind: item.kind,
            status: item.status.into(),
            effective_locale: item.effective_locale,
            title: item.title,
            slug: item.slug,
            excerpt: item.excerpt,
            author_id: item.author_id,
            created_at: item.created_at,
            published_at: item.published_at,
        }
    }
}
