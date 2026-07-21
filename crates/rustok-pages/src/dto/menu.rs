use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MenuLocation {
    Header,
    Footer,
    Sidebar,
    Mobile,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MenuTranslationInput {
    pub locale: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MenuItemTranslationInput {
    pub locale: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateMenuInput {
    pub translations: Vec<MenuTranslationInput>,
    pub location: MenuLocation,
    pub items: Vec<MenuItemInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MenuItemInput {
    pub translations: Vec<MenuItemTranslationInput>,
    pub url: Option<String>,
    pub page_id: Option<Uuid>,
    pub icon: Option<String>,
    pub position: i32,
    pub children: Option<Vec<MenuItemInput>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MenuResponse {
    pub id: Uuid,
    pub effective_locale: String,
    pub available_locales: Vec<String>,
    pub name: String,
    pub location: MenuLocation,
    pub items: Vec<MenuItemResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MenuItemResponse {
    pub id: Uuid,
    pub title: String,
    pub url: String,
    pub icon: Option<String>,
    pub children: Vec<MenuItemResponse>,
}
