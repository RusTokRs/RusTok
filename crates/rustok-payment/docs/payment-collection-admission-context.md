# Payment collection owner admission context

Status: **source-ready / unvalidated**

## Scope

This source slice closes the owner-side structured-context gap for admission
rejections in `PaymentCollectionPort`:

- `create_or_reuse_collection` write-policy admission;
- `create_or_reuse_collection` write-semantics admission;
- `read_collection_status` read-policy admission.

Before this slice, the port called `PortContext::require_policy` and
`PortContext::require_write_semantics` directly with `?`. A rejection therefore
returned the original `PortError`, but the payment owner did not record the
available correlation, actor, channel, locale, deadline, exact owner operation,
or the admission phase that rejected the request.

This slice is deliberately limited to payment collection admission. Tenant-id
parsing, request validation, provider execution, collection lifecycle mapping,
checkout consumers, compensation consumers, and transport adapters remain
separate concerns.

## Delivered source contract

Both public owner operations now select their canonical owner operation before
admission:

- `create_or_reuse_collection` uses `create_or_reuse_collection`;
- `read_collection_status` uses `read_collection_status`.

The write path delegates admission to a payment-owned helper that:

1. requires `PortCallPolicy::write()`;
2. records a rejection with admission phase `policy` before returning the
   original `PortError`;
3. requires write semantics;
4. records a rejection with admission phase `write_semantics` before returning
   the original `PortError`.

The read path delegates admission to a payment-owned helper that requires
`PortCallPolicy::read()` and records phase `policy` before returning the original
`PortError`.

Admission diagnostics record:

- truthful owner `rustok_payment`;
- exact owner operation;
- admission phase;
- correlation id and tenant id;
- actor, channel, and locale;
- causation id and traceparent when available;
- idempotency key and deadline when available;
- original error code, message, typed kind, and retryability;
- boundary `payment_collection_port`.

Unavailable, timeout, and invariant failures use error severity. Other
admission rejections use warning severity.

## Preserved behavior

This slice does not change:

- public `PaymentCollectionPort` trait signatures;
- create/reuse request fields or status request identity;
- status snapshot fields or typed status conversion;
- write policy or write-semantics requirements;
- read policy requirements;
- tenant UUID parsing or its validation code/message;
- reusable collection lookup by cart;
- create-after-read behavior;
- race adoption after a create error;
- granular owner operation labels for existing lookup, race adoption, and
  collection creation errors;
- payment validation, not-found, lifecycle, provider, reconciliation,
  configuration, or database `PortError` codes and public messages;
- provider ids or provider operation diagnostics;
- retryability values;
- checkout payment execution or compensation consumer behavior;
- FBA, FFA, or ecommerce audit status.

Admission helpers return the same original `PortError` produced by
`PortContext`; they only emit structured diagnostics before propagation.

## Static evidence

`scripts/verify/verify-payment-collection-admission-context.mjs` guards:

- stable payment owner and boundary identity;
- canonical read and write owner operations;
- operation selection before admission and tenant parsing;
- read-policy admission without write semantics;
- write-policy admission followed by write-semantics admission;
- distinct `policy` and `write_semantics` diagnostic phases;
- complete available `PortContext` fields;
- original error code, message, typed kind, and retryability;
- technical versus ordinary rejection severity;
- existing create/read/race owner mappings;
- unchanged status request and snapshot mapping;
- stable payment public `PortError` messages;
- absence of the old direct context-dropping admission paths.

## Remaining gaps

The master ecommerce correlation-safe mapper task remains open for:

- payment tenant-context validation diagnostics;
- remaining payment execution and compensation consumers;
- storefront customer read consumers;
- remaining order, fulfillment, inventory, customer, tax, and promotion
  adapters;
- non-`PortError` public envelopes;
- compile, runtime, replay, restart, remote-port, and cross-transport evidence.

No architecture status is promoted from source-only evidence.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-payment-collection-admission-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-payment --lib
```
