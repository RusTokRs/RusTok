# rustok-marketplace-payout

## Purpose

`rustok-marketplace-payout` owns seller payout scheduling and the exclusive
assignment of seller-payable ledger entries to payout batches.

## Boundary

- Ledger state is consumed only through `MarketplaceLedgerReadPort`.
- The module imports no ledger entities and declares no cross-module foreign keys.
- Completed receipt replay occurs before ledger provider reads.
- A ledger entry can belong to at most one payout.
- This slice schedules payouts only. It does not call a bank, payment processor,
  or transfer provider and does not mark funds as paid.

## Scheduling

The command receives an explicit set of seller-payable ledger entry IDs. Every
selected entry must:

- belong to the request tenant and seller;
- use the `seller_payable` account;
- be a credit;
- use the payout currency;
- have a positive minor-unit amount.

The payout header, item assignments, total amount, and completed command receipt
are committed atomically. A second idempotency key cannot claim entries already
assigned to another payout.

## Read projections

- read one payout with its ledger entry items;
- list bounded payouts by seller, optional currency, and optional status.

## Follow-on lifecycle

Provider submission, processing, paid, failed, cancellation, retries, transfer
references, and ledger settlement entries remain separate follow-on slices.
