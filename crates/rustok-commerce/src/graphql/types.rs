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

#[derive(SimpleObject)]
pub struct GqlProduct {
    pub id: Uuid,
    pub status: GqlProductStatus,
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
    pub prices: Vec<GqlPrice>,
    pub inventory_quantity: i32,
    pub inventory_policy: String,
    pub in_stock: bool,
}

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
    pub tax_rate: String,
    pub tax_included: bool,
    pub countries: Vec<String>,
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
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
    pub total_amount: String,
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
    pub line_items: Vec<GqlCartLineItem>,
    pub delivery_groups: Vec<GqlCartDeliveryGroup>,
}

#[derive(SimpleObject)]
pub struct GqlCartLineItem {
    pub id: Uuid,
    pub cart_id: Uuid,
    pub product_id: Option<Uuid>,
    pub variant_id: Option<Uuid>,
    pub shipping_profile_slug: String,
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
pub struct GqlCartDeliveryGroup {
    pub shipping_profile_slug: String,
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
    pub total_amount: String,
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
}

#[derive(SimpleObject)]
pub struct GqlOrderLineItem {
    pub id: Uuid,
    pub order_id: Uuid,
    pub product_id: Option<Uuid>,
    pub variant_id: Option<Uuid>,
    pub shipping_profile_slug: String,
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
    pub provider_id: Option<String>,
    pub cancellation_reason: Option<String>,
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
    pub authorized_at: Option<String>,
    pub captured_at: Option<String>,
    pub cancelled_at: Option<String>,
    pub payments: Vec<GqlPayment>,
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
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
    pub shipped_at: Option<String>,
    pub delivered_at: Option<String>,
    pub cancelled_at: Option<String>,
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
    pub amount: String,
    pub compare_at_amount: Option<String>,
}

#[derive(InputObject)]
pub struct UpdateProductInput {
    pub translations: Option<Vec<ProductTranslationInput>>,
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
    pub selected_shipping_option_id: Option<Uuid>,
}

#[derive(InputObject)]
#[graphql(name = "ShipFulfillmentInput")]
pub struct ShipFulfillmentInputObject {
    pub carrier: String,
    pub tracking_number: String,
    pub metadata: Option<String>,
}

#[derive(InputObject)]
#[graphql(name = "DeliverFulfillmentInput")]
pub struct DeliverFulfillmentInputObject {
    pub delivered_note: Option<String>,
    pub metadata: Option<String>,
}

#[derive(InputObject)]
#[graphql(name = "CancelFulfillmentInput")]
pub struct CancelFulfillmentInputObject {
    pub reason: Option<String>,
    pub metadata: Option<String>,
}

#[derive(InputObject)]
#[graphql(name = "CreateShippingOptionInput")]
pub struct CreateShippingOptionInputObject {
    pub name: String,
    pub currency_code: String,
    pub amount: String,
    pub provider_id: Option<String>,
    pub allowed_shipping_profile_slugs: Option<Vec<String>>,
    pub metadata: Option<String>,
}

#[derive(InputObject)]
#[graphql(name = "UpdateShippingOptionInput")]
pub struct UpdateShippingOptionInputObject {
    pub name: Option<String>,
    pub currency_code: Option<String>,
    pub amount: Option<String>,
    pub provider_id: Option<String>,
    pub allowed_shipping_profile_slugs: Option<Vec<String>>,
    pub metadata: Option<String>,
}

#[derive(InputObject)]
#[graphql(name = "CreateShippingProfileInput")]
pub struct CreateShippingProfileInputObject {
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub metadata: Option<String>,
}

#[derive(InputObject)]
#[graphql(name = "UpdateShippingProfileInput")]
pub struct UpdateShippingProfileInputObject {
    pub slug: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub metadata: Option<String>,
}

impl From<dto::ProductResponse> for GqlProduct {
    fn from(product: dto::ProductResponse) -> Self {
        Self {
            id: product.id,
            status: product.status.into(),
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
            tax_rate: value.tax_rate.to_string(),
            tax_included: value.tax_included,
            countries: value.countries,
            metadata: value.metadata.to_string(),
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
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
            total_amount: value.total_amount.to_string(),
            metadata: value.metadata.to_string(),
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
            completed_at: value.completed_at.map(|value| value.to_rfc3339()),
            line_items: value.line_items.into_iter().map(Into::into).collect(),
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

impl From<dto::CartDeliveryGroupResponse> for GqlCartDeliveryGroup {
    fn from(value: dto::CartDeliveryGroupResponse) -> Self {
        Self {
            shipping_profile_slug: value.shipping_profile_slug,
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
            total_amount: order.total_amount.to_string(),
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
            provider_id: value.provider_id,
            cancellation_reason: value.cancellation_reason,
            metadata: value.metadata.to_string(),
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
            authorized_at: value.authorized_at.map(|value| value.to_rfc3339()),
            captured_at: value.captured_at.map(|value| value.to_rfc3339()),
            cancelled_at: value.cancelled_at.map(|value| value.to_rfc3339()),
            payments: value.payments.into_iter().map(Into::into).collect(),
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
            metadata: value.metadata.to_string(),
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
            shipped_at: value.shipped_at.map(|value| value.to_rfc3339()),
            delivered_at: value.delivered_at.map(|value| value.to_rfc3339()),
            cancelled_at: value.cancelled_at.map(|value| value.to_rfc3339()),
        }
    }
}
