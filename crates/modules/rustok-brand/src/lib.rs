//! # RusToK Brand Module
//!
//! Provides brand catalogs, manufacturer metadata, media asset attachments,
//! localized translations, brand category scoping, and product-brand associations.

pub mod dto;
mod entities;
pub mod error;
pub mod migrations;
pub mod module;
pub mod ports;
pub mod services;

pub use dto::{
    BrandCategoryDto, BrandDto, BrandFilter, BrandListResponse, BrandProductDto,
    BrandTranslationDto, BrandTranslationInput, CreateBrandInput, UpdateBrandInput,
};
pub use error::{BrandError, BrandResult};
pub use module::BrandModule;
pub use ports::{BrandPort, BrandReadPort};
pub use services::BrandService;

#[cfg(test)]
mod tests;
