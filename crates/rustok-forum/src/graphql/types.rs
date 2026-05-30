use async_graphql::{InputObject, SimpleObject};
use rustok_profiles::graphql::GqlProfileSummary;
use serde_json::Value;
use uuid::Uuid;

use crate::graphql::connection::ListConnection;

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlForumCategory {
    pub id: Uuid,
    pub requested_locale: String,
    pub locale: String,
    pub effective_locale: String,
    pub available_locales: Vec<String>,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub topic_count: i32,
    pub reply_count: i32,
    pub is_subscribed: bool,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlForumTopic {
    pub id: Uuid,
    pub requested_locale: String,
    pub locale: String,
    pub effective_locale: String,
    pub available_locales: Vec<String>,
    pub category_id: Uuid,
    pub author_id: Option<Uuid>,
    pub author_profile: Option<GqlProfileSummary>,
    pub title: String,
    pub slug: String,
    pub body: String,
    pub body_format: String,
    pub metadata: Value,
    pub status: String,
    pub tags: Vec<String>,
    pub channel_slugs: Vec<String>,
    pub vote_score: i32,
    pub current_user_vote: Option<i32>,
    pub is_subscribed: bool,
    pub solution_reply_id: Option<Uuid>,
    pub is_pinned: bool,
    pub is_locked: bool,
    pub reply_count: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlForumReply {
    pub id: Uuid,
    pub requested_locale: String,
    pub locale: String,
    pub effective_locale: String,
    pub topic_id: Uuid,
    pub author_id: Option<Uuid>,
    pub author_profile: Option<GqlProfileSummary>,
    pub content: String,
    pub content_format: String,
    pub status: String,
    pub vote_score: i32,
    pub current_user_vote: Option<i32>,
    pub is_solution: bool,
    pub parent_reply_id: Option<Uuid>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlForumUserStats {
    pub user_id: Uuid,
    pub topic_count: i32,
    pub reply_count: i32,
    pub solution_count: i32,
    pub updated_at: String,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlForumWidgetCatalog {
    pub catalog_version: String,
    pub builder_contract_version: String,
    pub consumer_min_version: String,
    pub compatibility_matrix: Vec<GqlForumWidgetCompatibilityEntry>,
    pub items: Vec<GqlForumWidgetCatalogItem>,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlForumWidgetCompatibilityEntry {
    pub provider_contract_version: String,
    pub consumer_min_version: String,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlForumWidgetCatalogItem {
    pub widget_type: String,
    pub data_contract_version: String,
    pub props_schema: Value,
    pub capability_requirements: GqlForumWidgetCapabilityRequirements,
    pub fallback_mode: String,
    pub error_mapping: GqlForumWidgetErrorMapping,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlForumWidgetCapabilityRequirements {
    pub preview: bool,
    pub publish: bool,
    pub moderation_view: bool,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlForumWidgetErrorMapping {
    pub validation: String,
    pub sanitize: String,
    pub rbac: String,
    pub runtime: String,
}

#[derive(InputObject)]
pub struct CreateForumTopicInput {
    pub locale: String,
    pub category_id: Uuid,
    pub title: String,
    pub slug: Option<String>,
    pub body: String,
    pub body_format: Option<String>,
    pub content_json: Option<Value>,
    pub metadata: Option<Value>,
    pub tags: Vec<String>,
    pub channel_slugs: Option<Vec<String>>,
}

#[derive(InputObject)]
pub struct UpdateForumTopicInput {
    pub locale: String,
    pub title: Option<String>,
    pub body: Option<String>,
    pub body_format: Option<String>,
    pub content_json: Option<Value>,
    pub metadata: Option<Value>,
    pub tags: Option<Vec<String>>,
    pub channel_slugs: Option<Vec<String>>,
}

#[derive(InputObject)]
pub struct CreateForumReplyInput {
    pub locale: String,
    pub content: String,
    pub content_format: Option<String>,
    pub content_json: Option<Value>,
    pub parent_reply_id: Option<Uuid>,
}

#[derive(InputObject)]
pub struct CreateForumCategoryInput {
    pub locale: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub parent_id: Option<Uuid>,
    pub position: Option<i32>,
    pub moderated: bool,
}

pub type ForumCategoryConnection = ListConnection<GqlForumCategory>;
pub type ForumTopicConnection = ListConnection<GqlForumTopic>;
pub type ForumReplyConnection = ListConnection<GqlForumReply>;
