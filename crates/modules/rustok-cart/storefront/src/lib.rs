//! # RusToK Cart Storefront
//!
//! Leptos storefront UI package for the `rustok-cart` module.
//! Provides storefront cart views, drawer components, view models, and transport adapters.

pub mod core;
mod i18n;
pub mod model;
pub mod transport;
mod ui;

pub use ui::client::CartClient;
pub use ui::leptos::{CartCheckoutHandoffCard, CartHeaderTrigger, CartView};

#[cfg(test)]
mod contract_tests;

pub use ui::buyer::{BuyerCartController, CartCapability, use_buyer_cart};
