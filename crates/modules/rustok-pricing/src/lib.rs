//! # RusToK Pricing
//!
//! Pricing domain module extracted from commerce.
//! Provides price resolution, currency rules, discount calculation, price list management,
//! and transactional translation change tracking.

#![deny(clippy::float_arithmetic)]

pub(crate) mod entities;

pub mod dto;
pub mod error;
pub mod integrations;
pub mod migrations;
pub mod module;
pub mod ports;
pub mod services;

#[cfg(test)]
mod tests;

pub use error::{PricingPublicError, map_pricing_port_error, map_pricing_service_error};
pub use integrations::{
    InProcessPricingReadPort, InProcessPricingWritePort, PriceListTranslationChangeLifecycle,
    PriceListTranslationChangeRecord, PriceListTranslationTargetProvider,
    PricingProductPricingPortAdapter, in_process_price_list_owner_port,
    in_process_pricing_read_port, in_process_pricing_write_port, in_process_product_pricing_port,
    register_price_list_translation_target_provider,
};
pub use module::PricingModule;
pub use ports::{
    ActivePriceListProjectionRequest, ActivePriceListProjectionSnapshot,
    AdminProductPricingProjectionRequest, ApplyVariantDiscountRequest,
    MAX_PRICE_RESOLUTION_VARIANTS, PreviewVariantDiscountRequest, PriceListOwnerPort,
    PriceListProjectionRequest, PriceListProjectionSnapshot, PricingReadPort, PricingWritePort,
    ProductPriceResolutionSnapshot, ResolveProductPriceRequest, ResolveProductPricesRequest,
    ResolvedProductPriceSnapshot, ResolvedProductPricesSnapshot, SetPriceListPercentageRuleRequest,
    SetPriceListScopeRequest, StorefrontProductPricingListRequest,
    StorefrontProductPricingProjectionRequest, UpsertVariantPriceRequest,
};
pub use services::{
    ActivePriceListOption, AdminPricingPrice, AdminPricingProductDetail, AdminPricingProductList,
    AdminPricingProductListItem, AdminPricingProductTranslation, AdminPricingVariant,
    BootstrapService, CreatePriceListOwnerInput, PriceAdjustmentKind, PriceAdjustmentPreview,
    PriceListOwnerService, PriceListOwnerSnapshot, PriceListOwnerTranslationInput, PriceListRule,
    PriceListRuleKind, PriceListTranslationExactLocaleApply,
    PriceListTranslationExactLocaleApplyReceipt, PriceListTranslationExactLocaleError,
    PriceListTranslationExactLocaleRecord, PriceListTranslationExactLocaleResult,
    PriceListTranslationExactLocaleSnapshot, PriceListTranslationService, PriceResolutionContext,
    PriceResolutionSource, PricingService, ResolvedPrice, StorefrontPricingPrice,
    StorefrontPricingProductDetail, StorefrontPricingProductList, StorefrontPricingProductListItem,
    StorefrontPricingProductTranslation, StorefrontPricingVariant, UpdatePriceListOwnerInput,
    catalog_base_scope, legacy_amount_units,
};
