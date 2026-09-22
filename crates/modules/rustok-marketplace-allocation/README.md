# rustok-marketplace-allocation

## Purpose

`rustok-marketplace-allocation` owns the immutable assignment of every marketplace
order line to the seller and listing that won selection at checkout.

## Owned data

- one allocation row per tenant and order-line identity;
- seller, listing, master product, and master variant identities;
- the exact listing terms version used by checkout;
- pricing, inventory, and fulfillment references captured at allocation time;
- quantity and monetary amounts in currency minor units;
- durable allocation request receipts and normalized response snapshots.

## Boundary rules

- The module does not query seller, listing, product, pricing, inventory, or order
  tables directly.
- Checkout or order composition supplies the already selected identities and
  commercial snapshot through `MarketplaceAllocationCommandPort`.
- No cross-module database foreign keys are declared.
- A batch either allocates every supplied line and completes its receipt, or rolls
  back completely.
- Replaying the same idempotency key and normalized request returns the saved result.
- The same idempotency key with a different request is rejected.
- An order line cannot be allocated twice, including through another idempotency key.

## Entry points

- `MarketplaceAllocationModule`
- `MarketplaceAllocationService`
- `MarketplaceAllocationReadPort`
- `MarketplaceAllocationCommandPort`
- `dto::*`

## Follow-on capabilities

Commission calculation, ledger posting, payout scheduling, refund attribution, and
seller settlement consume allocation projections but remain separate owners.
