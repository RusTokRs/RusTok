# Inventory availability and quantity owner context

Status: **source-ready / unvalidated**

## Scope

The canonical `InProcessInventoryReservationPort` remains the admission and local-outcome wrapper for
availability reads and deprecated quantity reserve/release mutations. This slice bounds its admission
and tenant-validation diagnostics without changing accepted requests, owner delegation, or public
contracts.

## Canonical API and ordering

The crate continues to export `InProcessInventoryReservationPort` and
`in_process_inventory_reservation_port`. Repository checkout compositions continue to use the
canonical factory.

Availability still requires read policy and trimmed tenant parsing before owner delegation. Reserve
and release still require write policy, write semantics, and trimmed tenant parsing before delegation.
The original context and request are delegated unchanged after adapter acceptance.

## Admission diagnostics

Policy and write-semantics rejection still return the exact original `PortError`. Technical
unavailable, timeout, and invariant failures retain error severity; ordinary caller rejection retains
warning severity.

Admission events now retain only truthful owner/operation/phase/boundary, correlation ID, bounded
context shape, stable code, error-message length, retryability, and technical-failure classification.
They no longer record the full `PortError`, raw context values, message text, or debug error kind.

## Tenant validation

The parser still trims `PortContext.tenant_id`, parses it as a UUID, and returns the unchanged
validation envelope:

- code `inventory.context_invalid`;
- message `inventory request context is invalid`.

A parse rejection now records tenant original/trimmed lengths, a static parse-failed fact, bounded
context shape, stable code, message length, retryability, correlation, and boundary. It no longer
records the UUID parser cause, tenant text, complete mapped error, or other raw context values.

No actor validation was added.

## Local owner outcomes

Covered post-delegation outcomes retain exact code-and-message routing and same-error return. Their
bounded diagnostic contract is documented in
[`availability-quantity-local-context.md`](./availability-quantity-local-context.md).

## Checkout composition

Journaled compatibility, mounted staged storefront, and legacy storefront compatibility continue to
construct the adapter through `in_process_inventory_reservation_port`. Durable identity reservation
composition and all other checkout owners are unchanged.

## Evidence

- `crates/rustok-inventory/contracts/evidence/availability-quantity-diagnostic-safety-source-review.json`
- `scripts/verify/verify-inventory-availability-quantity-context.mjs`
- `scripts/verify/verify-inventory-availability-quantity-local-context.mjs`

## Remaining boundary

The source-level raw diagnostics identified in `reservation_port_context.rs` are closed. The broader
ecommerce correlation-safe mapper task remains open for other owner adapters and non-`PortError`
public envelopes.

## Validation disclosure

No tests, Node verifiers, Cargo commands, formatting, workflows, or CI were executed. No compile or
runtime status is promoted.
