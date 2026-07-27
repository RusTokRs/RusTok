# Fulfillment checkout tenant and causation context

Status: **source-ready / unvalidated**

## Scope

This source slice closes the delegated-context diagnostic gap for the two context validations
performed by `CheckoutFulfillmentExecutionPort` in
`crates/rustok-fulfillment/src/checkout_execution.rs`:

- tenant UUID parsing from `PortContext.tenant_id`;
- checkout operation identity matching against `PortContext.causation_id`.

Both validations already returned stable typed `PortError` values and emitted partial warnings.
The previous diagnostics retained correlation, tenant, channel, and operation, but omitted the
rest of the delegated context, the explicit validation phase, truthful owner and boundary, and
the mapped public error evidence.

This slice changes only those two context-validation paths.

## Delivered source contract

### Tenant UUID validation

`parse_tenant_id` still parses the same `PortContext.tenant_id` value and returns the same
validation envelope:

- code `fulfillment.tenant_id_invalid`;
- message `PortContext.tenant_id must be a UUID for fulfillment ports`;
- validation kind;
- non-retryable outcome.

On parse failure, the function now constructs that error before diagnostics, records the
original UUID parse cause, and returns the same constructed error unchanged.

### Checkout causation validation

`require_operation_context` still accepts only a parseable causation UUID equal to the request
checkout operation id. A missing, malformed, or mismatched causation value returns the same
validation envelope:

- code `fulfillment.checkout_operation_id_invalid`;
- message `checkout fulfillment causation_id must match the checkout operation`;
- validation kind;
- non-retryable outcome.

The function now constructs that error before diagnostics and returns the same error unchanged.

## Diagnostic context

Both rejection paths use warning severity because they represent invalid caller or delegated
context rather than an owner infrastructure failure.

Each event records:

- truthful owner `rustok_fulfillment`;
- exact owner operation `ensure_checkout_fulfillments` or `read_checkout_fulfillments`;
- validation phase `tenant_id` or `causation_id`;
- boundary `checkout_fulfillment_execution_port`;
- correlation id and tenant id;
- typed actor, channel, and locale;
- causation id and traceparent when available;
- idempotency key and deadline when available;
- stable mapped code and message;
- typed error kind and retryability;
- the mapped `PortError`.

Tenant rejection also records the original UUID parse cause. Causation rejection records the
expected checkout operation id while the raw delegated causation value remains available only
inside structured diagnostics.

## Preserved behavior

This slice does not change:

- port method signatures or request/response DTOs;
- read/write admission policy or write-semantics requirements;
- admission-before-tenant-before-causation ordering;
- tenant or causation acceptance rules;
- public validation codes, messages, kinds, or retryability;
- request and immutable fulfillment-plan validation;
- fulfillment create, adoption, lookup, or read behavior;
- metadata construction;
- `FulfillmentError` mappings;
- FBA, FFA, or ecommerce audit status.

No parse cause, raw causation value, expected operation identity, or delegated context field is
copied into a new public envelope.

## Static evidence

`scripts/verify/verify-fulfillment-checkout-context-validation.mjs` guards:

- unchanged tenant and causation acceptance rules;
- stable error construction before diagnostics;
- original UUID parse-cause retention;
- exact owner operation and validation phase;
- complete available delegated context;
- truthful owner and fulfillment execution boundary;
- diagnostics before returning the same mapped error;
- absence of the superseded partial validation shapes;
- unchanged public operation routing, admission, request validation, fulfillment validation,
  service operations, and stable owner mappings.

The preceding admission verifier is synchronized only for the two additional owner/boundary
diagnostic branches. Its admission assertions are unchanged. The existing fulfillment
execution error-safety verifier remains compatible with the stable codes and operation fields.

## Remaining gaps

The master ecommerce correlation-safe mapper task remains open for:

- fulfillment request, set, identity, and immutable-plan validation diagnostics;
- order settlement and compensation owner admission and validation;
- inventory reservation owner admission and validation;
- remaining payment execution and compensation consumers;
- GraphQL query customer reads and the shared storefront customer lookup;
- remaining customer, tax, promotion, ecommerce, and non-`PortError` envelopes;
- compile, runtime, replay, restart, remote-port, and cross-transport evidence.

No architecture status is promoted from source inspection alone.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-fulfillment-checkout-context-validation.mjs
node scripts/verify/verify-fulfillment-checkout-admission-context.mjs
node scripts/verify/verify-fulfillment-checkout-execution-error-safety.mjs
node scripts/verify/verify-fulfillment-checkout-lifecycle-error-safety.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-fulfillment --lib
```
