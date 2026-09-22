# Storefront GraphQL reprice typed error envelope

Status: source-ready, unvalidated.

## Scope

This slice covers the mounted GraphQL helper
`reprice_storefront_cart_line_items` used by storefront cart mutations.

Before this cutover the mounted path delegated through the private compatibility
helper in `helpers.rs`. That helper converted both pricing and cart `PortError`
values into `async_graphql::Error` using `PortError.message`. The safe facade then
converted that intermediate GraphQL error a second time into the stable public
`CART_REPRICE_FAILED` envelope.

The mounted path now delegates directly to typed pricing and cart owner results.
The former implementation remains private compatibility source and is not the
symbol exported by `graphql::mutations::helpers`.

## Preserved behavior

The following behavior is unchanged:

- carts without line items return unchanged;
- line items without a variant id are skipped;
- the existing storefront pricing context builder is used;
- the existing contextual pricing read port is used;
- one price resolution is requested for each eligible line item;
- the existing cart pricing update builder is used;
- no cart owner call is made when no updates were produced;
- the existing storefront cart `PortContext` builder is used;
- the existing `CartStorefrontRepriceRequest` is delegated unchanged;
- successful owner responses are returned unchanged.

## Public GraphQL contract

Every pricing or cart owner failure still produces exactly:

- message: `Cart pricing could not be refreshed`;
- code: `CART_REPRICE_FAILED`;
- retryable: `true`.

The transport no longer uses an owner message as an intermediate GraphQL
protocol. The owner `PortError.message` is not copied into the public envelope or
into dedicated transport diagnostic fields.

## Typed failure sources

The mounted helper distinguishes two sources:

| Source | Owner | Owner operation |
| --- | --- | --- |
| Per-line price resolution | `rustok_pricing` | `resolve_product_price` |
| Cart pricing update | `rustok_cart` | `reprice_storefront_line_items` |

The original owner code, kind, and retryability are retained for internal
classification. `Unavailable`, `Timeout`, and `InvariantViolation` are recorded
at error severity. Validation, not-found, conflict, and forbidden outcomes are
recorded at warning severity.

## Retained context

The transport diagnostic event retains:

- correlation id;
- tenant id;
- actor kind and actor id;
- delegated channel length;
- delegated locale length;
- causation-id presence;
- traceparent presence;
- idempotency-key presence;
- deadline;
- cart id;
- optional line-item, variant, and product ids;
- optional requested quantity;
- current planned update count;
- cart line-item count;
- currency-code length;
- normalized request-channel-slug length;
- owner code, kind, and retryability;
- stable public code and retryability.

## Excluded payloads

The typed boundary does not add dedicated diagnostic fields for:

- owner error message text;
- raw currency code;
- raw channel slug;
- raw locale;
- resolved price amount;
- compare-at amount;
- discount percentage;
- pricing adjustment amount or metadata;
- cart line-item title or SKU;
- request or response payload dumps.

The contextual owner wrappers remain responsible for owner-local diagnostic
evidence. This transport mapper records only the facts needed to identify the
consumer operation and the affected ecommerce resources.

## Routing

`graphql/mutations/mod.rs` loads `typed_reprice_helper.rs` as a private module.
`layered_order_helpers.rs` explicitly exports its
`reprice_storefront_cart_line_items` symbol after the private compatibility glob.
The cart resolver continues importing the same helper name through
`super::helpers::*`, so its call signature is unchanged.

## Static evidence

The focused source guard is:

```bash
node scripts/verify/verify-commerce-graphql-reprice-typed-error.mjs
```

The guard checks private routing, the two typed owner sources, preserved request
builders, retained `PortContext`, the stable public envelope, and absence of
message-based or raw-payload transport mapping.

## Remaining work

This slice does not:

- replace direct owner composition outside this mounted helper;
- retire `safe_helpers.rs`, `safe_legacy_helpers.rs`, or `helpers.rs`;
- change REST or native storefront repricing;
- establish mounted native/GraphQL parity;
- provide runtime or remote-profile evidence;
- close the broad ecommerce mapper cleanup item;
- promote ecommerce FBA or FFA status.
