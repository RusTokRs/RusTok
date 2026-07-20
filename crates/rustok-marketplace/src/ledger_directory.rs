use std::sync::Arc;

use rustok_api::{PortContext, PortError};
use rustok_marketplace_ledger::{
    ListMarketplaceSellerLedgerEntriesRequest, MarketplaceLedgerEntryListResponse,
    MarketplaceLedgerReadPort, MarketplaceLedgerTransactionResponse,
    ReadMarketplaceOrderLedgerRequest,
};
use uuid::Uuid;

/// Marketplace family consumer over ledger-owned read projections.
///
/// The family root never imports ledger entities or database connections.
pub struct MarketplaceLedgerDirectoryService {
    ledger_reader: Arc<dyn MarketplaceLedgerReadPort>,
}

impl MarketplaceLedgerDirectoryService {
    pub fn new(ledger_reader: Arc<dyn MarketplaceLedgerReadPort>) -> Self {
        Self { ledger_reader }
    }

    pub async fn read_by_order(
        &self,
        context: PortContext,
        order_id: Uuid,
    ) -> Result<MarketplaceLedgerTransactionResponse, PortError> {
        self.ledger_reader
            .read_order_ledger(context, ReadMarketplaceOrderLedgerRequest { order_id })
            .await
    }

    pub async fn list_seller_entries(
        &self,
        context: PortContext,
        request: ListMarketplaceSellerLedgerEntriesRequest,
    ) -> Result<MarketplaceLedgerEntryListResponse, PortError> {
        self.ledger_reader
            .list_seller_entries(context, request)
            .await
    }
}
