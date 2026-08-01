# Inventory reservation owner diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This slice closes the remaining payload-diagnostic gaps in the canonical durable
inventory reservation wrapper:

- write-policy admission;
- write-semantics admission;
- tenant UUID validation;
- stable post-delegation local outcomes.

The covered public operations remain:

- `InventoryReservationIdentityPort::reserve_inventory_by_identity`;
- `InventoryReservationIdentityPort::release_inventory_by_identity`.

The public facade, factory names, request/response DTOs, operation ordering,
persistent-owner delegation, and returned `PortError` values are unchanged.

## Preserved ordering

Both operations still perform:

1. write-policy admission;
2. write-semantics admission;
3. trimmed tenant UUID validation;
4. safe diagnostic-fact capture;
5. delegation of the original context and request;
6. exact stable local-outcome classification;
7. return of the same delegated error.

The persistent owner in `ports.rs` still repeats its existing admission and
tenant checks before storage work.

## Bounded context shape

Admission, tenant-validation, and covered local-outcome events retain:

- truthful owner and exact public operation;
- admission, validation, or local-operation label;
- correlation ID and stable boundary;
- tenant and actor-ID character lengths;
- closed actor-kind label;
- claim and role counts;
- channel, causation, traceparent, and idempotency presence plus lengths;
- locale length and deadline;
- stable error code, retryability, closed error-kind label, and error-message
  presence/length.

They do not record raw tenant, actor, channel, locale, causation, traceparent,
or idempotency values. The complete `PortError` is not written through Debug or
Display formatting, and raw error-message text is not copied into events.

Unavailable, timeout, and invariant outcomes retain error severity. Ordinary
validation, not-found, conflict, forbidden, and policy rejections retain warning
severity.

## Tenant validation

Tenant parsing still trims the delegated tenant string and returns:

- code `inventory.context_invalid`;
- message `inventory request context is invalid`;
- validation kind and unchanged retryability.

The UUID parse cause is not recorded. The event retains only
`tenant_id_parse_failed = true`, bounded context shape, and bounded error shape.

The exact admission and tenant-validation errors are returned unchanged.

## Preserved behavior

This work does not change:

- reservation/external identity semantics;
- external-ID normalization or limits;
- positive quantity validation;
- tenant-scoped variant loading;
- row locking, transactions, replay adoption, or insert-race handling;
- location selection or backorder policy;
- metadata or release state transitions;
- storage mappings;
- public codes, messages, kinds, or retryability.

No FBA or FFA status is promoted from source inspection.

## Evidence

- `crates/rustok-inventory/contracts/evidence/inventory-reservation-owner-diagnostic-safety-source.json`
- `crates/rustok-inventory/contracts/evidence/inventory-reservation-owner-diagnostic-safety-source-review.json`
- `scripts/verify/verify-inventory-reservation-owner-context.mjs`
- `scripts/verify/verify-inventory-reservation-local-context.mjs`

## Validation status

Tests, Node verifiers, Cargo commands, formatting, workflows, CI, and mounted
runtime validation were intentionally not run.

Suggested maintainer checks:

```bash
node scripts/verify/verify-inventory-reservation-owner-context.mjs
node scripts/verify/verify-inventory-reservation-local-context.mjs
node scripts/verify/verify-inventory-port-diagnostic-safety.mjs
cargo check -p rustok-inventory --lib
cargo check -p rustok-commerce --lib
```

The broader ecommerce correlation-safe mapper cleanup remains open.
