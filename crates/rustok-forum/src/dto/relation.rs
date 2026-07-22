use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ForumQuoteTargetKindInput {
    Topic,
    Reply,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
pub struct ForumQuoteReferenceInput {
    pub target_kind: ForumQuoteTargetKindInput,
    pub target_id: Uuid,
    pub revision_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SetForumQuotesInput {
    pub locale: String,
    #[serde(default)]
    pub quotes: Vec<ForumQuoteReferenceInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ForumRelationSnapshotQuery {
    pub target_kind: String,
    pub target_id: Uuid,
    pub locale: String,
    pub revision_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ForumRelationQuoteResponse {
    pub target_kind: String,
    pub target_id: Uuid,
    pub revision_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ForumRelationSnapshotResponse {
    pub revision_id: i64,
    pub target_kind: String,
    pub target_id: Uuid,
    pub locale: String,
    pub user_ids: Vec<Uuid>,
    pub audiences: Vec<String>,
    pub quotes: Vec<ForumRelationQuoteResponse>,
    pub created_at: String,
}
