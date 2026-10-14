# Implementation Plan for `rustok-accounting`

## Scope

`rustok-accounting` owns universal double-entry bookkeeping, Chart of Accounts, balanced journal transactions, posting rules, and relational subconto analytics across RusToK.

## Current state

- Status: `in_progress`
- FFA status: `not_started`
- FBA status: `boundary_ready`
- Structural shape: `no_ui_boundary`
- Runtime kind: Core

## Release and Data Rollback Readiness

- Runtime kind: Core
- Rollback unit: Component
- Data boundary owner: `accounting_accounts`, `accounting_journal_transactions`, `accounting_journal_postings`, `accounting_posting_rules`
- Native migrations: none
- Supported migration policy: AdditiveOnly
- Predecessor standby strategy: Standby DB + Slot
- Rollback eligibility: AutomaticSingleAttempt
- Recovery invariants: Balanced debits and credits ($\sum \text{Debit} = \sum \text{Credit}$), immutable storno audit trail, monotonic transaction sequence

## Milestones

1. **FBA Core & Ports Contract**: Domain types, Account entities, balanced Journal transaction invariants, `AccountingJournalPort`, and `AccountingBalanceReadPort`.
2. **Posting Rules Engine**: Declarative posting rules mapping operational document facts (invoices, waybills, cash orders, marketplace payouts) to debits and credits.
3. **Marketplace Balance Bridge**: FBA port adapter connecting `rustok-marketplace-balance` seller balance transitions to statutory G/L postings.
4. **Trial Balance & Relational Subconto**: Fast indexed trial balance (Оборотно-сальдовая ведомость) aggregation over indexed foreign keys.
5. **Leptos Admin Interface**: Chart of Accounts management, journal inspection, and trial balance reports.

## Invariants

- Balance equation: In every transaction, total debits equal total credits per currency.
- Append-only storno: Reversals create new inverse balancing entries rather than modifying past records.
- Tenant isolation: Every account, transaction, and posting requires a non-nil `tenant_id`.

## Verification

- `cargo check -p rustok-accounting`
- `cargo test -p rustok-accounting`
- `cargo xtask module validate accounting`
- `npm run verify:docs`

## Change rules

Changes must strictly preserve the zero-sum invariant ($\sum \text{Debit} = \sum \text{Credit}$), append-only storno semantics, and comply with `docs/backend/module-backend-implementation.md`.
