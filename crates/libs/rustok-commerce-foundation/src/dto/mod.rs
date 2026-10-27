//! Shared commerce DTOs.
//!
//! Product, product-translation, product-image and product-price DTOs are owned by
//! `rustok-product`; this crate keeps only the variant- and inventory-level DTOs the
//! pricing, inventory and cart owners share.

pub mod variant;

pub use variant::*;
