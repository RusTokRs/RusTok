//! # RusToK Cart
//!
//! Cart domain module for the RusToK commerce platform.
//!
//! Owns shopping carts, line items, cart promotions, guest cart capabilities,
//! checkout snapshots, and atomic checkout transitions.

#![deny(clippy::float_arithmetic)]

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
pub mod module;
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
pub use checkout_snapshot::{
    CompleteCartCheckoutRequest, InProcessCartCheckoutPort, PrepareCartCheckoutSnapshotRequest,
    PreparedCartCheckoutSnapshot,
};
pub use dto::{
    AddCartLineItemInput, AddMarketplaceCartLineItemInput, AddMarketplaceCartLineItemResponse,
    CartAdjustmentResponse, CartDeliveryGroupResponse, CartLineFulfillmentRequirement,
    CartLineItemResponse, CartMarketplaceLineSnapshot, CartResponse, CartShippingOptionSummary,
    CartShippingSelectionInput, CartStatus, CartTaxLineResponse, CreateCartInput,
    ListCartMarketplaceLineSnapshotsRequest, MarketplaceCartLineSnapshotInput,
    SetCartAdjustmentInput, UpdateCartContextInput,
};
pub use entities::{
    cart as cart_entity, cart_adjustment, cart_line_item, cart_line_item_marketplace_snapshot,
    cart_line_item_translation, cart_shipping_selection, cart_tax_line,
};
pub use error::{CartError, CartResult};
pub use guarded_ports::{
    guarded_cart_checkout_port as in_process_cart_checkout_port,
    guarded_cart_storefront_port as in_process_cart_storefront_port,
};
pub use guest_access::{
    GUEST_CART_TOKEN_COOKIE, GUEST_CART_TOKEN_HEADER, attach_transient_guest_token,
    current_guest_cart_token, guest_cart_claim, guest_cart_token_from_context,
    hash_guest_cart_token, issued_guest_cart_token, normalize_presented_guest_cart_token,
    prepare_guest_cart_metadata, record_issued_guest_cart_token, sanitize_guest_cart_metadata,
    take_transient_guest_token, verify_current_guest_cart_access, verify_guest_cart_token,
    with_guest_cart_request_scope,
};
pub use marketplace_snapshot::{
    AddMarketplaceCartLineRequest, BindMarketplaceCartLineSnapshotRequest,
    ListMarketplaceCartLineSnapshotsRequest, MarketplaceCartSnapshotCommandPort,
    MarketplaceCartSnapshotReadPort, in_process_marketplace_cart_snapshot_command_port,
    in_process_marketplace_cart_snapshot_read_port,
};
pub use module::CartModule;
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

#[cfg(test)]
mod contract_tests;
