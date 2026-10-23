pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod module;
pub mod ports;
pub mod services;

pub use dto::*;
pub use error::{BrandError, BrandResult};
pub use module::BrandModule;
pub use ports::BrandPort;
pub use services::BrandService;

#[cfg(test)]
mod contract_tests;
