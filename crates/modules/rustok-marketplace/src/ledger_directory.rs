use std::sync::Arc;

use rustok_api::{PortContext, PortError};
use rustok_marketplace_balance::{
    ListMarketplaceSellerLedgerEntriesRequest, MarketplaceLedgerEntryListResponse,
    MarketplaceLedgerReadPort, MarketplaceLedgerTransactionResponse,
    MarketplaceSellerBalanceResponse, ReadMarketplaceOrderLedgerRequest,
    ReadMarketplaceSellerBalanceRequest,
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
        if order_id.is_nil() {
            return Err(PortError::validation(
                "marketplace.ledger.order_id",
                "order_id must not be nil",
            ));
        }
        self.ledger_reader
            .read_order_ledger(context, ReadMarketplaceOrderLedgerRequest { order_id })
            .await
    }

    pub async fn list_seller_entries(
        &self,
        context: PortContext,
        request: ListMarketplaceSellerLedgerEntriesRequest,
    ) -> Result<MarketplaceLedgerEntryListResponse, PortError> {
        if request.seller_id.is_nil() {
            return Err(PortError::validation(
                "marketplace.ledger.seller_id",
                "seller_id must not be nil",
            ));
        }
        self.ledger_reader
            .list_seller_entries(context, request)
            .await
    }

    pub async fn read_seller_balance(
        &self,
        context: PortContext,
        seller_id: Uuid,
        currency_code: impl Into<String>,
    ) -> Result<MarketplaceSellerBalanceResponse, PortError> {
        if seller_id.is_nil() {
            return Err(PortError::validation(
                "marketplace.ledger.seller_id",
                "seller_id must not be nil",
            ));
        }
        let currency = currency_code.into();
        let trimmed = currency.trim();
        if trimmed.len() != 3 || !trimmed.bytes().all(|byte| byte.is_ascii_alphabetic()) {
            return Err(PortError::validation(
                "marketplace.ledger.currency_code",
                "currency_code must contain exactly three ASCII letters",
            ));
        }
        self.ledger_reader
            .read_seller_balance(
                context,
                ReadMarketplaceSellerBalanceRequest {
                    seller_id,
                    currency_code: currency,
                },
            )
            .await
    }
}
