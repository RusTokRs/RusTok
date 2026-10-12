# rustok-accounting

## Purpose

`rustok-accounting` is the canonical platform owner of universal double-entry bookkeeping, Chart of Accounts, balanced journal transactions, posting rules, and relational subconto analytics across RusToK.

## Responsibilities

- Own the tenant-scoped Chart of Accounts (`accounts`).
- Enforce the fundamental accounting equation and zero-sum invariant: $\sum \text{Debit} = \sum \text{Credit}$ on every transaction.
- Maintain immutable, append-only journal entries (`gl_transactions`, `gl_postings`).
- Provide reverse storno operations for correction without mutating posted records.
- Evaluate declarative Posting Rules mapping operational business facts to debit/credit lines.
- Expose typed FBA ports for cross-module posting (`AccountingJournalPort`) and balance inspection (`AccountingBalanceReadPort`).
- Maintain indexed relational subconto dimensions (`party_id`, `contract_id`, `item_id`, `warehouse_id`) for high-speed trial balance (ОСВ) calculation.

## Entry Points

- `AccountingModule`: The module registration and lifecycle entrypoint implementing `RusToKModule`.
- `AccountingJournalPort`: In-process port for posting balanced transactions and storno corrections.
- `AccountingBalanceReadPort`: In-process port for reading account balances, turnovers, and trial balances.

## Interactions

- **`rustok-outbox`:** Consumes domain events from operations (sales, payments, warehouse shipments, marketplace payouts) to execute posting rules.
- **`rustok-party`:** Validates and indexes counterparty subconto references (`party_id`).
- **`rustok-marketplace-balance`:** Receives translated order settlement, commission revenue, and payout clearing entries from marketplace operations.

## Invariants

1. **Double-Entry Balance:** Every journal transaction must satisfy $\sum \text{Debit} = \sum \text{Credit}$. Unbalanced transactions are rejected.
2. **Immutability:** Posted transactions and postings are append-only. Corrections require explicit storno reversal transactions.
3. **Tenant Isolation:** All accounts, transactions, and postings are strictly scoped by `tenant_id`.
4. **Minor Unit Money:** All amounts are integers in currency minor units; floating-point money is forbidden.
