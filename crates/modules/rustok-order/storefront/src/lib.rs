//! # rustok-order-storefront
//!
//! Leptos storefront UI package for the `rustok-order` module.
//!
//! Exposes order checkout result presentation, order history views, and checkout
//! completion handoff action components.

pub mod core;
mod i18n;
pub mod model;
pub mod transport;
mod ui;

pub use model::{
    StorefrontCustomerAddress, StorefrontOrder, StorefrontOrderAdjustment, StorefrontOrderLineItem,
    StorefrontOrdersResponse,
};
pub use ui::{OrderCheckoutCompleteButton, OrderCheckoutResultCard, OrderView, OrdersHistoryView};

#[cfg(test)]
mod contract_tests;
