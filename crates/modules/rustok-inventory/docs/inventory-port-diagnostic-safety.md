# Inventory port diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This contract closes the currently identified payload-diagnostic gaps in `crates/rustok-inventory/src/ports.rs` across:

- tenant UUID parsing;
- tenant-scoped variant lookup;
- SeaORM storage-error mapping;
- `inventory_error_to_port_error` owner mapping.

The three compatibility operations on `InventoryReservationPort` and the two durable identity operations on `InventoryReservationIdentityPort` remain unchanged. Request/response DTOs, policy and write-semantics checks, normalization, owner delegation, SQL queries, locking, transaction order, reservation identity rules, metadata and state transitions are unchanged.

## Bounded context

Inventory port events retain correlation ID, owner operation, stable code and boundary. Context is represented only through bounded shape:

- tenant and actor-ID character lengths;
- a closed actor-kind label;
- claim and role counts;
- optional channel, causation, trace and idempotency presence/length;
- locale length and deadline.

Raw tenant, actor, channel, locale, causation ID, traceparent and idempotency key values are not recorded by `ports.rs` diagnostics.

## Bounded owner errors

All fifteen current `CommerceError` variants receive a closed static variant label. Diagnostics retain only:

- text-field count and aggregate character length;
- UUID-field count and non-nil count;
- numeric-field count plus non-zero and negative counts;
- opaque-payload presence for database, rich and core errors.

Database/Rich/Core payloads, UUID values, validation or price text, handles, locales, SKUs, shipping-profile slugs and exact requested/available inventory values are not recorded.

## Parser, lookup and storage events

Invalid tenant input still returns `inventory.context_invalid` with `inventory request context is invalid`. The event retains only `tenant_id_parse_failed = true` and bounded context shape.

A missing tenant-scoped variant still returns `inventory.variant_not_found` with `inventory variant was not found`. The event retains UUID shape, not the variant ID.

SeaORM failures still return `inventory.database_unavailable` with the existing public unavailable envelope. The database payload is treated as opaque and is not logged.

## Preserved public behavior

- database failures remain error severity;
- unexpected owner variants remain invariant/error severity;
- not-found, insufficient-inventory and validation outcomes remain warning severity;
- public codes, messages, kinds and retryability are unchanged;
- all five port operations and owner/storage helper call sites remain context-aware.

## Deliberate boundary

`crates/rustok-inventory/src/reservation_owner_context.rs` is a separate local diagnostic surface and is not changed or claimed closed by this contract. The broader ecommerce mapper cleanup also remains open.

Compile validation, focused and aggregate verifier execution, verifier-test execution and mounted runtime evidence remain open.

## Evidence

- `crates/rustok-inventory/contracts/evidence/inventory-port-diagnostic-safety-source.json`
- `crates/rustok-inventory/contracts/evidence/inventory-port-diagnostic-safety-source-review.json`
- `scripts/verify/verify-inventory-port-diagnostic-safety.mjs`
- `scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs`
- `scripts/verify/verify-ecommerce-public-port-error-safety-v2.test.mjs`

No test, verifier, formatter, Cargo, workflow or CI command was executed for this source contract.
