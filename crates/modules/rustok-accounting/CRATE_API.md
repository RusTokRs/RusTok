# rustok-accounting / CRATE_API

## Public Modules
`domain`, `dto`, `error`, `events`, `module`, `ports`.

## Platform Capability Bindings
- **Module Registration**: Exposes `AccountingModule` implementing `RusToKModule` (slug `"accounting"`, kind `ModuleKind::Optional`, dependencies `["outbox"]`) and `MigrationSource`.
- **Double-Entry General Ledger**:
  - Balanced multi-currency journal posting (`AccountingJournalPort::post_transaction`).
  - Immutable reverse storno transactions (`AccountingJournalPort::post_storno_reversal`).
- **Trial Balance & Closing Balances**:
  - Aggregate trial balance / оборотно-сальдовая ведомость (`AccountingBalanceReadPort::read_trial_balance`).
  - Account-level closing balance queries (`AccountingBalanceReadPort::read_account_balance`).
- **Standard Chart of Accounts & Posting Rules**:
  - Standard accounts catalog (`standard_accounts::*`).
  - Account type semantics (`AccountType::Asset`, `Liability`, `Equity`, `Revenue`, `Expense`).
  - Rule-based posting generator (`PostingRule`).

## Domain Collaborations
- **Core Platform**: Integrates with `rustok-core` module system and `rustok-api` port context, call policies, and errors.
- **Outbox**: Emits audit and ledger events (`ACCOUNTING_EVENT_ACCOUNT_CREATED`, `ACCOUNTING_EVENT_ACCOUNT_UPDATED`, `ACCOUNTING_EVENT_TRANSACTION_POSTED`) through `rustok-outbox`.
- **Commerce & Marketplace Modules**: Downstream financial modules (Billing, Marketplace Payouts, Wallet, Orders) depend on accounting ports to register balanced financial movements without direct table writes.

## Primary Public Types and Signatures
- `pub struct AccountingModule`
- `pub trait AccountingJournalPort`
- `pub trait AccountingBalanceReadPort`
- `pub struct AccountDefinition`
- `pub enum AccountType`
- `pub enum PostingDirection`
- `pub struct PostingRule`
- `pub struct CreateAccountInput`
- `pub struct PostJournalTransactionInput`
- `pub struct JournalPostingItem`
- `pub struct JournalTransactionResponse`
- `pub struct TrialBalanceQuery`
- `pub struct TrialBalanceResponse`
- `pub enum AccountingError`

## Dependencies on Other RusToK Crates
- `rustok-core`
- `rustok-api`
- `rustok-outbox`

## Common AI Mistakes
- Using floating-point types (`f32`, `f64`) for money amounts — accounting math strictly requires integer minor units (`i64`) and integer balance verification.
- Allowing single-legged or unbalanced transactions — debits MUST equal credits per currency.
- Mutating or deleting posted journal entries directly instead of posting reverse storno entries.
- Missing tenant scoping or passing nil tenant UUIDs in port contexts.

## Minimum Contract Set

### Input DTOs/Commands
- `CreateAccountInput` (`code`, `name`, `account_type`, `currency`, `subconto_types`, `parent_code`)
- `PostJournalTransactionInput` (`reference_id`, `source_module`, `description`, `postings`)
- `JournalPostingItem` (`account_code`, `direction`, `amount`, `currency`, `subconto`)
- `TrialBalanceQuery` (`period_from`, `period_to`, `account_codes`, `subconto_filter`)

### Domain Invariants
- Zero balance invariant: Every posted journal transaction must be balanced (`sum(debits) == sum(credits)`) within each currency present in the transaction.
- Strictly positive amounts: Every posting item must have an amount greater than zero (`amount > 0`).
- Immutability: Posted transactions cannot be updated or deleted; reversals must be performed via `post_storno_reversal`.
- Valid currency codes: Non-empty ISO 4217 standard currency code (3-letter uppercase).
- Integer precision: No floating point arithmetic (`#![deny(clippy::float_arithmetic)]`).

### Events / Outbox Side Effects
- Successful account creation schedules `AccountCreatedEvent` via `rustok-outbox`.
- Successful account update schedules `AccountUpdatedEvent` via `rustok-outbox`.
- Successful journal posting schedules `JournalTransactionPostedEvent` via `rustok-outbox`.

### Errors / Failure Codes
- `AccountingError::UnbalancedTransaction` — debits and credits do not balance.
- `AccountingError::InvalidAccountCode` — account code format or length is invalid.
- `AccountingError::AccountNotFound` — specified account does not exist in Chart of Accounts.
- `AccountingError::CurrencyMismatch` — currency does not match account specification.
- `AccountingError::ImmutableTransactionViolation` — attempt to mutate posted entry.
- `AccountingError::Validation` — input constraint violation.
