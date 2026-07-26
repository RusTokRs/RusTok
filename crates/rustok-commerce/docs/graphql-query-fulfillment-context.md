# GraphQL query fulfillment owner context

Status: `source_ready_unvalidated`

## Closed gap

The safe commerce GraphQL query facade now intercepts the two direct
`FulfillmentService` reads mounted by the unchanged `query.rs` source:

- `storefront_shipping_options` delegates `list_shipping_options`;
- admin `order` delegates `find_by_order`.

Before this slice, `storefront_shipping_options` preserved the typed error only at the
shared GraphQL query boundary, where tenant, locale inputs, and exact owner operation
were no longer available. The admin order lookup converted the typed fulfillment cause
to a string before the generic fail-closed query envelope, losing the typed variant
entirely.

A private two-method facade now delegates to the canonical
`rustok_fulfillment::FulfillmentService`, records the original `FulfillmentError`, and
rethrows the same typed error. Diagnostics retain:

- truthful owner `rustok_fulfillment`;
- tenant identity;
- GraphQL query field;
- exact owner operation;
- truthful optional order identity;
- requested and tenant-default locale inputs where present;
- stable owner code, kind, and retryability;
- explicit boundary `commerce_graphql_query_fulfillment_facade`.

Database failures use error severity. Validation, missing-resource, and lifecycle
rejections use warning severity.

## Preserved contracts

- `query.rs` remains unchanged and still imports `rustok_fulfillment::FulfillmentService`.
- Both existing `FulfillmentService::new(db.clone())` call sites resolve through the
  private safe facade.
- The facade returns the canonical `FulfillmentResult` values and rethrows the original
  typed error.
- Storefront shipping-option currency, channel-visibility, and shipping-profile
  filtering remain unchanged.
- Admin order payment and fulfillment projection behavior remains unchanged.
- The storefront query keeps the existing `FULFILLMENT_*` message/code/retryability
  policy.
- The admin order lookup keeps its existing generic
  `COMMERCE_QUERY_OPERATION_FAILED` fail-closed envelope after string conversion.
- GraphQL schema, DTOs, owner service signatures, ports, and successful responses remain
  unchanged.

## Still open

- Replace legacy direct owner-service query reads with typed ports carrying complete
  `PortContext` where owner contracts exist.
- Propagate request correlation, actor, channel, causation, and deadline context into
  fulfillment query reads rather than reconstructing only local identities.
- Continue reviewing order, payment, customer, inventory, region, channel, catalog, and
  remaining commerce query conversions that still pass through dynamic strings.
- Add compile and transport evidence before promoting any FBA/FFA status.

## Intended checks

```bash
node scripts/verify/verify-commerce-graphql-query-fulfillment-context.mjs
node scripts/verify/verify-commerce-graphql-query-error-boundary.mjs
node scripts/verify/verify-commerce-storefront-shipping-enrichment-context.mjs
cargo check -p rustok-commerce --lib
```

Tests, Cargo commands, formatting commands, verifiers, workflow checks, and CI were not
run for this source wave; validation remains maintainer-owned.
