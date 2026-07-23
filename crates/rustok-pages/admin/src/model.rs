use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PageList {
    pub items: Vec<PageListItem>,
    pub total: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PageListItem {
    pub id: String,
    #[serde(deserialize_with = "deserialize_page_status")]
    pub status: String,
    pub template: String,
    pub title: Option<String>,
    pub slug: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

fn deserialize_page_status<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    match value.as_str() {
        "draft" | "published" | "archived" => Ok(value),
        _ => Err(D::Error::custom(format!(
            "unsupported Pages status `{value}`"
        ))),
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PageTranslation {
    pub locale: String,
    pub title: Option<String>,
    pub slug: Option<String>,
    #[serde(rename = "metaTitle")]
    pub meta_title: Option<String>,
    #[serde(rename = "metaDescription")]
    pub meta_description: Option<String>,
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
pub struct PageDetail {
    pub id: String,
    pub version: i32,
    pub status: String,
    pub template: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "availableLocales", default)]
    pub available_locales: Vec<String>,
    #[serde(rename = "channelSlugs", default)]
    pub channel_slugs: Vec<String>,
    pub translation: Option<PageTranslation>,
    pub body: Option<PageBody>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PageMutationResult {
    pub id: String,
    pub version: i32,
    pub status: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    pub translation: Option<PageTranslation>,
}

impl From<&PageDetail> for PageMutationResult {
    fn from(page: &PageDetail) -> Self {
        Self {
            id: page.id.clone(),
            version: page.version,
            status: page.status.clone(),
            updated_at: page.updated_at.clone(),
            translation: page.translation.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PublishPageReceipt {
    #[serde(rename = "operationId")]
    pub operation_id: String,
    #[serde(rename = "pageId")]
    pub page_id: String,
    pub version: i32,
    #[serde(rename = "idempotencyKey")]
    pub idempotency_key: String,
    #[serde(rename = "reviewHash")]
    pub review_hash: String,
    #[serde(rename = "sanitizedSetHash")]
    pub sanitized_set_hash: String,
    #[serde(rename = "artifactSetHash")]
    pub artifact_set_hash: String,
    pub replayed: bool,
    #[serde(rename = "publishedAt")]
    pub published_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RollbackPageReceipt {
    #[serde(rename = "operationId")]
    pub operation_id: String,
    #[serde(rename = "pageId")]
    pub page_id: String,
    pub version: i32,
    #[serde(rename = "idempotencyKey")]
    pub idempotency_key: String,
    #[serde(rename = "targetPublishOperationId")]
    pub target_publish_operation_id: String,
    #[serde(rename = "sourceArtifactSetHash")]
    pub source_artifact_set_hash: String,
    #[serde(rename = "targetArtifactSetHash")]
    pub target_artifact_set_hash: String,
    pub replayed: bool,
    #[serde(rename = "rolledBackAt")]
    pub rolled_back_at: String,
}

#[derive(Clone, Debug)]
pub enum PagePublicationResult {
    Published(PublishPageReceipt),
    RolledBack(RollbackPageReceipt),
    Unpublished(PageMutationResult),
}

impl PagePublicationResult {
    pub fn page_id(&self) -> &str {
        match self {
            Self::Published(receipt) => &receipt.page_id,
            Self::RolledBack(receipt) => &receipt.page_id,
            Self::Unpublished(page) => &page.id,
        }
    }

    pub fn version(&self) -> i32 {
        match self {
            Self::Published(receipt) => receipt.version,
            Self::RolledBack(receipt) => receipt.version,
            Self::Unpublished(page) => page.version,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct PageBuilderScenarioReleaseStatus {
    #[serde(rename = "pageId")]
    pub page_id: String,
    #[serde(rename = "baselinePresent")]
    pub baseline_present: bool,
    pub allowed: bool,
    pub status: String,
    #[serde(rename = "baselineId")]
    pub baseline_id: Option<String>,
    #[serde(rename = "baselineHash")]
    pub baseline_hash: Option<String>,
    #[serde(rename = "visualChanges")]
    pub visual_changes: i32,
    #[serde(rename = "breakingChanges")]
    pub breaking_changes: i32,
    pub diagnostics: Value,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
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
