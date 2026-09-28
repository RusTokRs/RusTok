//! Owner-defined cross-boundary ports for the product catalog.
//!
//! Provides transport-neutral read projections (`ProductCatalogReadPort`)
//! for checkout, storefront, and administrative commerce consumers.

mod catalog_read;
mod diagnostics;
mod types;

pub use catalog_read::*;
pub use types::*;
