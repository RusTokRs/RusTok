use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorefrontPagesData {
    pub selected_page: Option<PageDetail>,
    pub pages: PageList,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PageList {
    pub items: Vec<PageListItem>,
    pub total: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PageListItem {
    pub id: String,
    pub title: Option<String>,
    pub slug: Option<String>,
    pub status: String,
    pub template: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PageDetail {
    #[serde(rename = "effectiveLocale")]
    pub effective_locale: Option<String>,
    pub translation: Option<PageTranslation>,
    pub body: Option<PageBody>,
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
}
