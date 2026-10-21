/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! FBA Bridge connecting Marketplace Seller Escrow Ledger transactions to General Ledger journal entries.

use rustok_accounting::{
    AccountingJournalPort, JournalPostingItem, JournalTransactionResponse,
    PostJournalTransactionInput, PostingDirection, validate_transaction_balance,
};
use rustok_api::{PortContext, PortError};
use std::sync::Arc;
use uuid::Uuid;

use crate::dto::{
    MarketplaceLedgerAccountCode, MarketplaceLedgerEntryDirection,
    MarketplaceLedgerReversalResponse, MarketplaceLedgerTransactionResponse,
};

/// Canonical Chart of Accounts mapping for Marketplace seller operations:
/// - `76.09`: Расчеты по средствам маркетплейса в эскроу (Marketplace Escrow / Clearing)
pub const ACCOUNT_MARKETPLACE_CLEARING: &str = "76.09";
/// - `90.01`: Выручка от комиссионных вознаграждений маркетплейса (Platform Commission Revenue)
pub const ACCOUNT_COMMISSION_REVENUE: &str = "90.01";
/// - `76.05`: Расчеты с продавцами маркетплейса (Seller Payable)
pub const ACCOUNT_SELLER_PAYABLE: &str = "76.05";

/// Bridge service translating Marketplace wallet transactions to General Ledger.
pub struct MarketplaceAccountingBridge {
    journal_port: Arc<dyn AccountingJournalPort>,
}

impl MarketplaceAccountingBridge {
    pub fn new(journal_port: Arc<dyn AccountingJournalPort>) -> Self {
        Self { journal_port }
    }

    /// Maps a completed marketplace transaction into balanced general ledger postings.
    pub fn map_to_journal_input(
        tx: &MarketplaceLedgerTransactionResponse,
        currency_code: &str,
    ) -> PostJournalTransactionInput {
        let mut lines = Vec::new();

        for entry in &tx.entries {
            let account_code = match entry.account_code {
                MarketplaceLedgerAccountCode::MarketplaceClearing => ACCOUNT_MARKETPLACE_CLEARING,
                MarketplaceLedgerAccountCode::PlatformCommissionRevenue => {
                    ACCOUNT_COMMISSION_REVENUE
                }
                MarketplaceLedgerAccountCode::SellerPayable => ACCOUNT_SELLER_PAYABLE,
            };

            let direction = match entry.direction {
                MarketplaceLedgerEntryDirection::Debit => PostingDirection::Debit,
                MarketplaceLedgerEntryDirection::Credit => PostingDirection::Credit,
            };

            let cur = if !entry.currency_code.is_empty() {
                entry.currency_code.clone()
            } else {
                currency_code.to_string()
            };

            lines.push(JournalPostingItem {
                account_code: account_code.to_string(),
                direction,
                amount_minor: entry.amount,
                currency_code: cur,
                party_id: entry.seller_id,
                contract_id: None,
                item_id: Some(entry.order_line_item_id),
                warehouse_id: None,
                description: Some(format!(
                    "Assessment {} allocation {}",
                    entry.assessment_id, entry.allocation_id
                )),
            });
        }

        PostJournalTransactionInput {
            transaction_number: Some(format!("MP-{}", tx.id)),
            document_type: tx.source_kind.clone(),
            document_id: Some(tx.order_id),
            description: Some(format!("Marketplace ledger sync for tx {}", tx.id)),
            posted_at: Some(tx.posted_at.into()),
            lines,
        }
    }

    /// Validates and posts the marketplace transaction to the central General Ledger.
    pub async fn post_to_general_ledger(
        &self,
        context: PortContext,
        tx: &MarketplaceLedgerTransactionResponse,
        currency_code: &str,
    ) -> Result<JournalTransactionResponse, PortError> {
        let input = Self::map_to_journal_input(tx, currency_code);

        // Preflight domain check for zero-sum invariant:
        let balance_check = input.lines.iter().map(|line| {
            (
                line.currency_code.as_str(),
                line.direction,
                line.amount_minor,
            )
        });
        validate_transaction_balance(balance_check).map_err(|err| {
            PortError::invariant_violation(
                "marketplace_accounting_bridge.unbalanced_postings",
                format!("Marketplace transaction cannot be posted to G/L: {err}"),
            )
        })?;

        self.journal_port.post_transaction(context, input).await
    }

