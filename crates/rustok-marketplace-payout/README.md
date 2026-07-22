# rustok-marketplace-payout

## Purpose

`rustok-marketplace-payout` owns seller payout admission, scheduling, durable
reservation orchestration state, and the exclusive assignment of seller-payable
ledger capacity to payout batches.

## Boundary

- Ledger state is consumed and changed only through typed marketplace-ledger ports.
- The module imports no ledger entities and declares no cross-module foreign keys.
- Completed command replay must occur before ledger provider reads or writes.
- A ledger entry can belong to at most one payout assignment.
- External bank/processor execution is a later provider slice and must use its own
  operation journal and verified webhook inbox.

## Canonical persistence

The original payout migration generated legacy table names (`payouts`,
`payout_items`, and `payout_receipts`) while SeaORM entities used canonical
`marketplace_*` names. The operation-journal migration repairs those names before
creating new persistence and records a marker so normal down/reapply paths restore
the historical chain correctly.

Canonical owner tables are:

- `marketplace_payouts`;
- `marketplace_payout_items`;
- `marketplace_payout_receipts`;
- `marketplace_payout_operations`;
- `marketplace_payout_operation_transfers`.

## Current scheduling

The existing scheduling command receives an explicit set of seller-payable ledger
entry IDs. Every selected entry must:

- belong to the request tenant and seller;
- use the `seller_payable` account;
- be a credit;
- use the payout currency;
- have a positive minor-unit amount.

The current payout header, item assignments, total amount, and completed command
receipt are committed atomically. A second idempotency key cannot claim entries
already assigned to another payout.

## Durable reservation operation

The operation journal persists:

- actor-bound operation identity and canonical request hash;
- immutable request snapshot;
- typed operation status and stage;
- lease, revision, attempts, and safe error code;
- one deterministic reserve/release child operation per source order;
- ledger transfer and transaction identities after confirmed posting.

Marketplace ledger balance transfers intentionally require all reference entries in
one command to belong to one order. A multi-order payout must therefore group
selected entries by order, execute deterministic `reserve_hold` children, and
release confirmed children in reverse order when scheduling cannot complete.

This source slice creates the durable schema only. It does not yet call
`MarketplaceLedgerCommandPort`, reserve funds, or execute an external payout.

## Read projections

- read one payout with its ledger entry items;
- list bounded payouts by seller, optional currency, and optional status.

## Follow-on lifecycle

1. admit/replay the durable payout operation;
2. reserve each per-order seller balance group through ledger `reserve_hold`;
3. create the payout only from confirmed transfer responses;
4. compensate with exact `reserve_release` children on failure/cancellation;
5. add provider account, submission journal, lookup recovery, webhook inbox;
6. post `payout_settlement` after confirmed provider success;
7. post `payout_reversal` after confirmed external reversal;
8. retain PostgreSQL contention, restart, reconciliation, and mounted transport evidence.
