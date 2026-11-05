use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MAX_PRICE_RESOLUTION_VARIANTS: usize = rustok_product::MAX_VARIANT_IDENTITY_BATCH;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UpsertVariantPriceRequest {
    pub variant_id: Uuid,
    pub price_list_id: Option<Uuid>,
    pub currency_code: String,
    pub amount: Decimal,
    pub compare_at_amount: Option<Decimal>,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub min_quantity: Option<i32>,
    pub max_quantity: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SetPriceListScopeRequest {
    pub price_list_id: Uuid,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApplyVariantDiscountRequest {
    pub variant_id: Uuid,
    pub price_list_id: Option<Uuid>,
    pub currency_code: String,
    pub discount_percent: Decimal,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetPriceListPercentageRuleRequest {
    pub price_list_id: Uuid,
    pub adjustment_percent: Option<Decimal>,
    pub fallback_locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolveProductPriceRequest {
    pub product_id: Option<Uuid>,
    pub variant_id: Uuid,
    pub region_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub price_list_id: Option<Uuid>,
    pub quantity: Option<i32>,
    pub currency_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolveProductPricesRequest {
    pub product_id: Option<Uuid>,
    pub variant_ids: Vec<Uuid>,
    pub pricing: crate::PriceResolutionContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProductPriceResolutionSnapshot {
    pub product_id: Uuid,
    pub variant_id: Uuid,
    /// None is an authoritative absent price, not a transport/identity/context failure.
    pub price: Option<crate::ResolvedPrice>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResolvedProductPricesSnapshot {
    pub context: crate::PriceResolutionContext,
    pub resolved_at: chrono::DateTime<chrono::Utc>,
    pub items: Vec<ProductPriceResolutionSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorefrontProductPricingListRequest {
    pub fallback_locale: Option<String>,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PriceListProjectionRequest {
    pub price_list_id: Uuid,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActivePriceListProjectionRequest {
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub fallback_locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdminProductPricingProjectionRequest {
    pub product_id: Uuid,
    pub fallback_locale: Option<String>,
    pub selected_price_list_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorefrontProductPricingProjectionRequest {
    pub public_channel_id: Option<Uuid>,
    pub handle: String,
    pub fallback_locale: Option<String>,
    pub public_channel_slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PreviewVariantDiscountRequest {
    pub variant_id: Uuid,
    pub price_list_id: Option<Uuid>,
    pub currency_code: String,
    pub discount_percent: Decimal,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResolvedProductPriceSnapshot {
    pub source: crate::PriceResolutionSource,
    pub context: crate::PriceResolutionContext,
    pub resolved_at: chrono::DateTime<chrono::Utc>,
    pub product_id: Uuid,
    pub variant_id: Uuid,
    pub currency_code: String,
    pub amount: Decimal,
    pub compare_at_amount: Option<Decimal>,
    pub discount_percent: Option<Decimal>,
    pub on_sale: bool,
    pub region_id: Option<Uuid>,
    pub min_quantity: Option<i32>,
    pub max_quantity: Option<i32>,
    pub price_list_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
}

impl From<ResolvedProductPriceSnapshot> for crate::ResolvedPrice {
    fn from(snapshot: ResolvedProductPriceSnapshot) -> Self {
        Self {
            source: snapshot.source,
            currency_code: snapshot.currency_code,
            amount: snapshot.amount,
            compare_at_amount: snapshot.compare_at_amount,
            discount_percent: snapshot.discount_percent,
            on_sale: snapshot.on_sale,
            region_id: snapshot.region_id,
            min_quantity: snapshot.min_quantity,
            max_quantity: snapshot.max_quantity,
            price_list_id: snapshot.price_list_id,
            channel_id: snapshot.channel_id,
            channel_slug: snapshot.channel_slug,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PriceListProjectionSnapshot {
    pub price_list_id: Uuid,
    pub title: String,
    pub currency_code: Option<String>,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActivePriceListProjectionSnapshot {
    pub price_list_id: Uuid,
    pub title: String,
    pub list_type: String,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub rule_kind: Option<String>,
    pub adjustment_percent: Option<Decimal>,
}
