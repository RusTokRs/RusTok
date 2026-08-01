# Inventory durable reservation local diagnostic safety

Status: **source-ready / unvalidated**

## Scope

The canonical durable reservation wrapper classifies covered owner failures
using exact stable `code + message` pairs and returns the same delegated
`PortError` unchanged.

The covered operations remain reserve and release by durable reservation
identity. Unknown owner envelopes pass through without an additional local
event.

## Bounded request shape

Reserve diagnostics retain only:

- reservation-ID presence and non-nil status;
- variant-ID presence and non-nil status;
- quantity presence, non-zero status, and negative status;
- line-item-ID presence and non-nil status;
- external-ID character length.

Release diagnostics retain reservation-ID shape and external-ID length while
marking variant, quantity, and line-item facts absent.

Raw UUIDs, exact quantity, and external-ID text are not recorded.

## Local classification

The existing exact classifications remain unchanged for external-ID and
quantity validation, variant/state lookup, replay identity conflicts,
insufficient inventory, release lookup, missing inventory item, locked identity
revalidation, ledger consistency, available-quantity overflow, and storage
unavailability.

Technical unavailable, timeout, and invariant outcomes retain error severity.
Ordinary validation, not-found, and conflict outcomes retain warning severity.

## Bounded delegated error shape

Covered events retain stable code, retryability, a closed error-kind label, and
error-message presence/length. The complete delegated `PortError` is not logged,
raw message text is not copied into the event, and debug-formatted kind output is
not retained.

The same delegated error is returned unchanged.

## Preserved owner behavior

The persistent implementation in `ports.rs` is unchanged, including:

- external-ID normalization;
- positive quantity validation;
- reservation and external-ID replay adoption;
- row locking and transaction ordering;
- stock and backorder policy;
- release identity checks and idempotency;
- reservation-ledger mutation;
- available-quantity calculation;
- storage mapping.

## Evidence

- `crates/rustok-inventory/contracts/evidence/inventory-reservation-owner-diagnostic-safety-source.json`
- `crates/rustok-inventory/contracts/evidence/inventory-reservation-owner-diagnostic-safety-source-review.json`
- `scripts/verify/verify-inventory-reservation-local-context.mjs`
- `scripts/verify/verify-inventory-reservation-owner-context.mjs`

No tests, verifiers, formatting, Cargo commands, workflows, CI, or mounted
runtime validation were run.

The ecommerce correlation-safe mapper task remains open for the remaining
payment, fulfillment, customer, promotion, ecommerce adapter, and non-`PortError`
surfaces.
