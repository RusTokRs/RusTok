# Marketplace Payout implementation plan

Last reviewed: 2026-07-22

## Ownership

`rustok-marketplace-payout` owns seller payout admission, scheduling, durable
reservation orchestration state, payout/provider execution state, and payout read
projections. It does not own seller balances or ledger entries.

Ledger movement remains owned by `rustok-marketplace-ledger` and is invoked only
through typed ports. Bank, processor, or transfer-provider facts must be normalized
through a payout provider SPI and journal before changing payout state.

## Current source state

- [x] Payout header, item assignment, and schedule command receipt source.
- [x] Typed read and command ports for the existing scheduling slice.
- [x] Exclusive `(tenant_id, ledger_entry_id)` payout assignment.
- [x] Canonical `marketplace_*` entity names.
- [x] Upgrade repair for the original legacy migration table names.
- [x] Durable payout operation and per-order transfer journal schema.
- [x] Typed operation/transfer status and stage entities.
- [x] Tenant-scoped internal foreign keys and database checks.
- [x] Operation/transfer revision, lease, attempt, and safe error-code fields.
- [ ] Runtime admission/replay repository for payout operations.
- [ ] CAS lease acquisition and expired-lease recovery.
- [ ] Per-order `reserve_hold` execution through `MarketplaceLedgerCommandPort`.
- [ ] Reverse-order `reserve_release` compensation.
- [ ] Create payout only from confirmed ledger transfer responses.
- [ ] Cancellation and reservation release workflow.
- [ ] Payout provider accounts and destination ownership.
- [ ] Provider operation journal and idempotent submission.
- [ ] Provider lookup recovery and verified webhook inbox.
- [ ] `payout_settlement` and `payout_reversal` ledger posting.
- [ ] Accounting, operator, and seller surfaces.
- [ ] Live embedded/remote FBA evidence.

## Schema contract

### `marketplace_payout_operations`

Stores one actor-bound durable scheduling operation:

- tenant, actor, seller, and currency identity;
- idempotency key and canonical request hash;
- immutable request JSON needed for restart;
- typed status and stage;
- optional resulting payout ID;
- attempt count and optimistic revision;
- lease owner/expiry;
- safe error code only;
- completion timestamps.

### `marketplace_payout_operation_transfers`

Stores deterministic ledger child operations grouped by source order:

- parent payout operation;
- stable sequence and order identity;
- `reserve_hold` or `reserve_release` kind;
- idempotency key, request hash, and request snapshot;
- total minor-unit amount;
- ledger transfer and transaction IDs after confirmation;
- attempt, revision, safe error code, and completion timestamps.

A ledger balance-transfer command intentionally accepts references from one order.
Multi-order payouts therefore create multiple child rows. Compensation executes
confirmed reserve rows in descending sequence order.

## Migration contract

The original migration generated `payouts`, `payout_items`, and `payout_receipts`
while entities used `marketplace_*` names. The operation migration:

1. detects a complete legacy or complete canonical table set;
2. rejects mixed states;
3. renames the complete legacy set to canonical names;
4. records a repair marker only when it performed the rename;
5. creates operation persistence;
6. restores legacy names on down only when the marker exists.

Required evidence:

- [ ] clean SQLite apply/down/reapply;
- [ ] upgraded SQLite apply;
- [ ] clean PostgreSQL apply/down/reapply;
- [ ] upgraded PostgreSQL apply under existing rows;
- [ ] clean MySQL apply/down/reapply;
- [ ] mixed legacy/canonical state fails closed;
- [ ] existing payout data survives rename;
- [ ] tenant composite foreign keys reject cross-tenant links.

## Execution order

1. [x] Repair canonical table names and add operation schema.
2. [ ] Add operation admission and completed-result replay.
3. [ ] Resolve selected ledger entries and group by order.
4. [ ] Persist deterministic reserve children.
5. [ ] Execute/adopt `reserve_hold` children.
6. [ ] Create payout/items from confirmed reserve responses.
7. [ ] Complete operation and expose recovery reads.
8. [ ] Add reverse-order `reserve_release` compensation.
9. [ ] Add cancellation.
10. [ ] Add provider accounts and provider operation journal.
11. [ ] Add transfer submission, lookup recovery, webhook inbox.
12. [ ] Add settlement/reversal ledger transfers.
13. [ ] Add operator/seller transports and UI.
14. [ ] Retain contention, restart, reconciliation, and remote-profile evidence.

## Promotion gate

The module remains `in_progress`. Schema or source inspection alone must not promote
FBA status. Promotion requires compiled contracts, clean/upgraded database evidence,
concurrent reservation proof, process restart recovery, provider redelivery, ledger
reconciliation, mounted transports, and operator workflows.

## Implemented reservation slice

- durable operation admission and request-hash replay
- lease/revision claim before side effects
- deterministic per-order `reserve_hold` children
- persisted ledger request and response payloads for crash recovery
- existing atomic payout receipt transaction reused after reservation
- reverse-order `reserve_release` compensation based on Reserved credit entries
- retryable compensation and operator reconciliation states

External provider submission and Reserved-to-Paid settlement remain separate follow-up slices.
