use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use uuid::Uuid;

use crate::dto::{JournalTransactionResponse, PostJournalTransactionInput};

#[async_trait]
pub trait AccountingJournalPort: Send + Sync {
    /// Posts a balanced double-entry journal transaction.
    ///
    /// Validates that total debits equal total credits per currency.
    async fn post_transaction(
        &self,
        context: PortContext,
        input: PostJournalTransactionInput,
    ) -> Result<JournalTransactionResponse, PortError>;

    /// Creates an immutable reverse storno transaction for an existing transaction.
    async fn post_storno_reversal(
        &self,
        context: PortContext,
        original_transaction_id: Uuid,
        reason: Option<String>,
    ) -> Result<JournalTransactionResponse, PortError>;
}
