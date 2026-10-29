//! # rustok-payment-storefront
//!
//! Leptos storefront UI package for the `rustok-payment` module.
//!
//! Exposes payment collection cards, handoff action buttons, and payment views
//! for storefront checkouts and order completion.

pub mod core;
mod i18n;
pub mod transport;
mod ui;

pub use ui::{PaymentCollectionActionButton, PaymentCollectionCard, PaymentView};

#[cfg(test)]
mod contract_tests;
