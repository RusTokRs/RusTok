# Checkout order compensation error safety

Status: **source-reviewed / unvalidated**

## Scope

The mounted `checkout_compensation_error_safe.rs` facade adapts payment, order,
inventory, and cart owner ports while retaining
`checkout_compensation_owner_ports.rs` unchanged as private business logic.

This document records the order side of the combined four-owner facade.

## Order boundary policy

The facade wraps the default in-process order factory, the identity-aware
constructor, and custom `CheckoutOrderCompensationPort` injection. It delegates
the original context and request and preserves the successful snapshot,
`PortError.kind`, exact owner `code`, and `retryable`.

Owner message text is replaced before the retained mapper sees it. The
manual-reconciliation code receives
`Checkout order compensation requires manual reconciliation`; other messages are
selected statically from `PortErrorKind`.

Diagnostics contain bounded context/request shapes, owner classification,
message presence/length, and a redacted token only. Retained compatibility events
are suppressed for payment, order, inventory, and cart.

## Preserved behavior

Order identity selection, cancellation checks, successful snapshot validation,
compensation ordering, journal control flow, and public builder signatures remain
unchanged. The retained compensation source is private and unchanged.

## Remaining work

The checkout compensation owner-port consumer mapper is source-closed for all four
owners. Broader ecommerce adapters, non-`PortError` envelopes, and compile/runtime
evidence remain open.

## Validation disclosure

No tests, Node verifiers, Cargo commands, formatting, order scenarios, database
scenarios, restart scenarios, remote-port scenarios, workflows, or CI were run.
