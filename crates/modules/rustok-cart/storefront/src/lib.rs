//! # RusToK Cart Storefront
//!
//! Leptos storefront UI package for the `rustok-cart` module.
//! Provides storefront cart views, drawer components, view models, and transport adapters.

pub mod core;
mod i18n;
pub mod model;
pub mod transport;
mod ui;

pub use ui::leptos::{
    CartCheckoutHandoffCard, CartDrawer, CartDrawerState, CartFloatingTrigger, CartHeaderTrigger,
    CartView, use_cart_drawer,
};

#[cfg(test)]
mod contract_tests;
