use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProductRelationItem {
    pub id: String,
    #[serde(rename = "productId")]
    pub product_id: String,
    #[serde(rename = "relatedProductId")]
    pub related_product_id: String,
    #[serde(rename = "relationType")]
    pub relation_type: String,
    pub position: i32,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(rename = "createdAt", default)]
    pub created_at: String,
    #[serde(rename = "updatedAt", default)]
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default, PartialEq, Eq)]
pub struct CreateProductRelationDraft {
    #[serde(rename = "productId")]
    pub product_id: String,
    #[serde(rename = "relatedProductId")]
    pub related_product_id: String,
    #[serde(rename = "relationType")]
    pub relation_type: String,
    pub position: Option<i32>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductRelationsPanelCopy {
    pub title: String,
    pub subtitle: String,
    pub tab_cross_sell: String,
    pub tab_up_sell: String,
    pub tab_related: String,
    pub tab_accessory: String,
    pub tab_alternative: String,
    pub add: String,
    pub target_product_id: String,
    pub position: String,
    pub empty: String,
    pub remove: String,
    pub move_up: String,
    pub move_down: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProductRelationsAdminFilters {
    pub product_id: Option<String>,
    pub relation_type: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProductRelationsAdminCommand {
    Add {
        draft: CreateProductRelationDraft,
    },
    Remove {
        id: String,
    },
    Reorder {
        product_id: String,
        relation_type: String,
        ordered_ids: Vec<String>,
    },
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProductRelationsAdminCommandResult {
    pub item: Option<ProductRelationItem>,
    pub items: Vec<ProductRelationItem>,
    pub success: bool,
}

impl ProductRelationsAdminCommandResult {
    pub fn is_success(&self) -> bool {
        self.success
    }
}
