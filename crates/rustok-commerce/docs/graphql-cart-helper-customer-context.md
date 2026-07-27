# GraphQL cart-helper customer context

Status: **source-ready / unvalidated**

## Scope

This source slice closes the retained-context and diagnostic-severity gap for the
optional customer projection read in
`crates/rustok-commerce/src/graphql/mutations/safe_helpers.rs`:

- `resolve_optional_storefront_customer_id`;
- `CustomerReadPort::read_customer_projection_by_user`;
- the customer-specific `PortError -> async_graphql::Error` mapper used by that
  helper.

The preceding GraphQL cart-helper hardening established stable public customer,
cart, pricing, and intercepted legacy-helper envelopes. Before this follow-up, the
customer path created one `PortContext`, cloned it into a separate `error_context`,
and moved the original into the owner call. Its mapper recorded correlation and
tenant identity but omitted the rest of the delegated context, did not identify the
exact owner operation separately from the consumer helper operation, and used error
severity for every owner rejection.

GraphQL query resolvers, the shared HTTP storefront helper in
`controllers/store/mod.rs`, order/shipping helpers, cart/pricing mapper context, and
legacy helper internals remain outside this slice.

## Delivered source contract

The optional customer helper now:

1. creates one `customer_context` with the existing context constructor;
2. clones that context into
   `CustomerReadPort::read_customer_projection_by_user`;
3. retains the original context for the GraphQL boundary mapper;
4. preserves the existing optional-customer not-found behavior;
5. passes the existing consumer operation
   `resolve_optional_storefront_customer_id` to diagnostics.

The customer mapper attributes failures to:

- truthful owner `rustok_customer`;
- exact owner operation `read_customer_projection_by_user`;
- consumer operation `resolve_optional_storefront_customer_id`;
- boundary `commerce_graphql_storefront_cart_helper`.

Diagnostics retain:

- correlation id and tenant id;
- typed actor, channel, and locale;
- causation id and traceparent when available;
- idempotency key and deadline when available;
- original owner code, internal message, typed kind, and retryability;
- the already selected public GraphQL code and retryability.

Unavailable, timeout, and invariant failures use error severity. Validation,
not-found, conflict, and forbidden owner rejections use warning severity.

## Preserved behavior

This slice does not change:

- the external signature of `resolve_optional_storefront_customer_id`;
- anonymous-authentication behavior (`None -> Ok(None)`);
- `customer.customer_by_user_not_found -> Ok(None)` behavior;
- the customer projection request or authenticated user identity;
- the existing customer context correlation format, actor, fallback locale, or
  two-second deadline;
- public customer GraphQL messages, codes, or retryability:
  - `CUSTOMER_REQUEST_INVALID`;
  - `CUSTOMER_NOT_FOUND`;
  - `CUSTOMER_STATE_CONFLICT`;
  - `CUSTOMER_ACCESS_DENIED`;
  - `CUSTOMER_TEMPORARILY_UNAVAILABLE`;
  - `CUSTOMER_OPERATION_FAILED`;
- the shared `public_graphql_error` extension shape;
- cart/pricing source-owner classification and cart public envelopes;
- intercepted shipping, line-item, inventory, enrichment, and repricing helper
  envelopes;
- private module routing and the crate-private legacy-helper facade;
- FBA, FFA, or ecommerce audit status.

Raw owner messages remain internal to structured diagnostics and are not copied into
the public GraphQL envelope.

## Static evidence

The existing
`scripts/verify/verify-commerce-graphql-cart-helper-error-safety.mjs` guard was
strengthened rather than adding a duplicate verifier. It now guards:

- stable GraphQL boundary plus truthful customer owner and exact owner operation;
- one retained customer context and one delegation clone;
- operation-aware customer mapper use;
- complete available `PortContext` and original owner error fields;
- technical versus ordinary rejection severity;
- unchanged customer message/code/retryability policy before diagnostics and return;
- unchanged anonymous and customer-not-found behavior;
- unchanged cart/pricing and intercepted legacy-helper contracts;
- absence of the old `port_context` / `error_context` split and owner-free log form.

The verifier was also synchronized with the current source by replacing three stale
assertions that contradicted the facade:

- the crate-private legacy re-export is now required rather than forbidden;
- the operation list no longer requires a helper that is not implemented in this
  facade;
- the intercepted legacy-call count now reflects the five actual calls.

These verifier corrections do not change production behavior.

## Remaining gaps

The master ecommerce correlation-safe mapper task remains open for:

- optional and authenticated customer reads in `graphql/query.rs`;
- the shared storefront customer lookup in `controllers/store/mod.rs`;
- remaining payment execution and compensation consumers;
- remaining order, fulfillment, inventory, customer, tax, promotion, and ecommerce
  adapters;
- remaining cart/pricing and non-`PortError` public envelopes;
- compile, runtime, replay, restart, remote-port, and cross-transport evidence.

No architecture status is promoted from source-only evidence.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-commerce-graphql-cart-helper-error-safety.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-commerce --lib
```
