pub mod bootstrap;
pub mod inventory;
mod policy;
pub mod public_channel;
pub mod stock_location_translation;

pub use bootstrap::{BootstrapService, InitialInventory};
pub use inventory::{
    InventoryAvailabilityCheckResult, InventoryQuantityWriteResult,
    InventoryReservationReleaseWriteResult, InventoryReservationWriteResult, InventoryService,
};
pub use policy::inventory_policy_allows_backorder;
pub use public_channel::{
    PublicChannelInventoryProjection, PublicChannelInventoryVariantProjectionInput,
    check_public_channel_inventory_request, check_variant_availability_for_public_channel,
    extract_allowed_channel_slugs, is_allowlist_visible_for_public_channel,
    is_metadata_visible_for_public_channel, load_available_inventory_by_variant_for_public_channel,
    load_available_inventory_for_variant_in_public_channel,
    load_inventory_projection_by_variant_for_public_channel, normalize_public_channel_slug,
    public_channel_inventory_projection,
};
pub use stock_location_translation::{
    StockLocationTranslationExactLocaleApply, StockLocationTranslationExactLocaleApplyReceipt,
    StockLocationTranslationExactLocaleError, StockLocationTranslationExactLocaleRecord,
    StockLocationTranslationExactLocaleResult, StockLocationTranslationExactLocaleSnapshot,
    StockLocationTranslationService,
};
