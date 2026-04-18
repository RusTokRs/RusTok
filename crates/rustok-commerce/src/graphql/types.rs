use async_graphql::{Enum, InputObject, MaybeUndefined, SimpleObject};
use uuid::Uuid;

use crate::dto;

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum GqlProductStatus {
    Draft,
    Active,
    Archived,
}

impl From<crate::entities::product::ProductStatus> for GqlProductStatus {
    fn from(status: crate::entities::product::ProductStatus) -> Self {
        match status {
            crate::entities::product::ProductStatus::Draft => GqlProductStatus::Draft,
            crate::entities::product::ProductStatus::Active => GqlProductStatus::Active,
            crate::entities::product::ProductStatus::Archived => GqlProductStatus::Archived,
        }
    }
}

impl From<GqlProductStatus> for crate::entities::product::ProductStatus {
    fn from(status: GqlProductStatus) -> Self {
        match status {
            GqlProductStatus::Draft => crate::entities::product::ProductStatus::Draft,
            GqlProductStatus::Active => crate::entities::product::ProductStatus::Active,
            GqlProductStatus::Archived => crate::entities::product::ProductStatus::Archived,
        }
    }
}

/// Catalog-authoritative product detail.
///
/// For pricing-authoritative reads with explicit currency/region/price-list/channel
/// context, use `adminPricingProduct` or `storefrontPricingProduct`.
#[derive(SimpleObject)]
pub struct GqlProduct {
    pub id: Uuid,
    pub status: GqlProductStatus,
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub published_at: Option<String>,
    pub translations: Vec<GqlProductTranslation>,
    pub options: Vec<GqlProductOption>,
    pub variants: Vec<GqlVariant>,
}

#[derive(SimpleObject)]
pub struct GqlProductTranslation {
    pub locale: String,
    pub title: String,
    pub handle: String,
    pub description: Option<String>,
    pub meta_title: Option<String>,
    pub meta_description: Option<String>,
}

#[derive(SimpleObject)]
pub struct GqlProductOption {
    pub id: Uuid,
    pub name: String,
    pub values: Vec<String>,
    pub position: i32,
}

/// Catalog variant snapshot returned by the generic product roots.
#[derive(SimpleObject)]
pub struct GqlVariant {
    pub id: Uuid,
    pub sku: Option<String>,
    pub barcode: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub title: String,
    pub option1: Option<String>,
    pub option2: Option<String>,
    pub option3: Option<String>,
    /// Catalog-side compatibility price snapshot.
    ///
    /// This field is kept for catalog/product consumers and legacy fallbacks, but
    /// it is not the pricing-authoritative contract once `rustok-pricing` is present.
    #[graphql(
        deprecation = "Catalog compatibility snapshot only; use adminPricingProduct/storefrontPricingProduct or rustok-pricing module surfaces for pricing-authoritative reads."
    )]
    pub prices: Vec<GqlPrice>,
    pub inventory_quantity: i32,
    pub inventory_policy: String,
    pub in_stock: bool,
}

/// Catalog price snapshot without effective pricing context.
#[derive(SimpleObject)]
pub struct GqlPrice {
    pub currency_code: String,
    pub amount: String,
    pub compare_at_amount: Option<String>,
    pub on_sale: bool,
}

#[derive(SimpleObject)]
pub struct GqlProductList {
    pub items: Vec<GqlProductListItem>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub has_next: bool,
}

#[derive(SimpleObject)]
pub struct GqlProductListItem {
    pub id: Uuid,
    pub status: GqlProductStatus,
    pub title: String,
    pub handle: String,
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub published_at: Option<String>,
}

#[derive(SimpleObject)]
pub struct GqlAdminOrderDetail {
    pub order: GqlOrder,
    pub payment_collection: Option<GqlPaymentCollection>,
    pub fulfillment: Option<GqlFulfillment>,
}

#[derive(SimpleObject)]
pub struct GqlCustomer {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub phone: Option<String>,
    pub locale: Option<String>,
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(SimpleObject)]
pub struct GqlRegion {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub currency_code: String,
    pub tax_provider_id: Option<String>,
    pub tax_rate: String,
    pub tax_included: bool,
    pub country_tax_policies: Vec<GqlRegionCountryTaxPolicy>,
    pub countries: Vec<String>,
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
    pub requested_locale: Option<String>,
    pub effective_locale: Option<String>,
    pub available_locales: Vec<String>,
    pub translations: Vec<GqlRegionTranslation>,
}

#[derive(SimpleObject)]
pub struct GqlRegionTranslation {
    pub locale: String,
    pub name: String,
}

#[derive(SimpleObject)]
pub struct GqlRegionCountryTaxPolicy {
    pub country_code: String,
    pub tax_rate: String,
    pub tax_included: bool,
}

#[derive(SimpleObject)]
pub struct GqlShippingOption {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub currency_code: String,
    pub amount: String,
    pub provider_id: String,
    pub active: bool,
    pub allowed_shipping_profile_slugs: Option<Vec<String>>,
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
    pub requested_locale: Option<String>,
    pub effective_locale: Option<String>,
    pub available_locales: Vec<String>,
    pub translations: Vec<GqlShippingOptionTranslation>,
}

#[derive(SimpleObject)]
pub struct GqlShippingOptionTranslation {
    pub locale: String,
    pub name: String,
}

#[derive(SimpleObject)]
pub struct GqlShippingProfile {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub active: bool,
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
    pub requested_locale: Option<String>,
    pub effective_locale: Option<String>,
    pub available_locales: Vec<String>,
    pub translations: Vec<GqlShippingProfileTranslation>,
}

#[derive(SimpleObject)]
pub struct GqlShippingProfileTranslation {
    pub locale: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(SimpleObject)]
pub struct GqlShippingProfileList {
    pub items: Vec<GqlShippingProfile>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub has_next: bool,
}

#[derive(SimpleObject)]
pub struct GqlShippingOptionList {
    pub items: Vec<GqlShippingOption>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub has_next: bool,
}

#[derive(SimpleObject)]
pub struct GqlStoreContext {
    pub region: Option<GqlRegion>,
    pub locale: String,
    pub default_locale: String,
    pub available_locales: Vec<String>,
    pub currency_code: Option<String>,
}

#[derive(SimpleObject)]
pub struct GqlPricingChannelOption {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub is_active: bool,
    pub is_default: bool,
    pub status: String,
}

#[derive(SimpleObject)]
pub struct GqlActivePriceListOption {
    pub id: Uuid,
    pub name: String,
    pub list_type: String,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub rule_kind: Option<String>,
    pub adjustment_percent: Option<String>,
}

