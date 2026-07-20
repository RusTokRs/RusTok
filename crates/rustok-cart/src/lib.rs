use async_trait::async_trait;
use rustok_core::{MigrationDependencyDescriptor, MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod atomic_checkout_port;
pub mod checkout_snapshot;
pub mod dto;
pub mod entities;
pub mod error;
mod guarded_ports;
pub mod guest_access;
pub mod guest_access_http;
pub mod migrations;
pub mod ports;
pub mod services;

pub use atomic_checkout_port::*;
pub use checkout_snapshot::*;
pub use dto::*;
pub use entities::*;
pub use error::{CartError, CartResult};
pub use guarded_ports::{
    guarded_cart_checkout_port as in_process_cart_checkout_port,
    guarded_cart_storefront_port as in_process_cart_storefront_port,
};
pub use guest_access::*;
pub use ports::{
    CartCheckoutContextUpdateRequest, CartCheckoutLifecycleRequest, CartCheckoutPort,
    CartCheckoutSnapshotRequest, CartPromotionKindRequest, CartPromotionPort, CartPromotionRequest,
    CartPromotionScopeRequest, CartStorefrontAddLineItemRequest,
    CartStorefrontContextUpdateRequest, CartStorefrontCreateRequest,
    CartStorefrontLineItemPricingRequest, CartStorefrontLineItemQuantityRequest,
    CartStorefrontPort, CartStorefrontReadRequest, CartStorefrontRemoveLineItemRequest,
    CartStorefrontRepriceRequest, in_process_cart_promotion_port,
};
pub use services::CartService;
pub use services::cart::{
    CartLineItemPricingUpdate, CartPricingAdjustmentUpdate, CartPromotionPreview,
};

pub struct CartModule;

#[async_trait]
impl RusToKModule for CartModule {
    fn slug(&self) -> &'static str {
        "cart"
    }

    fn name(&self) -> &'static str {
        "Cart"
    }

    fn description(&self) -> &'static str {
        "Default cart submodule in the ecommerce family"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
}

impl MigrationSource for CartModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}
