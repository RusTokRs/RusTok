//! # RusToK Tax Module
//!
//! Provides tax calculation primitives, country and regional tax policy rules,
//! taxable amount valuations, and provider seams for external tax engines.

#![deny(clippy::float_arithmetic)]



mod calculation_context;
pub mod error;
pub mod ports;
pub mod services;

pub use calculation_context::{InProcessTaxCalculationPort, in_process_tax_calculation_port};
pub use error::{TaxError, TaxResult};
pub use ports::TaxCalculationPort;
pub use services::{
    CalculatedTaxLine, TaxCalculationInput, TaxCalculationResult, TaxPolicyCountryRule,
    TaxPolicySnapshot, TaxService, TaxableAmount,
};

pub mod module;
pub use module::TaxModule;

#[cfg(test)]
mod contract_tests;

