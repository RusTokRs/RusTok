use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorefrontMenu { pub id: String, #[serde(rename = "effectiveLocale")] pub effective_locale: String,
    pub name: String, pub location: StorefrontMenuLocation, pub items: Vec<StorefrontMenuItem> }
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StorefrontMenuLocation { Header, Footer, Sidebar, Mobile }
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorefrontMenuItem { pub id: String, pub title: String, pub url: String, pub icon: Option<String>,
    #[serde(default)] pub children: Vec<StorefrontMenuItem> }