#[derive(SimpleObject)]
pub struct GqlPricingEffectivePrice {
    pub currency_code: String,
    pub amount: String,
    pub compare_at_amount: Option<String>,
    pub discount_percent: Option<String>,
    pub on_sale: bool,
    pub region_id: Option<Uuid>,
    pub price_list_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub min_quantity: Option<i32>,
    pub max_quantity: Option<i32>,
}

#[derive(SimpleObject)]
pub struct GqlPricingPrice {
    pub currency_code: String,
    pub amount: String,
    pub compare_at_amount: Option<String>,
    pub discount_percent: Option<String>,
    pub on_sale: bool,
    pub price_list_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub min_quantity: Option<i32>,
    pub max_quantity: Option<i32>,
}

#[derive(SimpleObject)]
pub struct GqlPricingVariant {
    pub id: Uuid,
    pub sku: Option<String>,
    pub barcode: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub title: String,
    pub option1: Option<String>,
    pub option2: Option<String>,
    pub option3: Option<String>,
    pub prices: Vec<GqlPricingPrice>,
    pub effective_price: Option<GqlPricingEffectivePrice>,
}

#[derive(SimpleObject)]
pub struct GqlPricingProductDetail {
    pub id: Uuid,
    pub status: String,
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub published_at: Option<String>,
    pub translations: Vec<GqlProductTranslation>,
    pub variants: Vec<GqlPricingVariant>,
}

