use async_trait::async_trait;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod error;
pub mod ports;
pub mod services;

pub use error::{TaxError, TaxResult};
pub use ports::*;
pub use services::{
    CalculatedTaxLine, TaxCalculationInput, TaxCalculationResult, TaxPolicyCountryRule,
    TaxPolicySnapshot, TaxService, TaxableAmount,
};

pub struct TaxModule;

#[async_trait]
impl RusToKModule for TaxModule {
    fn slug(&self) -> &'static str {
        "tax"
    }

    fn name(&self) -> &'static str {
        "Tax"
    }

    fn description(&self) -> &'static str {
        "Tax domain foundation, tax-line calculation contract, and provider seam"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
}

impl MigrationSource for TaxModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        Vec::new()
    }
}
