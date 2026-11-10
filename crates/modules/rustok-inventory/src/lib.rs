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
pub mod product_port;
#[path = "ports.rs"]
mod ports_impl;

/// Canonical owner contracts and admitted in-process entry points. The persistent
/// implementation stays private; all public paths use the same diagnostic boundary.
pub mod ports {
    pub use crate::ports_impl::{
        InventoryAvailabilityRequest, InventoryAvailabilitySnapshot,
        InventoryIdentityReservationAdoptionItem, InventoryIdentityReservationAdoptionRequest,
        InventoryIdentityReservationAdoptionSnapshot, InventoryIdentityReservationReleaseRequest,
        InventoryIdentityReservationReleaseSnapshot, InventoryIdentityReservationRequest,
        InventoryIdentityReservationSnapshot, InventoryReservationIdentityPort,
        InventoryReservationPort, InventoryReservationReleaseRequest,
        InventoryReservationReleaseSnapshot, InventoryReservationRequest,
        InventoryReservationSnapshot, MAX_RESERVATION_ADOPTION_ITEMS,
    };
    pub use crate::product_port::{
        DatabaseFallbackProductAdapter, InventoryProductPort, InventoryProductVariantInfo,
    };
    pub use crate::reservation_owner_context::{
        PersistentInventoryReservationIdentityPort, in_process_inventory_reservation_identity_port,
    };
    pub use crate::reservation_port_context::{
        InProcessInventoryReservationPort, in_process_inventory_reservation_port,
    };
}
mod reservation_adoption;
mod reservation_events;
mod reservation_owner_context;
mod reservation_port_context;
pub mod return_restock;
pub mod services;
pub mod shipment_stock;
pub mod source_history;
mod stock_events;
mod translation_changes;
mod translation_target;

#[cfg(test)]
mod contract_tests;

pub use module::{InventoryModule, InventoryRuntimeSelected};
pub use ports::{
    InProcessInventoryReservationPort, InventoryAvailabilityRequest, InventoryAvailabilitySnapshot,
    InventoryIdentityReservationAdoptionItem, InventoryIdentityReservationAdoptionRequest,
    InventoryIdentityReservationAdoptionSnapshot, InventoryIdentityReservationReleaseRequest,
    InventoryIdentityReservationReleaseSnapshot, InventoryIdentityReservationRequest,
    InventoryIdentityReservationSnapshot, InventoryReservationIdentityPort,
    InventoryReservationPort, InventoryReservationReleaseRequest,
    InventoryReservationReleaseSnapshot, InventoryReservationRequest, InventoryReservationSnapshot,
    InventoryProductPort, InventoryProductVariantInfo, DatabaseFallbackProductAdapter,
    MAX_RESERVATION_ADOPTION_ITEMS, PersistentInventoryReservationIdentityPort,
    in_process_inventory_reservation_identity_port, in_process_inventory_reservation_port,
};
pub use rustok_commerce_foundation::entities::product::ProductStatus;
pub use services::{
    BootstrapService, CatalogDeletionBlocker, CatalogDeletionError, InitialInventory,
    InventoryAvailabilityCheckResult, InventoryQuantityWriteResult,
    InventoryReservationReleaseWriteResult, InventoryReservationWriteResult, InventoryService,
    MAX_CATALOG_DELETE_VARIANTS, PublicChannelInventoryProjection,
    PublicChannelInventoryVariantProjectionInput, QuantityWriteBlocker, QuantityWriteError,
    StockLocationTranslationExactLocaleApply, StockLocationTranslationExactLocaleApplyReceipt,
    StockLocationTranslationExactLocaleError, StockLocationTranslationExactLocaleRecord,
    StockLocationTranslationExactLocaleResult, StockLocationTranslationExactLocaleSnapshot,
    StockLocationTranslationService, check_public_channel_inventory_request,
    check_variant_availability_for_public_channel, extract_allowed_channel_slugs,
    inventory_policy_allows_backorder, is_allowlist_visible_for_public_channel,
    is_metadata_visible_for_public_channel, load_available_inventory_by_variant_for_public_channel,
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