    /// Generates storno reversal in the general ledger for a reversed marketplace transaction.
    pub async fn post_storno_reversal(
        &self,
        context: PortContext,
        original_gl_transaction_id: Uuid,
        reversal: &MarketplaceLedgerReversalResponse,
    ) -> Result<JournalTransactionResponse, PortError> {
        if original_gl_transaction_id.is_nil() {
            return Err(PortError::validation(
                "marketplace_accounting_bridge.original_gl_transaction_id_invalid",
                "original_gl_transaction_id must not be nil UUID",
            ));
        }
        let reason = format!("Marketplace reversal {}: {:?}", reversal.id, reversal.kind);
        self.journal_port
            .post_storno_reversal(context, original_gl_transaction_id, Some(reason))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::{
        MarketplaceLedgerAccountCode, MarketplaceLedgerEntryDirection,
        MarketplaceLedgerEntryResponse, MarketplaceLedgerTransactionStatus,
    };
    use chrono::{FixedOffset, TimeZone, Utc};

    #[test]
    fn test_map_order_commission_to_balanced_gl() {
        let tx_id = Uuid::new_v4();
        let order_id = Uuid::new_v4();
        let seller_id = Uuid::new_v4();
        let tz = FixedOffset::east_opt(0).unwrap();
        let now = tz.from_utc_datetime(&Utc::now().naive_utc());

        // Standard marketplace allocation:
        // Customer paid 1000 RUB:
        // - Debit Escrow (Clearing): 1000 RUB
        // - Credit Platform Commission: 150 RUB
        // - Credit Seller Payable: 850 RUB
        let tx = MarketplaceLedgerTransactionResponse {
            id: tx_id,
            tenant_id: Uuid::new_v4(),
            order_id,
            source_kind: "commission_assessment_batch".to_string(),
            source_id: Uuid::new_v4(),
            currency_code: "RUB".to_string(),
            debit_total_amount: 100_000,
            credit_total_amount: 100_000,
            status: MarketplaceLedgerTransactionStatus::Posted,
            posted_at: now,
            metadata: serde_json::json!({}),
            created_at: now,
            entries: vec![
                MarketplaceLedgerEntryResponse {
                    id: Uuid::new_v4(),
                    tenant_id: Uuid::new_v4(),
                    transaction_id: tx_id,
                    order_id,
                    assessment_id: Uuid::new_v4(),
                    allocation_id: Uuid::new_v4(),
                    order_line_item_id: Uuid::new_v4(),
                    seller_id: Some(seller_id),
                    account_code: MarketplaceLedgerAccountCode::MarketplaceClearing,
                    direction: MarketplaceLedgerEntryDirection::Debit,
                    currency_code: "RUB".to_string(),
                    amount: 100_000,
                    metadata: serde_json::json!({}),
                    created_at: now,
                },
                MarketplaceLedgerEntryResponse {
                    id: Uuid::new_v4(),
                    tenant_id: Uuid::new_v4(),
                    transaction_id: tx_id,
                    order_id,
                    assessment_id: Uuid::new_v4(),
                    allocation_id: Uuid::new_v4(),
                    order_line_item_id: Uuid::new_v4(),
                    seller_id: None,
                    account_code: MarketplaceLedgerAccountCode::PlatformCommissionRevenue,
                    direction: MarketplaceLedgerEntryDirection::Credit,
                    currency_code: "RUB".to_string(),
                    amount: 15_000,
                    metadata: serde_json::json!({}),
                    created_at: now,
                },
                MarketplaceLedgerEntryResponse {
                    id: Uuid::new_v4(),
                    tenant_id: Uuid::new_v4(),
                    transaction_id: tx_id,
                    order_id,
                    assessment_id: Uuid::new_v4(),
                    allocation_id: Uuid::new_v4(),
                    order_line_item_id: Uuid::new_v4(),
                    seller_id: Some(seller_id),
                    account_code: MarketplaceLedgerAccountCode::SellerPayable,
                    direction: MarketplaceLedgerEntryDirection::Credit,
                    currency_code: "RUB".to_string(),
                    amount: 85_000,
                    metadata: serde_json::json!({}),
                    created_at: now,
                },
            ],
        };

        let input = MarketplaceAccountingBridge::map_to_journal_input(&tx, "RUB");
        assert_eq!(input.document_type, "commission_assessment_batch");
        assert_eq!(input.document_id, Some(order_id));
        assert_eq!(input.lines.len(), 3);

        // Verify that the mapped input satisfies the general ledger balance invariant:
        let balance_check = input.lines.iter().map(|line| {
            (
                line.currency_code.as_str(),
                line.direction,
                line.amount_minor,
            )
        });
        assert!(validate_transaction_balance(balance_check).is_ok());
    }
}
