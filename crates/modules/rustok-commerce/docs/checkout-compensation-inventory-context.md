# Checkout inventory compensation error safety

Status: **source-reviewed / unvalidated**

## Scope

This document records the inventory side of the mounted
`checkout_compensation_error_safe.rs` facade. The facade adapts payment, order,
inventory, and cart owner errors while retaining
`checkout_compensation_owner_ports.rs` unchanged as private business logic.

## Inventory boundary policy

The constructor-injected canonical `InventoryReservationIdentityPort` is wrapped
before the retained service can use it. The adapter preserves the original
`PortContext`, release request, successful snapshot, `PortError.kind`, exact
owner `code`, and `retryable`.

Owner message text is replaced with a static Commerce-owned message before public
error construction or retryable journal persistence. Diagnostics contain only
bounded context/request shapes, owner classification, message presence/length,
and a redacted token.

Retained compatibility diagnostics are suppressed for payment, order, inventory,
and cart; unrelated owner labels still forward.

## Preserved behavior

Reservation selection, status handling, release request identity, response
validation, the durable `mark_released` checkpoint, compensation ordering, and
journal control flow are unchanged. The retained compensation source is private
and unchanged.

## Remaining work

The checkout compensation owner-port consumer mapper is source-closed for all four
owners. Broader ecommerce adapters, non-`PortError` envelopes, and compile/runtime
evidence remain open.

## Validation disclosure

No tests, Node verifiers, Cargo commands, formatting, inventory scenarios,
database scenarios, restart scenarios, remote-port scenarios, workflows, or CI
were run.
