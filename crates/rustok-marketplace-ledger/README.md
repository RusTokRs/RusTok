# rustok-marketplace-ledger

## Purpose

`rustok-marketplace-ledger` owns immutable, balanced accounting entries produced
from marketplace commission assessments.

## Boundary

- Commission state is consumed only through `MarketplaceCommissionReadPort`.
- The module imports no commission entities and declares no cross-module foreign keys.
- Completed receipt replay happens before commission provider reads.
- One posted ledger transaction is allowed per tenant and commission order batch.
- Each commission assessment can contribute only one entry per ledger account.
- Ledger rows are append-only. Corrections are represented by future reversing
  transactions, never by mutation of posted entries.

## Posting model

For each assessed marketplace order line:

1. debit `marketplace_clearing` for the allocation total;
2. credit `platform_commission_revenue` for commission;
3. credit `seller_payable` for seller proceeds.

The order transaction is committed only when total debits equal total credits.
All amounts use signed 64-bit currency minor units and checked arithmetic.

## Commands

`post_order_commissions`:

- replays a completed receipt before provider access;
- loads immutable commission assessments for the order;
- requires every assessment to be in `assessed` status;
- requires one currency across the order batch;
- validates `commission + seller proceeds = allocation total`;
- inserts the transaction, entries, and completed receipt atomically.

## Read projections

- read one order ledger transaction with all entries;
- list bounded seller-payable entries by seller and optional currency.

Payout scheduling consumes seller-payable ledger entries but remains a separate
owner module.
