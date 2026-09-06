# Customer read port policy context

Status: `source_ready_unvalidated`

## Scope

All four owner read methods assign their canonical operation before applying the shared `PortCallPolicy::read()` admission helper:

- `read_customer_projection`;
- `read_customer_projection_by_user`;
- `list_customer_projections`;
- `list_profile_enrichment`.

A rejected admission is diagnosed and the original admission `PortError` is returned unchanged.

## Bounded context shape

The owner policy event retains:

- owner `rustok_customer`;
- exact operation and boundary `customer_read_port`;
- correlation-ID character length;
- tenant and actor-ID character lengths;
- a closed actor-kind label;
- claim and role counts;
- optional channel, causation, traceparent, and idempotency presence/length;
- locale length and deadline.

The raw correlation ID is not recorded. Raw tenant, actor, channel, locale, causation, traceparent, and idempotency values are also excluded.

## Bounded admission error shape

The event retains stable code, retryability, message presence/length, and a closed seven-value error-kind label. The complete `PortError` is not copied into the event, message text is not recorded, and the kind is not debug-formatted.

The original admission `PortError` is returned unchanged.

Technical unavailable, timeout, and invariant admission failures retain their typed error semantics. The current read-policy helper emits the existing warning admission event without changing admission behavior or public envelopes.

## Canonical local outcomes

Canonical root construction separately uses `InProcessCustomerReadPort` to retain bounded context/request/error shape after owner delegation. Its diagnostics also retain only correlation-ID character length and do not record the raw correlation ID.

The wrapper classifies only exact stable `operation + code + message` outcomes and returns the same delegated `PortError` unchanged. Its contract is documented in [`read-local-context.md`](./read-local-context.md).

Policy rejection occurs before owner delegation. Covered local outcomes occur after the unchanged persistent implementation returns. Direct callers of the compatibility factory under `rustok_customer::ports` do not pass through the root wrapper, but they still use the bounded owner policy helper.

## Preserved contracts

This source slice does not alter:

- the `CustomerReadPort` trait or DTOs;
- read deadline requirements;
- tenant parsing or list validation;
- customer service calls and profile enrichment;
- public codes, messages, kinds, or retryability;
- Commerce GraphQL not-found and fallback behavior;
- Customer FFA/FBA status.

## Evidence and open validation

The source contract is guarded by:

- `scripts/verify/verify-customer-read-policy-context.mjs`;
- `scripts/verify/verify-customer-read-local-context.mjs`;
- `crates/rustok-customer/contracts/evidence/customer-read-diagnostic-safety-source.json`;
- `crates/rustok-customer/contracts/evidence/customer-read-diagnostic-safety-source-review.json`.

Direct compatibility callers, consumer-side transport mappings, customer writes, profile transports, compile/runtime traces, restart behavior, and remote-profile execution remain open. The wider ecommerce correlation-safe mapper cleanup also remains open.

No test, Node verifier, Cargo command, formatter, workflow, CI, or mounted runtime target was executed for this source slice.
