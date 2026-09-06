# Checkout payment compensation error safety

Status: **source-reviewed / unvalidated**

## Scope

The mounted Commerce checkout compensation facade is now
`checkout_compensation_error_safe.rs`. It retains
`checkout_compensation_owner_ports.rs` privately and adapts payment, order,
inventory, and cart owner errors before the retained Commerce mapper.

This document records the payment side of the combined four-owner facade.

## Payment boundary policy

The default payment factory, provider-registry constructor, and custom payment
port injection are wrapped. The wrapper preserves the canonical request and
response, delegated `PortContext`, `PortError.kind`, exact owner `code`,
`retryable`, successful snapshot, and compensation ordering.

Owner message text is replaced by a static Commerce-owned message before public
error construction or retryable journal persistence. The manual-reconciliation
code receives `Checkout payment compensation requires manual reconciliation`.

Diagnostics contain only bounded context/request shapes, owner classification,
message presence/length, and a redacted token. Retained compatibility diagnostics
are suppressed for payment, order, inventory, and cart; unrelated labels still
forward.

## Preserved behavior

Payment provider selection, collection identity checks, cancellation status
checks, compensation ordering, journal control flow, and public builder signatures
are unchanged. The retained compensation source remains private and unchanged.

## Remaining work

The checkout compensation owner-port consumer mapper is source-closed for all four
owners. Broader ecommerce adapters, non-`PortError` envelopes, and compile/runtime
evidence remain open.

## Validation disclosure

No tests, Node verifiers, Cargo commands, formatting, payment-provider calls,
database scenarios, restart scenarios, remote-port scenarios, workflows, or CI
were run.
