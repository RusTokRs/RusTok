//! # RusToK Pricing
//!
//! Pricing domain module extracted from commerce.
//! Provides price resolution, currency rules, discount calculation, price list management,
//! and transactional translation change tracking.

#![deny(clippy::float_arithmetic)]

pub mod entities {
    pub use rustok_pricing_persistence::entities::{price, price_list, price_list_translation};
}

pub mod migrations;
pub mod module;
pub mod ports;
mod price_list_owner_port;
mod public_error;
mod read_context;
pub mod services;
mod translation_changes;
mod translation_target;
mod write_context;

#[cfg(test)]
mod contract_tests;

pub use module::PricingModule;
pub use ports::{
    ActivePriceListProjectionRequest, ActivePriceListProjectionSnapshot,
    AdminProductPricingProjectionRequest, ApplyVariantDiscountRequest,
    PreviewVariantDiscountRequest, PriceListProjectionRequest, PriceListProjectionSnapshot,
    PricingReadPort, PricingWritePort, ResolveProductPriceRequest, ResolvedProductPriceSnapshot,
    SetPriceListPercentageRuleRequest, SetPriceListScopeRequest,
    StorefrontProductPricingProjectionRequest, UpsertVariantPriceRequest,
};
pub use price_list_owner_port::{PriceListOwnerPort, in_process_price_list_owner_port};
pub use public_error::{PricingPublicError, map_pricing_port_error, map_pricing_service_error};
pub use read_context::{InProcessPricingReadPort, in_process_pricing_read_port};
pub use write_context::{InProcessPricingWritePort, in_process_pricing_write_port};

pub use services::{
    ActivePriceListOption, AdminPricingPrice, AdminPricingProductDetail, AdminPricingProductList,
    AdminPricingProductListItem, AdminPricingProductTranslation, AdminPricingVariant,
    CreatePriceListOwnerInput, PriceAdjustmentKind, PriceAdjustmentPreview, PriceListOwnerService,
    PriceListOwnerSnapshot, PriceListOwnerTranslationInput, PriceListRule, PriceListRuleKind,
    PriceListTranslationExactLocaleApply, PriceListTranslationExactLocaleApplyReceipt,
    PriceListTranslationExactLocaleError, PriceListTranslationExactLocaleRecord,
    PriceListTranslationExactLocaleResult, PriceListTranslationExactLocaleSnapshot,
    PriceListTranslationService, PriceResolutionContext, PricingService, ResolvedPrice,
    StorefrontPricingPrice, StorefrontPricingProductDetail, StorefrontPricingProductList,
    StorefrontPricingProductListItem, StorefrontPricingProductTranslation,
    StorefrontPricingVariant, UpdatePriceListOwnerInput,
};
pub use translation_changes::{
    PriceListTranslationChangeLifecycle, PriceListTranslationChangeRecord,
};
pub use translation_target::{
    PriceListTranslationTargetProvider, register_price_list_translation_target_provider,
};
