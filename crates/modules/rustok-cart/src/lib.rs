use async_trait::async_trait;
use rustok_core::{MigrationDependencyDescriptor, MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

mod atomic_checkout_guard;
pub mod atomic_checkout_port;
pub mod checkout_snapshot;
pub mod dto;
pub mod entities;
pub mod error;
mod guarded_ports;
pub mod guest_access;
pub mod guest_access_http;
pub mod marketplace_snapshot;
pub mod migrations;
mod owner_ports;
pub mod ports;
mod promotion_guard;
pub mod services;

pub use atomic_checkout_guard::{
    AtomicCartCheckoutBinding, AtomicCartCheckoutHandle, bind_in_process_atomic_cart_checkout,
    bind_in_process_atomic_cart_checkout_with_pricing, in_process_atomic_cart_checkout_port,
};
pub use atomic_checkout_port::{
    AtomicCartCheckoutPort, AtomicCartCheckoutPricingResolver, CartCheckoutLineItemPricingUpdate,
    CartCheckoutPricingPlan,
};
pub use checkout_snapshot::*;
pub use dto::*;
pub use entities::*;
pub use error::{CartError, CartResult};
pub use guarded_ports::{
    guarded_cart_checkout_port as in_process_cart_checkout_port,
    guarded_cart_storefront_port as in_process_cart_storefront_port,
};
pub use guest_access::*;
pub use marketplace_snapshot::*;
pub use ports::{
    CartCheckoutContextUpdateRequest, CartCheckoutLifecycleRequest, CartCheckoutPort,
    CartCheckoutSnapshotRequest, CartPromotionKindRequest, CartPromotionPort, CartPromotionRequest,
    CartPromotionScopeRequest, CartStorefrontAddLineItemRequest,
    CartStorefrontContextUpdateRequest, CartStorefrontCreateRequest,
    CartStorefrontLineItemPricingRequest, CartStorefrontLineItemQuantityRequest,
    CartStorefrontPort, CartStorefrontReadRequest, CartStorefrontRemoveLineItemRequest,
    CartStorefrontRepriceRequest,
};
pub use promotion_guard::guarded_cart_promotion_port as in_process_cart_promotion_port;
pub use services::cart::{
    CartLineItemPricingUpdate, CartPricingAdjustmentUpdate, CartPromotionPreview,
};
pub use services::{CartMarketplaceSnapshotService, CartService};

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
