use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BrandAdminListItem {
    pub id: String,
    pub tenant_id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub website_url: Option<String>,
    pub logo_url: Option<String>,
    pub is_active: bool,
    pub sort_order: i32,
    pub products_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BrandAdminDirectory {
    pub items: Vec<BrandAdminListItem>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BrandAdminFilters {
    pub search: Option<String>,
    pub is_active: Option<bool>,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BrandAdminTranslation {
    pub locale: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BrandAdminRecord {
    pub id: String,
    pub tenant_id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub website_url: Option<String>,
    pub logo_url: Option<String>,
    pub metadata: serde_json::Value,
    pub is_active: bool,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
    pub translations: Vec<BrandAdminTranslation>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BrandAdminCreateDraft {
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub website_url: Option<String>,
    pub logo_url: Option<String>,
    pub is_active: bool,
    pub sort_order: i32,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BrandAdminUpdateDraft {
    pub slug: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub website_url: Option<String>,
    pub logo_url: Option<String>,
    pub is_active: Option<bool>,
    pub sort_order: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BrandAdminCommand {
    Create {
        draft: BrandAdminCreateDraft,
    },
    Update {
        id: String,
        draft: BrandAdminUpdateDraft,
    },
    Delete {
        id: String,
    },
    SetTranslation {
        id: String,
        locale: String,
        name: String,
        description: Option<String>,
    },
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BrandAdminCommandResult {
    pub brand: Option<BrandAdminRecord>,
    pub success: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BrandAdminShell {
    pub title: String,
    pub subtitle: String,
    pub empty_state: String,
    pub transport_profile: String,
}
