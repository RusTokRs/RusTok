use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use rustok_commerce_foundation::entities::product::ProductStatus;
use rustok_commerce_foundation::error::{CommerceError, CommerceResult};
use rustok_core::money;

pub const MAX_PRICE_RESOLUTION_ROWS: usize = 4096;
pub(crate) const MAX_PUBLIC_PRICE_CATALOG_ROWS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriceResolutionContext {
    pub currency_code: String,
    pub region_id: Option<Uuid>,
    pub price_list_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub quantity: Option<i32>,
}

impl PriceResolutionContext {
    /// Syntactic/money normalization only. Identity and channel completion stay in owner ports.
    #[allow(clippy::result_large_err)]
    pub fn normalized(mut self) -> CommerceResult<Self> {
        self.currency_code = normalize_resolution_currency(&self.currency_code)?;
        self.quantity = Some(normalize_resolution_quantity(self.quantity)?);
        for (field, id) in [
            ("region_id", self.region_id),
            ("price_list_id", self.price_list_id),
            ("channel_id", self.channel_id),
        ] {
            if id.is_some_and(|id| id.is_nil()) {
                return Err(CommerceError::Validation(format!(
                    "{field} must not be a nil UUID"
                )));
            }
        }
        if self
            .channel_slug
            .as_deref()
            .is_some_and(|slug| slug.trim().is_empty())
        {
            return Err(CommerceError::Validation(
                "channel_slug must not be empty".into(),
            ));
        }
        self.channel_slug = normalize_channel_slug(self.channel_slug.as_deref());
        Ok(self)
    }
}

/// Provenance is an observation, not a price reservation or an optimistic write token.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PriceResolutionSource {
    Base {
        price_id: Uuid,
    },
    PriceListOverride {
        price_id: Uuid,
        price_list_id: Uuid,
    },
    PriceListRule {
        price_id: Uuid,
        price_list_id: Uuid,
        rule: PriceListRule,
    },
}

impl PriceResolutionSource {
    pub fn price_id(&self) -> Uuid {
        match self {
            Self::Base { price_id }
            | Self::PriceListOverride { price_id, .. }
            | Self::PriceListRule { price_id, .. } => *price_id,
        }
    }

