/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Canonical platform owner of universal double-entry bookkeeping, Chart of Accounts,
//! balanced journal transactions, posting rules, and relational subconto analytics across RusToK.

#![deny(clippy::float_arithmetic)]

pub mod domain;
pub mod dto;
pub mod error;
pub mod events;
pub mod module;
pub mod ports;

pub use domain::{
    AccountDefinition, AccountType, PostingDirection, PostingRule, standard_accounts,
    validate_transaction_balance,
};
pub use dto::{
    AccountResponse, CreateAccountInput, JournalPostingItem, JournalPostingResponse,
    JournalTransactionResponse, PostJournalTransactionInput, TrialBalanceLine, TrialBalanceQuery,
    TrialBalanceResponse,
};
pub use error::{AccountingError, AccountingResult};
pub use events::{
    ACCOUNTING_EVENT_ACCOUNT_CREATED, ACCOUNTING_EVENT_ACCOUNT_UPDATED,
    ACCOUNTING_EVENT_TRANSACTION_POSTED, AccountCreatedEvent, AccountUpdatedEvent,
    JournalTransactionPostedEvent,
};
pub use module::AccountingModule;
pub use ports::{AccountingBalanceReadPort, AccountingJournalPort};
