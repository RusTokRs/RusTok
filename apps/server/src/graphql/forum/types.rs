use async_graphql::{InputObject, SimpleObject};

use crate::graphql::connection::ListConnection;
use uuid::Uuid;

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlForumCategory {
    pub id: Uuid,
    pub locale: String,
    pub effective_locale: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub topic_count: i32,
    pub reply_count: i32,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlForumTopic {
    pub id: Uuid,
    pub locale: String,
    pub effective_locale: String,
    pub category_id: Uuid,
    pub author_id: Option<Uuid>,
    pub title: String,
    pub slug: String,
    pub body: String,
    pub status: String,
    pub tags: Vec<String>,
    pub is_pinned: bool,
    pub is_locked: bool,
    pub reply_count: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlForumReply {
    pub id: Uuid,
    pub locale: String,
    pub effective_locale: String,
    pub topic_id: Uuid,
    pub author_id: Option<Uuid>,
    pub content: String,
    pub status: String,
    pub parent_reply_id: Option<Uuid>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(InputObject)]
pub struct CreateForumTopicInput {
    pub locale: String,
    pub category_id: Uuid,
    pub title: String,
    pub slug: Option<String>,
    pub body: String,
    pub tags: Vec<String>,
}

#[derive(InputObject)]
pub struct UpdateForumTopicInput {
    pub locale: String,
    pub title: Option<String>,
    pub body: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(InputObject)]
pub struct CreateForumReplyInput {
    pub locale: String,
    pub content: String,
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
