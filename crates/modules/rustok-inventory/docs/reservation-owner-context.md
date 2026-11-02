# Inventory reservation owner diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This slice closes the remaining payload-diagnostic gaps in the canonical durable
inventory reservation wrapper:

- write-policy admission;
- write-semantics admission;
- tenant UUID validation;
- stable post-delegation local outcomes.

The original diagnostic slice covered reserve and release. The current public operations are:

- `InventoryReservationIdentityPort::reserve_inventory_by_identity`;
- `InventoryReservationIdentityPort::release_inventory_by_identity`;
- `InventoryReservationIdentityPort::adopt_inventory_by_identity` (2026-10-31 source continuation).

The original reserve/release facade, factory names, request/response DTOs, ordering,
persistent-owner delegation, and returned `PortError` values remain unchanged. Adoption adds
one explicit bounded request/response on the same identity capability, not an alternate provider.
See the [owner adoption contract](./implementation-plan.md#reservation-adoption-into-order-2026-10-31-source-work).

## Canonical public paths (2026-10-30)

Both crate-root and `ports` factory/concrete-wrapper paths now expose the same admitted
wrapper. `ports.rs` is compiled privately as `ports_impl`; its persistence implementation
is constructed only inside that wrapper, not publicly re-exported. DTO and trait identities
remain unchanged. Entry diagnostics in the touched owner paths also use boolean/length
facts rather than request UUIDs, channel/external-ID text or exact quantities.

Selected-channel candidate filtering and new-only `allocated_channel_slug` provenance
are described in the [implementation plan](./implementation-plan.md). Exact receipt replay
and release stay before/independent of current location visibility. These source changes do
not certify database concurrency, outbox/projection completeness or mounted behavior.

## Preserved ordering

Reserve/release retain this order; adoption uses the same admitted order:

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

- `crates/modules/rustok-inventory/contracts/evidence/inventory-reservation-owner-diagnostic-safety-source.json`
- `crates/modules/rustok-inventory/contracts/evidence/inventory-reservation-owner-diagnostic-safety-source-review.json`
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


## Adoption diagnostic continuation (2026-10-31)

The same wrapper admits the batch before delegation and records only binding count and bounded
shape facts. Shape probes inspect at most 1025 bindings and cap an external-ID preview length
at 192 characters; no UUID, exact quantity or external-ID text is recorded. Owner-local adoption
validation and ledger failures use stable exact code/message classifications and return the same
`PortError`. The consuming Commerce boundary separately bounds public code/message and keeps
exact retry advice; advice does not authorize compensation or key rotation.

The new source guard is `scripts/verify/verify-commerce-checkout-inventory-adoption.mjs`.
The existing diagnostic packet's adoption continuation remains source-ready/uncompiled with tests,
SQL, concurrency and mounted/remote execution unproven; original diagnostic status is not promoted.
Metadata-capacity/size-proof/backend refusals also use exact static classifications without raw
saved metadata. Both public factory/concrete paths retain canonical positive-deadline admission.
