//! # RusToK Fulfillment Storefront
//!
//! Leptos storefront UI package for the `rustok-fulfillment` module.
//! Provides shipping selection panels, delivery group models, and fulfillment handoff notices.

#![recursion_limit = "256"]

pub mod core;
mod i18n;
mod model;
pub mod transport;
mod ui;

pub use model::{StorefrontDeliveryGroup, StorefrontShippingOption};
pub use ui::{
    FulfillmentShippingHandoffNotice, FulfillmentShippingSelectionPanel, FulfillmentView,
};

#[cfg(test)]
mod contract_tests;
