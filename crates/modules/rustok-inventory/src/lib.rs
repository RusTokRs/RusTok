//! # RusToK Inventory
//!
//! Inventory domain module extracted from commerce.
//! Provides stock availability checks, reservation leases, concurrency guards,
//! backorder policies, and stock location translations.

pub mod entities {
    pub use rustok_commerce_foundation::entities::{
        inventory_item, inventory_level, reservation_item, stock_location,
        stock_location_translation,
    };
}

pub mod migrations;
pub mod module;
pub mod ports;
mod reservation_owner_context;
mod reservation_port_context;
pub mod return_restock;
pub mod services;
pub mod shipment_stock;
mod translation_changes;
mod translation_target;

#[cfg(test)]
mod contract_tests;

pub use module::InventoryModule;
pub use ports::{
    InventoryAvailabilityRequest, InventoryAvailabilitySnapshot,
    InventoryIdentityReservationReleaseRequest, InventoryIdentityReservationReleaseSnapshot,
    InventoryIdentityReservationRequest, InventoryIdentityReservationSnapshot,
    InventoryReservationIdentityPort, InventoryReservationPort, InventoryReservationReleaseRequest,
    InventoryReservationReleaseSnapshot, InventoryReservationRequest, InventoryReservationSnapshot,
};
pub use reservation_owner_context::{
    PersistentInventoryReservationIdentityPort, in_process_inventory_reservation_identity_port,
};
pub use reservation_port_context::{
    InProcessInventoryReservationPort, in_process_inventory_reservation_port,
};
pub use rustok_commerce_foundation::entities::product::ProductStatus;
pub use services::{
    BootstrapService, InitialInventory, InventoryAvailabilityCheckResult,
    InventoryQuantityWriteResult, InventoryReservationReleaseWriteResult,
    InventoryReservationWriteResult, InventoryService, PublicChannelInventoryProjection,
    PublicChannelInventoryVariantProjectionInput, StockLocationTranslationExactLocaleApply,
    StockLocationTranslationExactLocaleApplyReceipt, StockLocationTranslationExactLocaleError,
    StockLocationTranslationExactLocaleRecord, StockLocationTranslationExactLocaleResult,
    StockLocationTranslationExactLocaleSnapshot, StockLocationTranslationService,
    check_public_channel_inventory_request, check_variant_availability_for_public_channel,
    extract_allowed_channel_slugs, inventory_policy_allows_backorder,
    is_allowlist_visible_for_public_channel, is_metadata_visible_for_public_channel,
    load_available_inventory_by_variant_for_public_channel,
    load_available_inventory_for_variant_in_public_channel,
    load_inventory_projection_by_variant_for_public_channel, normalize_public_channel_slug,
    public_channel_inventory_projection,
};
pub use translation_changes::{
    StockLocationTranslationChangeLifecycle, StockLocationTranslationChangeRecord,
};
pub use translation_target::{
    StockLocationTranslationTargetProvider, register_stock_location_translation_target_provider,
};
