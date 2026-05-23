use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PageList {
    pub items: Vec<PageListItem>,
    pub total: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PageListItem {
    pub id: String,
    pub status: String,
    pub template: String,
    pub title: Option<String>,
    pub slug: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PageTranslation {
    pub locale: String,
    pub title: Option<String>,
    pub slug: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PageBody {
    pub locale: String,
    pub content: String,
    pub format: String,
    #[serde(rename = "contentJson")]
    pub content_json: Option<Value>,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PageBlock {
    pub id: String,
    #[serde(rename = "blockType")]
    pub block_type: String,
    pub position: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PageDetail {
    pub id: String,
    pub status: String,
    pub template: String,
    #[serde(rename = "channelSlugs", default)]
    pub channel_slugs: Vec<String>,
    pub translation: Option<PageTranslation>,
    pub body: Option<PageBody>,
    #[serde(default)]
    pub blocks: Vec<PageBlock>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PageMutationResult {
    pub id: String,
    pub status: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    pub translation: Option<PageTranslation>,
}

#[derive(Clone, Debug)]
pub struct CreatePageDraft {
    pub locale: String,
    pub title: String,
    pub slug: String,
    pub body_content: String,
    pub body_format: String,
    pub body_content_json: Value,
    pub template: Option<String>,
    pub channel_slugs: Vec<String>,
    pub publish: bool,
}