#[derive(SimpleObject)]
pub struct GqlPricingAdjustmentPreview {
    pub kind: String,
    pub currency_code: String,
    pub current_amount: String,
    pub base_amount: String,
    pub adjustment_percent: String,
    pub adjusted_amount: String,
    pub compare_at_amount: Option<String>,
    pub price_list_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum GqlAdminCartPromotionKind {
    PercentageDiscount,
    FixedDiscount,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum GqlAdminCartPromotionScope {
    Cart,
    LineItem,
    Shipping,
}

#[derive(InputObject)]
pub struct AdminCartPromotionInput {
    pub kind: GqlAdminCartPromotionKind,
    pub scope: GqlAdminCartPromotionScope,
    pub line_item_id: Option<Uuid>,
    pub source_id: String,
    pub discount_percent: Option<String>,
    pub amount: Option<String>,
    pub metadata: Option<String>,
}

#[derive(SimpleObject)]
pub struct GqlCartPromotionPreview {
    pub kind: String,
    pub scope: String,
    pub line_item_id: Option<Uuid>,
    pub currency_code: String,
    pub base_amount: String,
    pub adjustment_amount: String,
    pub adjusted_amount: String,
}

#[derive(SimpleObject)]
pub struct GqlCart {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub customer_id: Option<Uuid>,
    pub email: Option<String>,
    pub region_id: Option<Uuid>,
    pub country_code: Option<String>,
    pub locale_code: Option<String>,
    pub selected_shipping_option_id: Option<Uuid>,
    pub status: String,
    pub currency_code: String,
    pub subtotal_amount: String,
    pub adjustment_total: String,
    pub shipping_total: String,
    pub total_amount: String,
    pub tax_total: String,
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
    pub line_items: Vec<GqlCartLineItem>,
    pub adjustments: Vec<GqlCartAdjustment>,
    pub tax_lines: Vec<GqlCartTaxLine>,
    pub delivery_groups: Vec<GqlCartDeliveryGroup>,
}

#[derive(SimpleObject)]
pub struct GqlCartLineItem {
    pub id: Uuid,
    pub cart_id: Uuid,
    pub product_id: Option<Uuid>,
    pub variant_id: Option<Uuid>,
    pub shipping_profile_slug: String,
    pub seller_id: Option<String>,
    pub seller_scope: Option<String>,
    pub sku: Option<String>,
    pub title: String,
    pub quantity: i32,
    pub unit_price: String,
    pub total_price: String,
    pub currency_code: String,
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(SimpleObject)]
pub struct GqlCartAdjustment {
    pub id: Uuid,
    pub cart_id: Uuid,
    pub line_item_id: Option<Uuid>,
    pub source_type: String,
    pub source_id: Option<String>,
    pub amount: String,
    pub currency_code: String,
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(SimpleObject)]
pub struct GqlCartTaxLine {
    pub id: Uuid,
    pub cart_id: Uuid,
    pub line_item_id: Option<Uuid>,
    pub shipping_option_id: Option<Uuid>,
    pub description: Option<String>,
    pub provider_id: String,
    pub rate: String,
    pub amount: String,
    pub currency_code: String,
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(SimpleObject)]
pub struct GqlCartDeliveryGroup {
    pub shipping_profile_slug: String,
    pub seller_id: Option<String>,
    pub seller_scope: Option<String>,
    pub line_item_ids: Vec<Uuid>,
    pub selected_shipping_option_id: Option<Uuid>,
    pub available_shipping_options: Vec<GqlCartShippingOptionSummary>,
}

#[derive(SimpleObject)]
pub struct GqlCartShippingOptionSummary {
    pub id: Uuid,
    pub name: String,
    pub currency_code: String,
    pub amount: String,
    pub provider_id: String,
    pub active: bool,
    pub metadata: String,
}

#[derive(SimpleObject)]
pub struct GqlCompleteCheckout {
    pub cart: GqlCart,
    pub order: GqlOrder,
    pub payment_collection: GqlPaymentCollection,
    pub fulfillment: Option<GqlFulfillment>,
    pub fulfillments: Vec<GqlFulfillment>,
    pub context: GqlStoreContext,
}

#[derive(SimpleObject)]
pub struct GqlStoreCart {
    pub cart: GqlCart,
    pub context: GqlStoreContext,
}

#[derive(SimpleObject)]
pub struct GqlOrder {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub customer_id: Option<Uuid>,
    pub status: String,
    pub currency_code: String,
    pub subtotal_amount: String,
    pub adjustment_total: String,
    pub shipping_total: String,
    pub total_amount: String,
    pub tax_total: String,
    pub tax_included: bool,
    pub metadata: String,
    pub payment_id: Option<String>,
    pub payment_method: Option<String>,
    pub tracking_number: Option<String>,
    pub carrier: Option<String>,
    pub cancellation_reason: Option<String>,
    pub delivered_signature: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub confirmed_at: Option<String>,
    pub paid_at: Option<String>,
    pub shipped_at: Option<String>,
    pub delivered_at: Option<String>,
    pub cancelled_at: Option<String>,
    pub line_items: Vec<GqlOrderLineItem>,
    pub adjustments: Vec<GqlOrderAdjustment>,
    pub tax_lines: Vec<GqlOrderTaxLine>,
}

#[derive(SimpleObject)]
pub struct GqlOrderLineItem {
    pub id: Uuid,
    pub order_id: Uuid,
    pub product_id: Option<Uuid>,
    pub variant_id: Option<Uuid>,
    pub shipping_profile_slug: String,
    pub seller_id: Option<String>,
    pub sku: Option<String>,
    pub title: String,
    pub quantity: i32,
    pub unit_price: String,
    pub total_price: String,
    pub currency_code: String,
    pub metadata: String,
    pub created_at: String,
}

#[derive(SimpleObject)]
pub struct GqlOrderAdjustment {
    pub id: Uuid,
    pub order_id: Uuid,
    pub line_item_id: Option<Uuid>,
    pub source_type: String,
    pub source_id: Option<String>,
    pub amount: String,
    pub currency_code: String,
    pub metadata: String,
    pub created_at: String,
}

#[derive(SimpleObject)]
pub struct GqlOrderTaxLine {
    pub id: Uuid,
    pub order_id: Uuid,
    pub line_item_id: Option<Uuid>,
    pub shipping_option_id: Option<Uuid>,
    pub description: Option<String>,
    pub provider_id: String,
    pub rate: String,
    pub amount: String,
    pub currency_code: String,
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(SimpleObject)]
pub struct GqlOrderList {
    pub items: Vec<GqlOrder>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub has_next: bool,
}

#[derive(SimpleObject)]
pub struct GqlPaymentCollection {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub cart_id: Option<Uuid>,
    pub order_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
    pub status: String,
    pub currency_code: String,
    pub amount: String,
    pub authorized_amount: String,
    pub captured_amount: String,
    pub refunded_amount: String,
    pub provider_id: Option<String>,
    pub cancellation_reason: Option<String>,
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
    pub authorized_at: Option<String>,
    pub captured_at: Option<String>,
    pub cancelled_at: Option<String>,
    pub payments: Vec<GqlPayment>,
    pub refunds: Vec<GqlRefund>,
}

#[derive(SimpleObject)]
pub struct GqlPayment {
    pub id: Uuid,
    pub payment_collection_id: Uuid,
    pub provider_id: String,
    pub provider_payment_id: String,
    pub status: String,
    pub currency_code: String,
    pub amount: String,
    pub captured_amount: String,
    pub error_message: Option<String>,
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
    pub authorized_at: Option<String>,
    pub captured_at: Option<String>,
    pub cancelled_at: Option<String>,
}

#[derive(SimpleObject)]
pub struct GqlPaymentCollectionList {
    pub items: Vec<GqlPaymentCollection>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub has_next: bool,
}

#[derive(SimpleObject)]
pub struct GqlRefund {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub payment_collection_id: Uuid,
    pub status: String,
    pub currency_code: String,
    pub amount: String,
    pub reason: Option<String>,
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
    pub refunded_at: Option<String>,
    pub cancelled_at: Option<String>,
}

#[derive(SimpleObject)]
pub struct GqlRefundList {
    pub items: Vec<GqlRefund>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub has_next: bool,
}

#[derive(SimpleObject)]
pub struct GqlFulfillment {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub order_id: Uuid,
    pub shipping_option_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
    pub status: String,
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    pub delivered_note: Option<String>,
    pub cancellation_reason: Option<String>,
    pub items: Vec<GqlFulfillmentItem>,
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
    pub shipped_at: Option<String>,
    pub delivered_at: Option<String>,
    pub cancelled_at: Option<String>,
}

#[derive(SimpleObject)]
pub struct GqlFulfillmentItem {
    pub id: Uuid,
    pub fulfillment_id: Uuid,
    pub order_line_item_id: Uuid,
    pub quantity: i32,
    pub shipped_quantity: i32,
    pub delivered_quantity: i32,
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(SimpleObject)]
pub struct GqlFulfillmentList {
    pub items: Vec<GqlFulfillment>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub has_next: bool,
}

#[derive(InputObject)]
pub struct CreateProductInput {
    pub translations: Vec<ProductTranslationInput>,
    pub options: Option<Vec<ProductOptionInput>>,
    pub variants: Vec<CreateVariantInput>,
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub tags: Option<Vec<String>>,
    pub publish: Option<bool>,
}

#[derive(InputObject)]
pub struct ProductTranslationInput {
    pub locale: String,
    pub title: String,
    pub handle: Option<String>,
    pub description: Option<String>,
    pub meta_title: Option<String>,
    pub meta_description: Option<String>,
}

#[derive(InputObject)]
pub struct ProductOptionInput {
    pub translations: Vec<ProductOptionTranslationInput>,
}

#[derive(InputObject)]
pub struct ProductOptionTranslationInput {
    pub locale: String,
    pub name: String,
    pub values: Vec<String>,
}

#[derive(InputObject)]
pub struct CreateVariantInput {
    pub sku: Option<String>,
    pub barcode: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub option1: Option<String>,
    pub option2: Option<String>,
    pub option3: Option<String>,
    pub prices: Vec<PriceInput>,
    pub inventory_quantity: Option<i32>,
    pub inventory_policy: Option<String>,
}

#[derive(InputObject)]
pub struct PriceInput {
    pub currency_code: String,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub amount: String,
    pub compare_at_amount: Option<String>,
}

#[derive(InputObject)]
pub struct UpdateProductInput {
    pub translations: Option<Vec<ProductTranslationInput>>,
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub tags: Option<Vec<String>>,
    pub status: Option<GqlProductStatus>,
}

#[derive(InputObject)]
pub struct ProductsFilter {
    pub status: Option<GqlProductStatus>,
    pub vendor: Option<String>,
    pub search: Option<String>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

#[derive(InputObject)]
pub struct StorefrontProductsFilter {
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub search: Option<String>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

#[derive(InputObject)]
pub struct StorefrontContextFilter {
    pub cart_id: Option<Uuid>,
    pub region_id: Option<Uuid>,
    pub country_code: Option<String>,
    pub locale: Option<String>,
    pub currency_code: Option<String>,
}

#[derive(InputObject)]
pub struct OrdersFilter {
    pub status: Option<String>,
    pub customer_id: Option<Uuid>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

#[derive(InputObject)]
pub struct PaymentCollectionsFilter {
    pub status: Option<String>,
    pub order_id: Option<Uuid>,
    pub cart_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

#[derive(InputObject)]
pub struct RefundsFilter {
    pub payment_collection_id: Option<Uuid>,
    pub status: Option<String>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

#[derive(InputObject)]
pub struct FulfillmentsFilter {
    pub status: Option<String>,
    pub order_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

#[derive(InputObject)]
pub struct ShippingOptionsFilter {
    pub active: Option<bool>,
    pub currency_code: Option<String>,
    pub provider_id: Option<String>,
    pub search: Option<String>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

#[derive(InputObject)]
pub struct ShippingProfilesFilter {
    pub active: Option<bool>,
    pub search: Option<String>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

#[derive(InputObject)]
pub struct MarkPaidOrderInput {
    pub payment_id: String,
    pub payment_method: String,
}

#[derive(InputObject)]
pub struct ShipOrderInput {
    pub tracking_number: String,
    pub carrier: String,
}

#[derive(InputObject)]
pub struct DeliverOrderInput {
    pub delivered_signature: Option<String>,
}

#[derive(InputObject)]
pub struct CancelOrderInput {
    pub reason: Option<String>,
}

#[derive(InputObject)]
pub struct AuthorizePaymentCollectionInput {
    pub provider_id: Option<String>,
    pub provider_payment_id: Option<String>,
    pub amount: Option<String>,
    pub metadata: Option<String>,
}

#[derive(InputObject)]
pub struct CapturePaymentCollectionInput {
    pub amount: Option<String>,
    pub metadata: Option<String>,
}

#[derive(InputObject)]
pub struct CancelPaymentCollectionInput {
    pub reason: Option<String>,
    pub metadata: Option<String>,
}

#[derive(InputObject)]
pub struct CreateRefundInputObject {
    pub amount: String,
    pub reason: Option<String>,
    pub metadata: Option<String>,
}

#[derive(InputObject)]
pub struct CompleteRefundInputObject {
    pub metadata: Option<String>,
}

#[derive(InputObject)]
pub struct CancelRefundInputObject {
    pub reason: Option<String>,
    pub metadata: Option<String>,
}

#[derive(InputObject)]
pub struct CreateStorefrontPaymentCollectionInput {
    pub cart_id: Uuid,
    pub metadata: Option<String>,
}

#[derive(InputObject)]
pub struct CompleteStorefrontCheckoutInput {
    pub cart_id: Uuid,
    pub shipping_option_id: Option<Uuid>,
    pub shipping_selections: Option<Vec<StorefrontShippingSelectionInput>>,
    pub region_id: Option<Uuid>,
    pub country_code: Option<String>,
    pub locale: Option<String>,
    pub create_fulfillment: Option<bool>,
    pub metadata: Option<String>,
}

#[derive(InputObject)]
pub struct UpdateAdminPricingVariantPriceInput {
    pub currency_code: String,
    pub amount: String,
    pub compare_at_amount: Option<String>,
    pub price_list_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub min_quantity: Option<i32>,
    pub max_quantity: Option<i32>,
}

#[derive(InputObject)]
pub struct AdminPricingVariantDiscountInput {
    pub currency_code: String,
    pub discount_percent: String,
    pub price_list_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
}

#[derive(InputObject)]
pub struct UpdateAdminPricingPriceListRuleInput {
    pub adjustment_percent: Option<String>,
}

#[derive(InputObject)]
pub struct UpdateAdminPricingPriceListScopeInput {
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
}

#[derive(InputObject)]
pub struct CreateStorefrontCartInput {
    pub email: Option<String>,
    pub currency_code: Option<String>,
    pub region_id: Option<Uuid>,
    pub country_code: Option<String>,
    pub locale: Option<String>,
    pub metadata: Option<String>,
}

#[derive(InputObject)]
pub struct AddStorefrontCartLineItemInput {
    pub variant_id: Uuid,
    pub quantity: i32,
    pub metadata: Option<String>,
}

#[derive(InputObject)]
pub struct UpdateStorefrontCartLineItemInput {
    pub quantity: i32,
}

#[derive(InputObject)]
pub struct UpdateStorefrontCartContextInput {
    pub email: MaybeUndefined<String>,
    pub region_id: MaybeUndefined<Uuid>,
    pub country_code: MaybeUndefined<String>,
    pub locale: MaybeUndefined<String>,
    pub selected_shipping_option_id: MaybeUndefined<Uuid>,
    pub shipping_selections: MaybeUndefined<Vec<StorefrontShippingSelectionInput>>,
}

#[derive(InputObject)]
pub struct StorefrontShippingSelectionInput {
    pub shipping_profile_slug: String,
    pub seller_id: Option<String>,
    pub seller_scope: Option<String>,
    pub selected_shipping_option_id: Option<Uuid>,
}

#[derive(InputObject)]
#[graphql(name = "CreateFulfillmentItemInput")]
pub struct CreateFulfillmentItemInputObject {
    pub order_line_item_id: Uuid,
    pub quantity: i32,
    pub metadata: Option<String>,
}

#[derive(InputObject)]
#[graphql(name = "CreateFulfillmentInput")]
pub struct CreateFulfillmentInputObject {
    pub order_id: Uuid,
    pub shipping_option_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    pub items: Vec<CreateFulfillmentItemInputObject>,
    pub metadata: Option<String>,
}

#[derive(InputObject)]
#[graphql(name = "FulfillmentItemQuantityInput")]
pub struct FulfillmentItemQuantityInputObject {
    pub fulfillment_item_id: Uuid,
    pub quantity: i32,
}

#[derive(InputObject)]
#[graphql(name = "ShipFulfillmentInput")]
pub struct ShipFulfillmentInputObject {
    pub carrier: String,
    pub tracking_number: String,
    pub items: Option<Vec<FulfillmentItemQuantityInputObject>>,
    pub metadata: Option<String>,
}

#[derive(InputObject)]
#[graphql(name = "DeliverFulfillmentInput")]
pub struct DeliverFulfillmentInputObject {
    pub delivered_note: Option<String>,
    pub items: Option<Vec<FulfillmentItemQuantityInputObject>>,
    pub metadata: Option<String>,
}

#[derive(InputObject)]
#[graphql(name = "ReopenFulfillmentInput")]
pub struct ReopenFulfillmentInputObject {
    pub items: Option<Vec<FulfillmentItemQuantityInputObject>>,
    pub metadata: Option<String>,
}

#[derive(InputObject)]
#[graphql(name = "ReshipFulfillmentInput")]
pub struct ReshipFulfillmentInputObject {
    pub carrier: String,
    pub tracking_number: String,
    pub items: Option<Vec<FulfillmentItemQuantityInputObject>>,
    pub metadata: Option<String>,
}

#[derive(InputObject)]
#[graphql(name = "CancelFulfillmentInput")]
pub struct CancelFulfillmentInputObject {
    pub reason: Option<String>,
    pub metadata: Option<String>,
}

#[derive(InputObject)]
#[graphql(name = "ShippingOptionTranslationInput")]
pub struct ShippingOptionTranslationInput {
    pub locale: String,
    pub name: String,
}

#[derive(InputObject)]
#[graphql(name = "CreateShippingOptionInput")]
pub struct CreateShippingOptionInputObject {
    pub translations: Vec<ShippingOptionTranslationInput>,
    pub currency_code: String,
    pub amount: String,
    pub provider_id: Option<String>,
    pub allowed_shipping_profile_slugs: Option<Vec<String>>,
    pub metadata: Option<String>,
}

#[derive(InputObject)]
#[graphql(name = "UpdateShippingOptionInput")]
pub struct UpdateShippingOptionInputObject {
    pub translations: Option<Vec<ShippingOptionTranslationInput>>,
    pub currency_code: Option<String>,
    pub amount: Option<String>,
    pub provider_id: Option<String>,
    pub allowed_shipping_profile_slugs: Option<Vec<String>>,
    pub metadata: Option<String>,
}

#[derive(InputObject)]
#[graphql(name = "ShippingProfileTranslationInput")]
pub struct ShippingProfileTranslationInput {
    pub locale: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(InputObject)]
#[graphql(name = "CreateShippingProfileInput")]
pub struct CreateShippingProfileInputObject {
    pub slug: String,
    pub translations: Vec<ShippingProfileTranslationInput>,
    pub metadata: Option<String>,
}

#[derive(InputObject)]
#[graphql(name = "UpdateShippingProfileInput")]
pub struct UpdateShippingProfileInputObject {
    pub slug: Option<String>,
    pub translations: Option<Vec<ShippingProfileTranslationInput>>,
    pub metadata: Option<String>,
}

impl From<dto::ProductResponse> for GqlProduct {
    fn from(product: dto::ProductResponse) -> Self {
        Self {
            id: product.id,
            status: product.status.into(),
            seller_id: product.seller_id,
            vendor: product.vendor,
            product_type: product.product_type,
            shipping_profile_slug: product.shipping_profile_slug,
            tags: product.tags,
            created_at: product.created_at.to_rfc3339(),
            updated_at: product.updated_at.to_rfc3339(),
            published_at: product.published_at.map(|value| value.to_rfc3339()),
            translations: product
                .translations
                .into_iter()
                .map(GqlProductTranslation::from)
                .collect(),
            options: product
                .options
                .into_iter()
                .map(GqlProductOption::from)
                .collect(),
            variants: product.variants.into_iter().map(GqlVariant::from).collect(),
        }
    }
}

impl From<dto::ProductTranslationResponse> for GqlProductTranslation {
    fn from(translation: dto::ProductTranslationResponse) -> Self {
        Self {
            locale: translation.locale,
            title: translation.title,
            handle: translation.handle,
            description: translation.description,
            meta_title: translation.meta_title,
            meta_description: translation.meta_description,
        }
    }
}

impl From<dto::ProductOptionResponse> for GqlProductOption {
    fn from(option: dto::ProductOptionResponse) -> Self {
        Self {
            id: option.id,
            name: option.name,
            values: option.values,
            position: option.position,
        }
    }
}

impl From<dto::VariantResponse> for GqlVariant {
    fn from(variant: dto::VariantResponse) -> Self {
        Self {
            id: variant.id,
            sku: variant.sku,
            barcode: variant.barcode,
            shipping_profile_slug: variant.shipping_profile_slug,
            title: variant.title,
            option1: variant.option1,
            option2: variant.option2,
            option3: variant.option3,
            prices: variant.prices.into_iter().map(GqlPrice::from).collect(),
            inventory_quantity: variant.inventory_quantity,
            inventory_policy: variant.inventory_policy,
            in_stock: variant.in_stock,
        }
    }
}

impl From<dto::PriceResponse> for GqlPrice {
    fn from(price: dto::PriceResponse) -> Self {
        Self {
            currency_code: price.currency_code,
            amount: price.amount.to_string(),
            compare_at_amount: price.compare_at_amount.map(|value| value.to_string()),
            on_sale: price.on_sale,
        }
    }
}

impl From<crate::controllers::admin::AdminOrderDetailResponse> for GqlAdminOrderDetail {
    fn from(value: crate::controllers::admin::AdminOrderDetailResponse) -> Self {
        Self {
            order: value.order.into(),
            payment_collection: value.payment_collection.map(Into::into),
            fulfillment: value.fulfillment.map(Into::into),
        }
    }
}

impl From<dto::CustomerResponse> for GqlCustomer {
    fn from(value: dto::CustomerResponse) -> Self {
        Self {
            id: value.id,
            tenant_id: value.tenant_id,
            user_id: value.user_id,
            email: value.email,
            first_name: value.first_name,
            last_name: value.last_name,
            phone: value.phone,
            locale: value.locale,
            metadata: value.metadata.to_string(),
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
        }
    }
}

impl From<dto::RegionResponse> for GqlRegion {
    fn from(value: dto::RegionResponse) -> Self {
        Self {
            id: value.id,
            tenant_id: value.tenant_id,
            name: value.name,
            currency_code: value.currency_code,
            tax_provider_id: value.tax_provider_id,
            tax_rate: value.tax_rate.to_string(),
            tax_included: value.tax_included,
            country_tax_policies: value
                .country_tax_policies
                .into_iter()
                .map(|policy| GqlRegionCountryTaxPolicy {
                    country_code: policy.country_code,
                    tax_rate: policy.tax_rate.to_string(),
                    tax_included: policy.tax_included,
                })
                .collect(),
            countries: value.countries,
            metadata: value.metadata.to_string(),
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
            requested_locale: value.requested_locale,
            effective_locale: value.effective_locale,
            available_locales: value.available_locales,
            translations: value
                .translations
                .into_iter()
                .map(|translation| GqlRegionTranslation {
                    locale: translation.locale,
                    name: translation.name,
                })
                .collect(),
        }
    }
}

impl From<dto::ShippingOptionResponse> for GqlShippingOption {
    fn from(value: dto::ShippingOptionResponse) -> Self {
        Self {
            id: value.id,
            tenant_id: value.tenant_id,
            name: value.name,
            currency_code: value.currency_code,
            amount: value.amount.to_string(),
            provider_id: value.provider_id,
            active: value.active,
            allowed_shipping_profile_slugs: value.allowed_shipping_profile_slugs,
            metadata: value.metadata.to_string(),
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
            requested_locale: value.requested_locale,
            effective_locale: value.effective_locale,
            available_locales: value.available_locales,
            translations: value
                .translations
                .into_iter()
                .map(|translation| GqlShippingOptionTranslation {
                    locale: translation.locale,
                    name: translation.name,
                })
                .collect(),
        }
    }
}

impl From<dto::ShippingProfileResponse> for GqlShippingProfile {
    fn from(value: dto::ShippingProfileResponse) -> Self {
        Self {
            id: value.id,
            tenant_id: value.tenant_id,
            slug: value.slug,
            name: value.name,
            description: value.description,
            active: value.active,
            metadata: value.metadata.to_string(),
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
            requested_locale: value.requested_locale,
            effective_locale: value.effective_locale,
            available_locales: value.available_locales,
            translations: value
                .translations
                .into_iter()
                .map(|translation| GqlShippingProfileTranslation {
                    locale: translation.locale,
                    name: translation.name,
                    description: translation.description,
                })
                .collect(),
        }
    }
}

impl From<dto::StoreContextResponse> for GqlStoreContext {
    fn from(value: dto::StoreContextResponse) -> Self {
        Self {
            region: value.region.map(Into::into),
            locale: value.locale,
            default_locale: value.default_locale,
            available_locales: value.available_locales,
            currency_code: value.currency_code,
        }
    }
}

impl From<rustok_channel::ChannelResponse> for GqlPricingChannelOption {
    fn from(value: rustok_channel::ChannelResponse) -> Self {
        Self {
            id: value.id,
            slug: value.slug,
            name: value.name,
            is_active: value.is_active,
            is_default: value.is_default,
            status: value.status,
        }
    }
}

impl From<rustok_pricing::ActivePriceListOption> for GqlActivePriceListOption {
    fn from(value: rustok_pricing::ActivePriceListOption) -> Self {
        Self {
            id: value.id,
            name: value.name,
            list_type: value.list_type,
            channel_id: value.channel_id,
            channel_slug: value.channel_slug,
            rule_kind: value.rule_kind,
            adjustment_percent: value
                .adjustment_percent
                .map(|item| item.normalize().to_string()),
        }
    }
}

impl From<rustok_pricing::ResolvedPrice> for GqlPricingEffectivePrice {
    fn from(value: rustok_pricing::ResolvedPrice) -> Self {
        Self {
            currency_code: value.currency_code,
            amount: value.amount.normalize().to_string(),
            compare_at_amount: value
                .compare_at_amount
                .map(|item| item.normalize().to_string()),
            discount_percent: value
                .discount_percent
                .map(|item| item.normalize().to_string()),
            on_sale: value.on_sale,
            region_id: value.region_id,
            price_list_id: value.price_list_id,
            channel_id: value.channel_id,
            channel_slug: value.channel_slug,
            min_quantity: value.min_quantity,
            max_quantity: value.max_quantity,
        }
    }
}

impl From<rustok_pricing::AdminPricingPrice> for GqlPricingPrice {
    fn from(value: rustok_pricing::AdminPricingPrice) -> Self {
        Self {
            currency_code: value.currency_code,
            amount: value.amount.normalize().to_string(),
            compare_at_amount: value
                .compare_at_amount
                .map(|item| item.normalize().to_string()),
            discount_percent: value
                .discount_percent
                .map(|item| item.normalize().to_string()),
            on_sale: value.on_sale,
            price_list_id: value.price_list_id,
            channel_id: value.channel_id,
            channel_slug: value.channel_slug,
            min_quantity: value.min_quantity,
            max_quantity: value.max_quantity,
        }
    }
}

impl From<rustok_pricing::StorefrontPricingPrice> for GqlPricingPrice {
    fn from(value: rustok_pricing::StorefrontPricingPrice) -> Self {
        Self {
            currency_code: value.currency_code,
            amount: value.amount.normalize().to_string(),
            compare_at_amount: value
                .compare_at_amount
                .map(|item| item.normalize().to_string()),
            discount_percent: value
                .discount_percent
                .map(|item| item.normalize().to_string()),
            on_sale: value.on_sale,
            price_list_id: None,
            channel_id: None,
            channel_slug: None,
            min_quantity: None,
            max_quantity: None,
        }
    }
}

impl From<rustok_pricing::AdminPricingVariant> for GqlPricingVariant {
    fn from(value: rustok_pricing::AdminPricingVariant) -> Self {
        Self {
            id: value.id,
            sku: value.sku,
            barcode: value.barcode,
            shipping_profile_slug: value.shipping_profile_slug,
            title: value.title,
            option1: value.option1,
            option2: value.option2,
            option3: value.option3,
            prices: value.prices.into_iter().map(Into::into).collect(),
            effective_price: None,
        }
    }
}

impl From<rustok_pricing::StorefrontPricingVariant> for GqlPricingVariant {
    fn from(value: rustok_pricing::StorefrontPricingVariant) -> Self {
        Self {
            id: value.id,
            sku: value.sku,
            barcode: None,
            shipping_profile_slug: None,
            title: value.title,
            option1: None,
            option2: None,
            option3: None,
            prices: value.prices.into_iter().map(Into::into).collect(),
            effective_price: None,
        }
    }
}

impl From<rustok_pricing::AdminPricingProductTranslation> for GqlProductTranslation {
    fn from(value: rustok_pricing::AdminPricingProductTranslation) -> Self {
        Self {
            locale: value.locale,
            title: value.title,
            handle: value.handle,
            description: value.description,
            meta_title: None,
            meta_description: None,
        }
    }
}

impl From<rustok_pricing::StorefrontPricingProductTranslation> for GqlProductTranslation {
    fn from(value: rustok_pricing::StorefrontPricingProductTranslation) -> Self {
        Self {
            locale: value.locale,
            title: value.title,
            handle: value.handle,
            description: value.description,
            meta_title: None,
            meta_description: None,
        }
    }
}

impl From<rustok_pricing::AdminPricingProductDetail> for GqlPricingProductDetail {
    fn from(value: rustok_pricing::AdminPricingProductDetail) -> Self {
        Self {
            id: value.id,
            status: value.status.to_string(),
            seller_id: value.seller_id,
            vendor: value.vendor,
            product_type: value.product_type,
            shipping_profile_slug: value.shipping_profile_slug,
            created_at: Some(value.created_at.to_rfc3339()),
            updated_at: Some(value.updated_at.to_rfc3339()),
            published_at: value.published_at.map(|item| item.to_rfc3339()),
            translations: value.translations.into_iter().map(Into::into).collect(),
            variants: value.variants.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<rustok_pricing::StorefrontPricingProductDetail> for GqlPricingProductDetail {
    fn from(value: rustok_pricing::StorefrontPricingProductDetail) -> Self {
        Self {
            id: value.id,
            status: value.status.to_string(),
            seller_id: value.seller_id,
            vendor: value.vendor,
            product_type: value.product_type,
            shipping_profile_slug: None,
            created_at: None,
            updated_at: None,
            published_at: value.published_at.map(|item| item.to_rfc3339()),
            translations: value.translations.into_iter().map(Into::into).collect(),
            variants: value.variants.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<rustok_pricing::PriceAdjustmentPreview> for GqlPricingAdjustmentPreview {
    fn from(value: rustok_pricing::PriceAdjustmentPreview) -> Self {
        Self {
            kind: match value.kind {
                rustok_pricing::PriceAdjustmentKind::PercentageDiscount => {
                    "percentage_discount".to_string()
                }
            },
            currency_code: value.currency_code,
            current_amount: value.current_amount.normalize().to_string(),
            base_amount: value.base_amount.normalize().to_string(),
            adjustment_percent: value.adjustment_percent.normalize().to_string(),
            adjusted_amount: value.adjusted_amount.normalize().to_string(),
            compare_at_amount: value
                .compare_at_amount
                .map(|item| item.normalize().to_string()),
            price_list_id: value.price_list_id,
            channel_id: value.channel_id,
            channel_slug: value.channel_slug,
        }
    }
}

impl From<dto::CartResponse> for GqlCart {
    fn from(value: dto::CartResponse) -> Self {
        Self {
            id: value.id,
            tenant_id: value.tenant_id,
            channel_id: value.channel_id,
            channel_slug: value.channel_slug,
            customer_id: value.customer_id,
            email: value.email,
            region_id: value.region_id,
            country_code: value.country_code,
            locale_code: value.locale_code,
            selected_shipping_option_id: value.selected_shipping_option_id,
            status: value.status,
            currency_code: value.currency_code,
            subtotal_amount: value.subtotal_amount.to_string(),
            adjustment_total: value.adjustment_total.to_string(),
            shipping_total: value.shipping_total.to_string(),
            total_amount: value.total_amount.to_string(),
            tax_total: value.tax_total.to_string(),
            metadata: value.metadata.to_string(),
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
            completed_at: value.completed_at.map(|value| value.to_rfc3339()),
            line_items: value.line_items.into_iter().map(Into::into).collect(),
            adjustments: value.adjustments.into_iter().map(Into::into).collect(),
            tax_lines: value.tax_lines.into_iter().map(Into::into).collect(),
            delivery_groups: value.delivery_groups.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<dto::CartLineItemResponse> for GqlCartLineItem {
    fn from(value: dto::CartLineItemResponse) -> Self {
        Self {
            id: value.id,
            cart_id: value.cart_id,
            product_id: value.product_id,
            variant_id: value.variant_id,
            shipping_profile_slug: value.shipping_profile_slug,
            seller_id: value.seller_id,
            seller_scope: value.seller_scope,
            sku: value.sku,
            title: value.title,
            quantity: value.quantity,
            unit_price: value.unit_price.to_string(),
            total_price: value.total_price.to_string(),
            currency_code: value.currency_code,
            metadata: value.metadata.to_string(),
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
        }
    }
}

impl From<dto::CartAdjustmentResponse> for GqlCartAdjustment {
    fn from(value: dto::CartAdjustmentResponse) -> Self {
        Self {
            id: value.id,
            cart_id: value.cart_id,
            line_item_id: value.line_item_id,
            source_type: value.source_type,
            source_id: value.source_id,
            amount: value.amount.to_string(),
            currency_code: value.currency_code,
            metadata: value.metadata.to_string(),
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
        }
    }
}

impl From<dto::CartTaxLineResponse> for GqlCartTaxLine {
    fn from(value: dto::CartTaxLineResponse) -> Self {
        Self {
            id: value.id,
            cart_id: value.cart_id,
            line_item_id: value.line_item_id,
            shipping_option_id: value.shipping_option_id,
            description: value.description,
            provider_id: value.provider_id,
            rate: value.rate.to_string(),
            amount: value.amount.to_string(),
            currency_code: value.currency_code,
            metadata: value.metadata.to_string(),
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
        }
    }
}

impl From<dto::CartDeliveryGroupResponse> for GqlCartDeliveryGroup {
    fn from(value: dto::CartDeliveryGroupResponse) -> Self {
        Self {
            shipping_profile_slug: value.shipping_profile_slug,
            seller_id: value.seller_id,
            seller_scope: value.seller_scope,
            line_item_ids: value.line_item_ids,
            selected_shipping_option_id: value.selected_shipping_option_id,
            available_shipping_options: value
                .available_shipping_options
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

impl From<dto::CartShippingOptionSummary> for GqlCartShippingOptionSummary {
    fn from(value: dto::CartShippingOptionSummary) -> Self {
        Self {
            id: value.id,
            name: value.name,
            currency_code: value.currency_code,
            amount: value.amount.to_string(),
            provider_id: value.provider_id,
            active: value.active,
            metadata: value.metadata.to_string(),
        }
    }
}

impl From<dto::CompleteCheckoutResponse> for GqlCompleteCheckout {
    fn from(value: dto::CompleteCheckoutResponse) -> Self {
        Self {
            cart: value.cart.into(),
            order: value.order.into(),
            payment_collection: value.payment_collection.into(),
            fulfillment: value.fulfillment.map(Into::into),
            fulfillments: value.fulfillments.into_iter().map(Into::into).collect(),
            context: value.context.into(),
        }
    }
}

impl From<crate::controllers::store::StoreCartResponse> for GqlStoreCart {
    fn from(value: crate::controllers::store::StoreCartResponse) -> Self {
        Self {
            cart: value.cart.into(),
            context: value.context.into(),
        }
    }
}

impl From<dto::OrderResponse> for GqlOrder {
    fn from(order: dto::OrderResponse) -> Self {
        Self {
            id: order.id,
            tenant_id: order.tenant_id,
            channel_id: order.channel_id,
            channel_slug: order.channel_slug,
            customer_id: order.customer_id,
            status: order.status,
            currency_code: order.currency_code,
            subtotal_amount: order.subtotal_amount.to_string(),
            adjustment_total: order.adjustment_total.to_string(),
            shipping_total: order.shipping_total.to_string(),
            total_amount: order.total_amount.to_string(),
            tax_total: order.tax_total.to_string(),
            tax_included: order.tax_included,
            metadata: order.metadata.to_string(),
            payment_id: order.payment_id,
            payment_method: order.payment_method,
            tracking_number: order.tracking_number,
            carrier: order.carrier,
            cancellation_reason: order.cancellation_reason,
            delivered_signature: order.delivered_signature,
            created_at: order.created_at.to_rfc3339(),
            updated_at: order.updated_at.to_rfc3339(),
            confirmed_at: order.confirmed_at.map(|value| value.to_rfc3339()),
            paid_at: order.paid_at.map(|value| value.to_rfc3339()),
            shipped_at: order.shipped_at.map(|value| value.to_rfc3339()),
            delivered_at: order.delivered_at.map(|value| value.to_rfc3339()),
            cancelled_at: order.cancelled_at.map(|value| value.to_rfc3339()),
            line_items: order.line_items.into_iter().map(Into::into).collect(),
            adjustments: order.adjustments.into_iter().map(Into::into).collect(),
            tax_lines: order.tax_lines.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<dto::OrderLineItemResponse> for GqlOrderLineItem {
    fn from(item: dto::OrderLineItemResponse) -> Self {
        Self {
            id: item.id,
            order_id: item.order_id,
            product_id: item.product_id,
            variant_id: item.variant_id,
            shipping_profile_slug: item.shipping_profile_slug,
            seller_id: item.seller_id,
            sku: item.sku,
            title: item.title,
            quantity: item.quantity,
            unit_price: item.unit_price.to_string(),
            total_price: item.total_price.to_string(),
            currency_code: item.currency_code,
            metadata: item.metadata.to_string(),
            created_at: item.created_at.to_rfc3339(),
        }
    }
}

impl From<dto::OrderAdjustmentResponse> for GqlOrderAdjustment {
    fn from(item: dto::OrderAdjustmentResponse) -> Self {
        Self {
            id: item.id,
            order_id: item.order_id,
            line_item_id: item.line_item_id,
            source_type: item.source_type,
            source_id: item.source_id,
            amount: item.amount.to_string(),
            currency_code: item.currency_code,
            metadata: item.metadata.to_string(),
            created_at: item.created_at.to_rfc3339(),
        }
    }
}

impl From<dto::OrderTaxLineResponse> for GqlOrderTaxLine {
    fn from(item: dto::OrderTaxLineResponse) -> Self {
        Self {
            id: item.id,
            order_id: item.order_id,
            line_item_id: item.line_item_id,
            shipping_option_id: item.shipping_option_id,
            description: item.description,
            provider_id: item.provider_id,
            rate: item.rate.to_string(),
            amount: item.amount.to_string(),
            currency_code: item.currency_code,
            metadata: item.metadata.to_string(),
            created_at: item.created_at.to_rfc3339(),
            updated_at: item.updated_at.to_rfc3339(),
        }
    }
}

impl From<dto::PaymentCollectionResponse> for GqlPaymentCollection {
    fn from(value: dto::PaymentCollectionResponse) -> Self {
        Self {
            id: value.id,
            tenant_id: value.tenant_id,
            cart_id: value.cart_id,
            order_id: value.order_id,
            customer_id: value.customer_id,
            status: value.status,
            currency_code: value.currency_code,
            amount: value.amount.to_string(),
            authorized_amount: value.authorized_amount.to_string(),
            captured_amount: value.captured_amount.to_string(),
            refunded_amount: value.refunded_amount.to_string(),
            provider_id: value.provider_id,
            cancellation_reason: value.cancellation_reason,
            metadata: value.metadata.to_string(),
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
            authorized_at: value.authorized_at.map(|value| value.to_rfc3339()),
            captured_at: value.captured_at.map(|value| value.to_rfc3339()),
            cancelled_at: value.cancelled_at.map(|value| value.to_rfc3339()),
            payments: value.payments.into_iter().map(Into::into).collect(),
            refunds: value.refunds.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<dto::PaymentResponse> for GqlPayment {
    fn from(value: dto::PaymentResponse) -> Self {
        Self {
            id: value.id,
            payment_collection_id: value.payment_collection_id,
            provider_id: value.provider_id,
            provider_payment_id: value.provider_payment_id,
            status: value.status,
            currency_code: value.currency_code,
            amount: value.amount.to_string(),
            captured_amount: value.captured_amount.to_string(),
            error_message: value.error_message,
            metadata: value.metadata.to_string(),
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
            authorized_at: value.authorized_at.map(|value| value.to_rfc3339()),
            captured_at: value.captured_at.map(|value| value.to_rfc3339()),
            cancelled_at: value.cancelled_at.map(|value| value.to_rfc3339()),
        }
    }
}

impl From<dto::RefundResponse> for GqlRefund {
    fn from(value: dto::RefundResponse) -> Self {
        Self {
            id: value.id,
            tenant_id: value.tenant_id,
            payment_collection_id: value.payment_collection_id,
            status: value.status,
            currency_code: value.currency_code,
            amount: value.amount.to_string(),
            reason: value.reason,
            metadata: value.metadata.to_string(),
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
            refunded_at: value.refunded_at.map(|value| value.to_rfc3339()),
            cancelled_at: value.cancelled_at.map(|value| value.to_rfc3339()),
        }
    }
}

impl From<dto::FulfillmentResponse> for GqlFulfillment {
    fn from(value: dto::FulfillmentResponse) -> Self {
        Self {
            id: value.id,
            tenant_id: value.tenant_id,
            order_id: value.order_id,
            shipping_option_id: value.shipping_option_id,
            customer_id: value.customer_id,
            status: value.status,
            carrier: value.carrier,
            tracking_number: value.tracking_number,
            delivered_note: value.delivered_note,
            cancellation_reason: value.cancellation_reason,
            items: value.items.into_iter().map(Into::into).collect(),
            metadata: value.metadata.to_string(),
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
            shipped_at: value.shipped_at.map(|value| value.to_rfc3339()),
            delivered_at: value.delivered_at.map(|value| value.to_rfc3339()),
            cancelled_at: value.cancelled_at.map(|value| value.to_rfc3339()),
        }
    }
}

impl From<dto::FulfillmentItemResponse> for GqlFulfillmentItem {
    fn from(value: dto::FulfillmentItemResponse) -> Self {
        Self {
            id: value.id,
            fulfillment_id: value.fulfillment_id,
            order_line_item_id: value.order_line_item_id,
            quantity: value.quantity,
            shipped_quantity: value.shipped_quantity,
            delivered_quantity: value.delivered_quantity,
            metadata: value.metadata.to_string(),
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
        }
    }
}
