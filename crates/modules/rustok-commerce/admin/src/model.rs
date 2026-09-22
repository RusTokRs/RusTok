use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CurrentTenant {
    pub id: String,
    pub slug: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CommerceAdminBootstrap {
    #[serde(rename = "currentTenant")]
    pub current_tenant: CurrentTenant,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShippingProfileList {
    pub items: Vec<ShippingProfile>,
    pub total: u64,
    pub page: u64,
    #[serde(rename = "perPage")]
    pub per_page: u64,
    #[serde(rename = "hasNext")]
    pub has_next: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShippingProfile {
    pub id: String,
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub active: bool,
    pub metadata: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Clone, Debug)]
pub struct ShippingProfileDraft {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub metadata_json: String,
    pub locale: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommerceCartPromotionKind {
    PercentageDiscount,
    FixedDiscount,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommerceCartPromotionScope {
    Cart,
    LineItem,
    Shipping,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CommerceCartPromotionDraft {
    pub kind: CommerceCartPromotionKind,
    pub scope: CommerceCartPromotionScope,
    pub line_item_id: String,
    pub source_id: String,
    pub discount_percent: String,
    pub amount: String,
    pub metadata_json: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CommerceCartPromotionPreview {
    pub kind: CommerceCartPromotionKind,
    pub scope: CommerceCartPromotionScope,
    pub line_item_id: Option<String>,
    pub currency_code: String,
    pub base_amount: String,
    pub adjustment_amount: String,
    pub adjusted_amount: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CommerceAdminCartAdjustment {
    pub id: String,
    pub line_item_id: Option<String>,
    pub source_type: String,
    pub source_id: Option<String>,
    pub scope: Option<String>,
    pub amount: String,
    pub currency_code: String,
    pub metadata: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CommerceAdminCartSnapshot {
    pub id: String,
    pub currency_code: String,
    pub shipping_total: String,
    pub adjustment_total: String,
    pub total_amount: String,
    pub adjustments: Vec<CommerceAdminCartAdjustment>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CommerceOrderChangeList {
    pub items: Vec<CommerceOrderChange>,
    pub total: u64,
    pub page: u64,
    #[serde(rename = "perPage")]
    pub per_page: u64,
    #[serde(rename = "hasNext")]
    pub has_next: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CommerceOrderChange {
    pub id: String,
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    #[serde(rename = "orderId")]
    pub order_id: String,
    #[serde(rename = "createdBy")]
    pub created_by: String,
    #[serde(rename = "changeType")]
    pub change_type: String,
    pub status: String,
    pub description: Option<String>,
    pub preview: String,
    pub metadata: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "appliedAt")]
    pub applied_at: Option<String>,
    #[serde(rename = "cancelledAt")]
    pub cancelled_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CommerceOrderChangeActionDraft {
    pub metadata_json: String,
    pub reason: String,
}