    pub fn rule(&self) -> Option<&PriceListRule> {
        match self {
            Self::PriceListRule { rule, .. } => Some(rule),
            Self::Base { .. } | Self::PriceListOverride { .. } => None,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Base { .. } => "base",
            Self::PriceListOverride { .. } => "price_list_override",
            Self::PriceListRule { .. } => "price_list_rule",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedPrice {
    pub source: PriceResolutionSource,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivePriceListOption {
    pub id: Uuid,
    pub name: String,
    pub list_type: String,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub rule_kind: Option<String>,
    pub adjustment_percent: Option<Decimal>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PriceAdjustmentKind {
    PercentageDiscount,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceAdjustmentPreview {
    pub kind: PriceAdjustmentKind,
    pub currency_code: String,
    pub current_amount: Decimal,
    pub base_amount: Decimal,
    pub adjustment_percent: Decimal,
    pub adjusted_amount: Decimal,
    pub compare_at_amount: Option<Decimal>,
    pub price_list_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PriceListRuleKind {
    PercentageDiscount,
}

impl PriceListRuleKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PercentageDiscount => "percentage_discount",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriceListRule {
    pub kind: PriceListRuleKind,
    pub adjustment_percent: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminPricingProductList {
    pub items: Vec<AdminPricingProductListItem>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub has_next: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminPricingProductListItem {
    pub id: Uuid,
    pub status: ProductStatus,
    pub seller_id: Option<String>,
    pub title: String,
    pub handle: String,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub tags: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminPricingProductDetail {
    pub id: Uuid,
    pub status: ProductStatus,
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
    pub translations: Vec<AdminPricingProductTranslation>,
    pub variants: Vec<AdminPricingVariant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminPricingProductTranslation {
    pub locale: String,
    pub title: String,
    pub handle: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminPricingVariant {
    pub id: Uuid,
    pub sku: Option<String>,
    pub barcode: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub title: String,
    pub combination_identity: Option<String>,
    pub prices: Vec<AdminPricingPrice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminPricingPrice {
    pub currency_code: String,
    pub amount: Decimal,
    pub compare_at_amount: Option<Decimal>,
    pub discount_percent: Option<Decimal>,
    pub on_sale: bool,
    pub price_list_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub min_quantity: Option<i32>,
    pub max_quantity: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorefrontPricingProductList {
    pub items: Vec<StorefrontPricingProductListItem>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub has_next: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorefrontPricingProductListItem {
    pub id: Uuid,
    pub title: String,
    pub handle: String,
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
    pub variant_count: u64,
    pub sale_variant_count: u64,
    pub currencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorefrontPricingProductDetail {
    pub id: Uuid,
    pub status: ProductStatus,
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
    pub translations: Vec<StorefrontPricingProductTranslation>,
    pub variants: Vec<StorefrontPricingVariant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorefrontPricingProductTranslation {
    pub locale: String,
    pub title: String,
    pub handle: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorefrontPricingVariant {
    pub id: Uuid,
    pub title: String,
    pub sku: Option<String>,
    pub prices: Vec<StorefrontPricingPrice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorefrontPricingPrice {
    pub currency_code: String,
    pub amount: Decimal,
    pub compare_at_amount: Option<Decimal>,
    pub discount_percent: Option<Decimal>,
    pub on_sale: bool,
}

pub(crate) struct PriceResolutionBatch {
    pub context: PriceResolutionContext,
    pub resolved_at: chrono::DateTime<chrono::Utc>,
    pub prices: Vec<(Uuid, Option<ResolvedPrice>)>,
}

#[allow(clippy::result_large_err)]
pub(crate) fn normalize_resolution_currency(currency_code: &str) -> CommerceResult<String> {
    let normalized = money::normalize_currency_code(currency_code)
        .map_err(|_| CommerceError::Validation("currency_code must be a 3-letter code".into()))?;
    money::currency_exponent(&normalized).map_err(|_| {
        CommerceError::Validation("currency_code has no supported decimal minor unit".into())
    })?;
    Ok(normalized)
}

pub(crate) fn normalize_channel_slug(channel_slug: Option<&str>) -> Option<String> {
    channel_slug
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}

#[allow(clippy::result_large_err)]
pub(crate) fn normalize_resolution_quantity(quantity: Option<i32>) -> CommerceResult<i32> {
    match quantity {
        Some(value) if value < 1 => Err(CommerceError::Validation(
            "quantity must be at least 1".to_string(),
        )),
        Some(value) => Ok(value),
        None => Ok(1),
    }
}

pub(crate) fn is_sale_price(amount: Decimal, compare_at_amount: Option<Decimal>) -> bool {
    compare_at_amount
        .filter(|compare_at| *compare_at > Decimal::ZERO)
        .map(|compare_at| compare_at > amount)
        .unwrap_or(false)
}

pub(crate) fn calculate_discount_percent(
    amount: Decimal,
    compare_at_amount: Option<Decimal>,
) -> Option<Decimal> {
    let compare_at_amount = compare_at_amount.filter(|compare_at| *compare_at > Decimal::ZERO)?;
    if compare_at_amount <= amount {
        return None;
    }

    Some((((compare_at_amount - amount) / compare_at_amount) * Decimal::from(100)).round_dp(2))
}

#[allow(clippy::result_large_err)]
pub(crate) fn validate_price_precision(
    amount: Decimal,
    compare_at_amount: Option<Decimal>,
    currency_code: &str,
) -> CommerceResult<()> {
    for (label, value) in [
        ("Amount", Some(amount)),
        ("Compare at price", compare_at_amount),
    ] {
        if let Some(value) = value {
            money::to_minor_units_exact(value, currency_code).map_err(|error| {
                CommerceError::InvalidPrice(format!(
                    "{label} {value} is not representable in {currency_code} minor units: {error}"
                ))
            })?;
        }
    }
    Ok(())
}

#[allow(clippy::result_large_err)]
pub(crate) fn validate_price_tier_quantities(
    min_quantity: Option<i32>,
    max_quantity: Option<i32>,
) -> CommerceResult<()> {
    if let Some(min_quantity) = min_quantity
        && min_quantity <= 0
    {
        return Err(CommerceError::InvalidPrice(
            "Minimum quantity must be positive".into(),
        ));
    }

    if let Some(max_quantity) = max_quantity
        && max_quantity <= 0
    {
        return Err(CommerceError::InvalidPrice(
            "Maximum quantity must be positive".into(),
        ));
    }

    if let (Some(min_quantity), Some(max_quantity)) = (min_quantity, max_quantity)
        && max_quantity < min_quantity
    {
        return Err(CommerceError::InvalidPrice(
            "Maximum quantity must be greater than or equal to minimum quantity".into(),
        ));
    }

    Ok(())
}

#[allow(clippy::result_large_err)]
pub(crate) fn validate_discount_percent(discount_percent: Decimal) -> CommerceResult<()> {
    if discount_percent <= Decimal::ZERO || discount_percent > Decimal::from(100) {
        return Err(CommerceError::InvalidPrice(
            "discount_percent must be greater than 0 and at most 100".into(),
        ));
    }

    Ok(())
}
