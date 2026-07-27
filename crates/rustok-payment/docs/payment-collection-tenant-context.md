# Payment collection tenant validation owner context

Status: **source-ready / unvalidated**

## Scope

This source slice closes the owner-side structured-context gap for invalid tenant
identity in `PaymentCollectionPort`:

- `create_or_reuse_collection` tenant UUID validation;
- `read_collection_status` tenant UUID validation.

Both public operations already selected a canonical owner operation and completed
policy admission before parsing `PortContext.tenant_id`. Before this slice, tenant
parsing returned `payment.tenant_id_invalid` directly from a context-free closure.
The original validation envelope was stable, but the payment owner did not record
the available correlation, actor, channel, locale, deadline, exact owner operation,
or the UUID parse cause.

This slice is deliberately limited to tenant UUID validation. Payment collection
admission diagnostics were closed by the preceding slice. Request validation,
provider execution, collection lifecycle mapping, checkout consumers,
compensation consumers, and transport adapters remain separate concerns.

## Delivered source contract

Both public owner operations now pass their already-selected canonical operation
to the tenant parser:

- `create_or_reuse_collection` passes `create_or_reuse_collection`;
- `read_collection_status` passes `read_collection_status`.

The payment-owned tenant parser now:

1. parses the existing `PortContext.tenant_id` as a UUID;
2. constructs the same validation `PortError` on failure;
3. records the UUID parse cause and complete available owner context;
4. returns that same validation error after diagnostics.

Tenant validation diagnostics record:

- truthful owner `rustok_payment`;
- exact owner operation;
- validation phase `tenant_id`;
- correlation id and tenant id;
- actor, channel, and locale;
- causation id and traceparent when available;
- idempotency key and deadline when available;
- original validation code, message, typed kind, and retryability;
- internal UUID parse cause;
- boundary `payment_collection_port`.

Invalid tenant identity is a caller/context validation rejection and therefore uses
warning severity.

## Preserved behavior

This slice does not change:

- public `PaymentCollectionPort` trait signatures;
- write-policy or write-semantics admission;
- read-policy admission;
- admission ordering before tenant parsing;
- tenant validation code `payment.tenant_id_invalid`;
- tenant validation message
  `PortContext.tenant_id must be a UUID for payment ports`;
- validation kind or retryability;
- reusable collection lookup by cart;
- create-after-read behavior;
- race adoption after a create error;
- granular owner operation labels for existing lookup, race adoption, and
  collection creation errors;
- status collection identity or status snapshot mapping;
- payment validation, not-found, lifecycle, provider, reconciliation,
  configuration, or database `PortError` codes and public messages;
- provider ids or provider operation diagnostics;
- checkout payment execution or compensation consumer behavior;
- FBA, FFA, or ecommerce audit status.

## Static evidence

`scripts/verify/verify-payment-collection-tenant-context.mjs` guards:

- two operation-aware tenant parser callsites;
- operation selection and admission before tenant parsing;
- tenant parsing before owner storage/service work;
- stable tenant validation code, message, kind, and retryability;
- UUID parse cause plus complete available `PortContext` fields;
- truthful payment owner, exact operation, validation phase, and boundary;
- diagnostics before returning the validation error;
- unchanged admission helpers;
- unchanged reusable lookup, race adoption, status projection, provider,
  reconciliation, and storage envelopes;
- absence of the old operation-free and silent parser paths.

The preceding
`scripts/verify/verify-payment-collection-admission-context.mjs` is synchronized
to the operation-aware parser signature while preserving all admission assertions.

## Remaining gaps

The master ecommerce correlation-safe mapper task remains open for:

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
node scripts/verify/verify-payment-collection-tenant-context.mjs
node scripts/verify/verify-payment-collection-admission-context.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-payment --lib
```
