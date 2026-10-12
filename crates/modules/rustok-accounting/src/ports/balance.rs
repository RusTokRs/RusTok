use async_trait::async_trait;
use rustok_api::{PortContext, PortError};

use crate::dto::{TrialBalanceQuery, TrialBalanceResponse};

#[async_trait]
pub trait AccountingBalanceReadPort: Send + Sync {
    /// Reads trial balance (Оборотно-сальдовая ведомость) aggregated by accounts
    /// and optional subconto filters.
    async fn read_trial_balance(
        &self,
        context: PortContext,
        query: TrialBalanceQuery,
    ) -> Result<TrialBalanceResponse, PortError>;

    /// Reads closing balance for a single account code.
    async fn read_account_balance(
        &self,
        context: PortContext,
        account_code: &str,
        currency_code: &str,
    ) -> Result<i64, PortError>;
}
