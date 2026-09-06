# Admin fulfillment diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens the three failure mappers used by the Commerce Admin Fulfillment HTTP boundary:

- owner-port list and detail reads;
- direct fulfillment owner mutations;
- Commerce fulfillment orchestration mutations, including reconciliation-required failures.

The eight mounted route callsites remain unchanged. Only the diagnostic projection emitted after typed policy selection changes.

## Bounded diagnostic projection

Typed `PortError`, `FulfillmentError`, `FulfillmentOrchestrationError`, `PortContext`, and `AdminFulfillmentErrorContext` remain available while HTTP policy is selected. Reconciliation variants also continue to adopt their persisted fulfillment identifier before diagnostics are projected.

Before each `tracing::error!` event:

- the typed error is shadowed by a diagnostic type whose `Debug` output is always `redacted`;
- tenant identity becomes `nil` / `non_nil`;
- optional fulfillment and order identities become `absent` / `present_nil` / `present_non_nil`;
- correlation, actor, and channel values become closed presence-shape labels;
- locale becomes its length rather than its content;
- deadline, stable internal code, retryability, owner operation, route operation, error kind, public code, HTTP status, owner, boundary, and static event message remain observable where previously available.

No backend cause, provider detail, transition payload, validation text, UUID, actor identifier, correlation identifier, channel value, or locale value is emitted by these mappers.

## Preserved behavior

This work does not change:

- `PortErrorKind`, `FulfillmentError`, or `FulfillmentOrchestrationError` HTTP policy;
- nested `FulfillmentOrchestrationError::Fulfillment` delegation;
- persisted fulfillment identity adoption for provider/persistence split-brain variants;
- `HttpError::new(status, code, message)` construction;
- read, create, update permissions;
- list filters, pagination, owner-port requests, and response envelopes;
- direct fulfillment owner calls;
- provider-registry-backed orchestration calls;
- create, show, ship, deliver, reopen, reship, or cancel success contracts.

## Remaining boundary

The broad ecommerce correlation-safe mapper and non-`PortError` public-envelope cleanup remains open. This slice does not claim completion for shipping administration, payment administration, checkout-operation administration, storefront transports, GraphQL boundaries, owner adapters, or runtime verification.

## Evidence

- `crates/rustok-commerce/contracts/evidence/admin-fulfillment-diagnostic-safety-source-review.json`
- `scripts/verify/verify-commerce-admin-fulfillment-route-error-context.mjs`

## Validation disclosure

No tests, Node verifiers, formatting, Cargo commands, mounted HTTP scenarios, workflows, or CI were run. No compile, runtime, FFA, or FBA status is promoted.
