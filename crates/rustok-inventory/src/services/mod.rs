pub mod admin_read;
pub mod inventory;

pub use inventory::InventoryService;

pub use admin_read::{
    AdminInventoryPrice, AdminInventoryProductDetail, AdminInventoryProductList,
    AdminInventoryProductListItem, AdminInventoryProductTranslation, AdminInventoryProductsFilter,
    AdminInventoryReadService, AdminInventoryVariant,
};
