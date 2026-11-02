//! # RusToK Marketplace Balance
//!
//! Native module owner for Marketplace Family seller balances, escrow holds, and ledger postings.
//! Maintains double-entry seller sub-ledger accounting, escrow bucket transitions, and financial reversals.

#![deny(clippy::float_arithmetic)]

mod balance;
mod balance_transfer;
mod receipts;
mod reversal;
#[cfg(test)]
mod tests;

pub mod accounting_bridge;
pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod module;
pub mod ports;
pub mod service;

pub use accounting_bridge::{
    ACCOUNT_COMMISSION_REVENUE, ACCOUNT_MARKETPLACE_CLEARING, ACCOUNT_SELLER_PAYABLE,
    MarketplaceAccountingBridge,
};
pub use dto::{
    ListMarketplaceSellerLedgerEntriesRequest, MAX_LEDGER_BALANCE_TRANSFER_LINES,
    MAX_LEDGER_ENTRIES_PER_PAGE, MAX_LEDGER_REVERSAL_LINES, MarketplaceLedgerAccountCode,
    MarketplaceLedgerEntryDirection, MarketplaceLedgerEntryListResponse,
    MarketplaceLedgerEntryResponse, MarketplaceLedgerReversalEntryResponse,
    MarketplaceLedgerReversalKind, MarketplaceLedgerReversalLineInput,
    MarketplaceLedgerReversalResponse, MarketplaceLedgerTransactionResponse,
    MarketplaceLedgerTransactionStatus, MarketplaceSellerBalanceBucket,
    MarketplaceSellerBalanceResponse, MarketplaceSellerBalanceTransferKind,
    MarketplaceSellerBalanceTransferLineInput, MarketplaceSellerBalanceTransferLineResponse,
    MarketplaceSellerBalanceTransferResponse, MarketplaceSellerPayableBalanceResponse,
    PostMarketplaceLedgerReversalInput, PostMarketplaceOrderLedgerInput,
    PostMarketplaceSellerBalanceTransferInput, ReadMarketplaceOrderLedgerRequest,
    ReadMarketplaceSellerBalanceRequest, RebuildMarketplaceSellerBalanceInput,
};
pub use error::{MarketplaceLedgerError, MarketplaceLedgerResult};
pub use module::{MarketplaceBalanceModule, Marketplace_balanceModule};
pub use ports::{
    MarketplaceLedgerCommandPort, MarketplaceLedgerReadPort,
    in_process_marketplace_ledger_command_port,
};
pub use service::MarketplaceLedgerService;

pub type MarketplaceBalanceError = MarketplaceLedgerError;
pub type MarketplaceBalanceResult<T> = MarketplaceLedgerResult<T>;
pub type MarketplaceBalanceService = MarketplaceLedgerService;

#[cfg(test)]
mod contract_tests;
