use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};

use crate::{TaxCalculationInput, TaxCalculationResult, TaxError};

/// Transport-neutral owner boundary for tax calculation.
#[async_trait]
pub trait TaxCalculationPort: Send + Sync {
    async fn calculate_tax(
        &self,
        context: PortContext,
        request: TaxCalculationInput,
    ) -> Result<TaxCalculationResult, PortError>;
}

#[async_trait]
impl TaxCalculationPort for crate::TaxService {
    async fn calculate_tax(
        &self,
        context: PortContext,
        request: TaxCalculationInput,
    ) -> Result<TaxCalculationResult, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        self.calculate(request)
            .await
            .map_err(tax_error_to_port_error)
    }
}

fn tax_error_to_port_error(error: TaxError) -> PortError {
    match error {
        TaxError::Validation(message) => PortError::validation("tax.validation", message),
    }
}
