# rustok-marketplace-ledger

## Purpose

`rustok-marketplace-ledger` owns immutable, balanced accounting entries produced
from marketplace commission assessments, append-only financial corrections, and
seller balance bucket transfers.

## Boundary

- Commission state is consumed only through `MarketplaceCommissionReadPort`.
- The module imports no commission, payout, order, or payment entities and declares no
  cross-module foreign keys.
- Completed receipt replay happens before commission provider reads.
- One posted ledger transaction is allowed per tenant and commission order batch.
- Refund and chargeback corrections are new balanced transactions; posted transactions and
  entries are never updated or deleted.
- Reversal links and balance-transfer lineage are owner-local typed rows, not JSON metadata used
  as financial truth.
- Seller balances are derived projections. Ledger entries remain authoritative and can rebuild
  every balance row.
- Payout execution and provider lifecycle remain in `rustok-marketplace-payout`; the ledger owner
  only posts typed, idempotent accounting effects requested through its FBA port.

## Initial posting model

For each assessed marketplace order line:

1. debit `marketplace_clearing` for the allocation total;
2. credit `platform_commission_revenue` for commission;
3. credit `seller_payable` for seller proceeds.

Unclassified initial `seller_payable` credits enter the `pending` balance bucket.
The order transaction is committed only when total debits equal total credits.
All amounts use signed 64-bit currency minor units and checked arithmetic.

## Reversal model

`post_financial_reversal` supports `refund` and `chargeback` sources.
For each reversed assessment line it:

1. debits `platform_commission_revenue` by the reversed commission amount;
2. debits `seller_payable` by the reversed seller amount in the caller-selected balance bucket;
3. credits `marketplace_clearing` by the combined reversal amount.

The command validates tenant, order, currency, assessment, allocation, order-line, and seller
identity against original immutable entries. Cumulative reversals cannot exceed an original entry.
A provider or workflow source ID can post only one reversal of the same kind.

The ledger owner does not decide refund allocation or which seller bucket should absorb a refund.
That policy belongs to cross-domain financial orchestration, which supplies normalized,
owner-verified facts.

## Seller balance transfer model

`post_seller_balance_transfer` supports exactly these typed transitions:

| Kind | From | To |
| --- | --- | --- |
| `pending_release` | `pending` | `available` |
| `reserve_hold` | `available` | `reserved` |
| `reserve_release` | `reserved` | `available` |
| `payout_settlement` | `reserved` | `paid` |
| `payout_reversal` | `paid` | `available` |

Each line references an immutable positive `seller_payable` credit and posts two new entries:

1. debit `seller_payable` in the source bucket;
2. credit `seller_payable` in the destination bucket.

The owner validates seller, currency, source bucket, order scope, event time, and cumulative
reference-entry capacity. A reference credit can be transferred only up to its original amount.
The destination credit becomes the typed reference for a later bucket transition. Aggregate bucket
capacity and reference lineage are calculated from immutable entries under owner-local locks rather
than from the materialized balance projection.

Because the existing ledger transaction requires a real `order_id`, one owner command accepts
reference entries from one order only. Multi-order payout batches must be decomposed by payout or
root orchestration into deterministic per-order ledger commands; the ledger never fabricates an
order identity.

Transfer headers, line links, and entry bucket classifications are append-only at the database
level on PostgreSQL, SQLite, and MySQL.

## Commands

`post_order_commissions`:

- replays a completed receipt before provider access;
- loads immutable commission assessments for the order;
- requires every assessment to be in `assessed` status;
- requires one currency across the order batch;
- validates `commission + seller proceeds = allocation total`;
- inserts the transaction, entries, and completed receipt atomically;
- rebuilds affected seller balance projections after a successful posting or receipt replay.

`post_financial_reversal`:

- requires a stable idempotency key and canonical request hash;
- supports append-only refund and chargeback corrections;
- links every correction entry to the exact original entry;
- enforces cumulative reversal capacity;
- inserts the balanced transaction, reversal facts, lines, and receipt atomically;
- rebuilds affected seller balance projections after posting or replay.

`post_seller_balance_transfer`:

- requires a stable actor-bound idempotency key and canonical request hash;
- enforces one transfer per tenant, transfer kind, and source ID;
- locks seller-payable entries and performs a fresh locking reread after concurrent waits;
- validates aggregate source-bucket capacity and cumulative per-reference capacity;
- inserts balanced debit/credit entries, typed bucket classification, transfer facts, lines, and
  completed receipt atomically;
- rebuilds the affected seller balance projection after posting or receipt replay.

`rebuild_seller_balance`:

- recomputes one tenant/seller/currency projection from immutable seller-payable entries;
- gives explicit entry bucket classification precedence over reversal-line fallback;
- treats original unclassified seller-payable entries as `pending`;
- derives `negative_amount` from pending + available + reserved balances;
- never changes ledger transactions or entries.

## Read projections

- read one order commission ledger transaction with all entries;
- list bounded seller-payable entries by seller and optional currency;
- read one materialized seller balance projection;
- expose pending, available, reserved, paid, and negative amounts.

## Remaining production lifecycle

- payout provider operation journal and verified webhook inbox;
- deterministic multi-order payout settlement orchestration over per-order transfer commands;
- external ledger event contracts and transactional outbox publication;
- authenticated accounting and seller balance transfer operator surfaces;
- clean/upgraded migration, replay, contention, restart, and provider evidence.
