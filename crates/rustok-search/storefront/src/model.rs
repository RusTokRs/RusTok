use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchFacetBucket {
    pub value: String,
    #[serde(default)]
    pub label: Option<String>,
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
    #[serde(rename = "presetKey")]
    pub preset_key: Option<String>,
    pub items: Vec<SearchPreviewResultItem>,
    pub total: u64,
    #[serde(rename = "tookMs")]
    pub took_ms: u64,
    pub engine: String,
    #[serde(rename = "rankingProfile")]
    pub ranking_profile: String,
    pub facets: Vec<SearchFacetGroup>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchSuggestion {
    pub text: String,
    pub kind: String,
    #[serde(rename = "documentId")]
    pub document_id: Option<String>,
    #[serde(rename = "entityType")]
    pub entity_type: Option<String>,
    #[serde(rename = "sourceModule")]
    pub source_module: Option<String>,
    pub locale: Option<String>,
    pub url: Option<String>,
    pub score: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SearchFilterPreset {
    pub key: String,
    pub label: String,
    #[serde(rename = "entityTypes")]
    pub entity_types: Vec<String>,
    #[serde(rename = "sourceModules")]
    pub source_modules: Vec<String>,
    pub statuses: Vec<String>,
    #[serde(rename = "rankingProfile")]
    pub ranking_profile: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct SearchAttributeFilter {
    #[serde(rename = "attributeCode")]
    pub attribute_code: String,
    #[serde(default)]
    pub values: Vec<String>,
    #[serde(default)]
    pub min: Option<String>,
    #[serde(default)]
    pub max: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct SearchPreviewFilters {
    #[serde(default, rename = "channelId")]
    pub channel_id: Option<String>,
    #[serde(default)]
    pub entity_types: Vec<String>,
    #[serde(default)]
    pub source_modules: Vec<String>,
    #[serde(default)]
    pub statuses: Vec<String>,
    #[serde(default, rename = "categoryIds")]
    pub category_ids: Vec<String>,
    #[serde(default, rename = "attributeFilters")]
    pub attribute_filters: Vec<SearchAttributeFilter>,
    #[serde(default, rename = "sortAttributeCode")]
    pub sort_attribute_code: Option<String>,
    #[serde(default, rename = "sortDesc")]
    pub sort_desc: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TrackSearchClickPayload {
    pub success: bool,
    pub tracked: bool,
}
