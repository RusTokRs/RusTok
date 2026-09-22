use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BundleAdminItemRecord {
    pub id: String,
    pub bundle_id: String,
    pub product_id: String,
    pub variant_id: Option<String>,
    pub quantity: i32,
    pub is_optional: bool,
    pub discount_rate: Option<String>,
    pub position: i32,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BundleAdminListItem {
    pub id: String,
    pub tenant_id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub bundle_type: String,
    pub status: String,
    pub discount_type: String,
    pub discount_value: String,
    pub items_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BundleAdminDirectory {
    pub items: Vec<BundleAdminListItem>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BundleAdminFilters {
    pub search: Option<String>,
    pub status: Option<String>,
    pub bundle_type: Option<String>,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BundleAdminTranslation {
    pub locale: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BundleAdminRecord {
    pub id: String,
    pub tenant_id: String,
    pub bundle_product_id: Option<String>,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub bundle_type: String,
    pub status: String,
    pub discount_type: String,
    pub discount_value: String,
    pub metadata: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
    pub translations: Vec<BundleAdminTranslation>,
    pub items: Vec<BundleAdminItemRecord>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BundleAdminCreateDraft {
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub bundle_type: String,
    pub status: String,
    pub discount_type: String,
    pub discount_value: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BundleAdminUpdateDraft {
    pub slug: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub bundle_type: Option<String>,
    pub status: Option<String>,
    pub discount_type: Option<String>,
    pub discount_value: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BundleAdminCommand {
    Create {
        draft: BundleAdminCreateDraft,
    },
    Update {
        id: String,
        draft: BundleAdminUpdateDraft,
    },
    Delete {
        id: String,
    },
    AddItem {
        bundle_id: String,
        product_id: String,
        variant_id: Option<String>,
        quantity: i32,
        is_optional: bool,
    },
    RemoveItem {
        bundle_id: String,
        item_id: String,
    },
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BundleAdminCommandResult {
    pub bundle: Option<BundleAdminRecord>,
    pub success: bool,
}

impl BundleAdminCommandResult {
    pub fn is_success(&self) -> bool {
        self.success
    }

    pub fn error_summary(&self) -> String {
        if self.success {
            String::new()
        } else {
            "Command execution failed".to_string()
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BundleAdminShell {
    pub title: String,
    pub subtitle: String,
    pub empty_state: String,
    pub transport_profile: String,
}
