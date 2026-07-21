# rustok-marketplace-ledger

## Purpose

`rustok-marketplace-ledger` owns immutable, balanced accounting entries produced
from marketplace commission assessments and append-only financial corrections.

## Boundary

- Commission state is consumed only through `MarketplaceCommissionReadPort`.
- The module imports no commission entities and declares no cross-module foreign keys.
- Completed receipt replay happens before commission provider reads.
- One posted ledger transaction is allowed per tenant and commission order batch.
- Refund and chargeback corrections are new balanced transactions; posted transactions and
  entries are never updated or deleted.
- Reversal links are owner-local typed rows, not JSON metadata used as financial truth.
- Seller balances are derived projections. Ledger entries remain authoritative and can rebuild
  every balance row.

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

The command validates the tenant, order, currency, assessment, allocation, order-line, and seller
identity against the original immutable entries. Cumulative reversals cannot exceed an original
entry. A provider or workflow source ID can post only one reversal of the same kind.

The ledger owner does not decide refund allocation or which seller bucket should absorb a refund.
That policy belongs to the cross-domain financial orchestrator, which must supply normalized,
owner-verified facts.

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

`rebuild_seller_balance`:

- recomputes one tenant/seller/currency projection from immutable seller-payable entries;
- treats original unclassified seller-payable entries as `pending`;
- uses typed reversal-line bucket classification for correction entries;
- derives `negative_amount` from pending + available + reserved balances;
- never changes ledger transactions or entries.

## Read projections

- read one order commission ledger transaction with all entries;
- list bounded seller-payable entries by seller and optional currency;
- read one materialized seller balance projection;
- expose pending, available, reserved, paid, and negative amounts.

Payout scheduling consumes ledger entries but remains a separate owner module. Payout settlement,
payout reversal, reserve hold/release, and automatic refund/chargeback event orchestration remain
separate follow-up slices.
