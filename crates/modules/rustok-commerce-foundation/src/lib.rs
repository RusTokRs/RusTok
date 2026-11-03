//! # RusToK Commerce Foundation
//!
//! Shared commerce DTOs, entities, search helpers, and error types across RusToK commerce modules.

pub mod dto;
pub mod entities;
pub mod error;
pub mod search;

pub use dto::{
    AdjustInventoryInput, CreateVariantInput, PriceInput, PriceResponse, VariantResponse,
    VariantTranslationResponse,
};
pub use error::{CommerceError, CommerceResult};
pub use search::{
    product_translation_title_and_locale_search_condition,
    product_translation_title_search_condition, MAX_SEARCH_QUERY_LEN,
};

#[cfg(test)]
mod contract_tests;
