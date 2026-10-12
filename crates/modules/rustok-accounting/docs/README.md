# `rustok-accounting` Documentation

`rustok-accounting` is the platform module providing universal double-entry bookkeeping, Chart of Accounts, balanced journal transactions, posting rules, and relational subconto analytics across RusToK.

## Purpose

- publish the canonical double-entry accounting runtime contract for Chart of Accounts and general ledger postings;
- ensure the fundamental accounting equation ($\sum \text{Debit} = \sum \text{Credit}$) is strictly enforced per currency in every transaction;
- maintain immutable, audit-compliant journal entries with append-only storno reversals;
- provide indexed relational subconto analytics over sovereign domain entities (`party_id`, `contract_id`, `item_id`, `warehouse_id`);
- offer high-performance trial balance (Оборотно-сальдовая ведомость) aggregation without dynamic EAV tables.

## Responsibility Zone

- Chart of Accounts (`AccountDefinition`, `AccountType`: Asset, Liability, Equity, Revenue, Expense);
- balanced General Ledger journal entries and postings (`JournalTransaction`, `JournalPostingItem`);
- posting rules engine translating business events and documents into balanced debits and credits;
- storno reversals (red ink / negative or inverse balancing transactions) preserving full historical audit trails;
- trial balance generation and account turnover calculation;
- FBA ports `AccountingJournalPort` and `AccountingBalanceReadPort`.

## Domain Concepts

1. **Account (Счет учета):**
   - Unique code within tenant (e.g. `10.01`, `41.01`, `50.01`, `51.01`, `60.01`, `62.01`, `76.01`, `90.01`, `90.02`).
   - Type: `Asset`, `Liability`, `Equity`, `Revenue`, `Expense`.
   - Subconto dimensions: configuration of enabled analytical dimensions for the account.

2. **Journal Transaction (Хозяйственная операция):**
   - Header with `transaction_number`, `posting_date`, `document_type`, `document_id`, `description`, `is_storno`.
   - Contains a set of balanced `JournalPosting` entries.

3. **Journal Posting (Проводка):**
   - Individual line with `debit_account_code`, `credit_account_code`, `amount_minor`, `currency_code`.
   - Analytical subconto references: `party_id`, `contract_id`, `item_id`, `warehouse_id`.

4. **Posting Rules Engine:**
   - Evaluates incoming document events against tenant posting profiles to generate debit and credit lines deterministically.

## Integration

- integrates with `rustok-marketplace-balance` (seller balance) via FBA port adapter to record escrow accruals, commissions, and payouts into general ledger;
- integrates with `rustok-finance` for bank account and cash register settlements;
- integrates with `rustok-billing` for customer invoices, revenue recognition, and receivable tracking;
- integrates with `rustok-inventory` for inventory movement and COGS calculations;
- integrates with `rustok-outbox` for reliable, transactional event publishing.

## Verification

- `cargo check -p rustok-accounting`
- `cargo test -p rustok-accounting`
- `cargo xtask module validate accounting`
- `npm run verify:docs`

## Related Documentation

- [README crate](../README.md)
- [Implementation Plan](./implementation-plan.md)
- [Architecture Matrix](../../../../docs/architecture/crm-erp-accounting-matrix-and-plan.md)
- [ADR: Atomic CRM/ERP/Accounting Architecture](../../../../DECISIONS/2026-10-12-atomic-crm-erp-accounting-architecture.md)
