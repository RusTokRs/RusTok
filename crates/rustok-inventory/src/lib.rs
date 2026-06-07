use async_trait::async_trait;
use rustok_core::permissions::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod migrations;
pub mod services;

pub use rustok_commerce_foundation::entities::product::ProductStatus;
pub use services::{
    check_public_channel_inventory_request, check_variant_availability_for_public_channel,
    extract_allowed_channel_slugs, inventory_policy_allows_backorder,
    is_allowlist_visible_for_public_channel, is_metadata_visible_for_public_channel,
    load_available_inventory_by_variant_for_public_channel,
    load_available_inventory_for_variant_in_public_channel,
    load_inventory_projection_by_variant_for_public_channel, normalize_public_channel_slug,
    public_channel_inventory_projection, AdminInventoryPrice, AdminInventoryProductDetail,
    AdminInventoryProductList, AdminInventoryProductListItem, AdminInventoryProductTranslation,
    AdminInventoryProductsFilter, AdminInventoryReadService, AdminInventoryVariant,
    InventoryAvailabilityCheckResult, InventoryQuantityWriteResult,
    InventoryReservationReleaseWriteResult, InventoryReservationWriteResult, InventoryService,
    PublicChannelInventoryProjection, PublicChannelInventoryVariantProjectionInput,
};

pub struct InventoryModule;

#[async_trait]
impl RusToKModule for InventoryModule {
    fn slug(&self) -> &'static str {
        "inventory"
    }

    fn name(&self) -> &'static str {
        "Inventory"
    }

    fn description(&self) -> &'static str {
        "Inventory adjustments, availability checks, and stock-level persistence"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn dependencies(&self) -> &[&'static str] {
        &["product"]
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::INVENTORY_CREATE,
            Permission::INVENTORY_READ,
            Permission::INVENTORY_UPDATE,
            Permission::INVENTORY_DELETE,
            Permission::INVENTORY_LIST,
            Permission::INVENTORY_MANAGE,
        ]
    }
}

impl MigrationSource for InventoryModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<rustok_core::MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}
