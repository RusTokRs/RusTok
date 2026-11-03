mod calculation;

pub use crate::domain::calculation::{TaxPolicyCountryRule, TaxPolicySnapshot};
pub use calculation::{
    CalculatedTaxLine, TaxCalculationInput, TaxCalculationResult, TaxableAmount,
};
