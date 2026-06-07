pub mod admin_read;
pub mod inventory;
mod policy;
pub mod public_channel;

pub use inventory::InventoryService;
pub use policy::inventory_policy_allows_backorder;
pub use public_channel::{
    check_public_channel_inventory_request, check_variant_availability_for_public_channel,
    extract_allowed_channel_slugs, is_allowlist_visible_for_public_channel,
    is_metadata_visible_for_public_channel, load_available_inventory_by_variant_for_public_channel,
    load_available_inventory_for_variant_in_public_channel,
    load_inventory_projection_by_variant_for_public_channel, normalize_public_channel_slug,
    public_channel_inventory_projection, PublicChannelInventoryProjection,
};

pub use admin_read::{
    AdminInventoryPrice, AdminInventoryProductDetail, AdminInventoryProductList,
    AdminInventoryProductListItem, AdminInventoryProductTranslation, AdminInventoryProductsFilter,
    AdminInventoryReadService, AdminInventoryVariant,
};
