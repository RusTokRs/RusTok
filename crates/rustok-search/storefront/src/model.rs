use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchFacetBucket {
    pub value: String,
    pub count: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchFacetGroup {
    pub name: String,
    pub buckets: Vec<SearchFacetBucket>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchPreviewResultItem {
    pub id: String,
    #[serde(rename = "entityType")]
    pub entity_type: String,
    #[serde(rename = "sourceModule")]
    pub source_module: String,
    pub title: String,
    pub snippet: Option<String>,
    pub score: f64,
    pub locale: Option<String>,
    pub url: Option<String>,
    pub payload: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchPreviewPayload {
    #[serde(rename = "queryLogId")]
    pub query_log_id: Option<String>,
    pub items: Vec<SearchPreviewResultItem>,
    pub total: u64,
    #[serde(rename = "tookMs")]
    pub took_ms: u64,
    pub engine: String,
    pub facets: Vec<SearchFacetGroup>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchPreviewFilters {
    pub entity_types: Vec<String>,
    pub source_modules: Vec<String>,
    pub statuses: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TrackSearchClickPayload {
    pub success: bool,
    pub tracked: bool,
}
