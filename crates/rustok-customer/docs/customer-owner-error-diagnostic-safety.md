# Customer read-boundary diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This contract closes the currently identified payload-diagnostic gaps in `crates/rustok-customer/src/ports.rs` across:

- read-policy admission;
- list request validation;
- tenant UUID parsing;
- post-delegation `customer_error_to_port_error` mapping.

The four `CustomerReadPort` operations, request/response DTOs, admission and validation order, tenant parsing result, owner service delegation, pagination and enrichment behavior remain unchanged.

All seven current `CustomerError` variants retain their existing public `PortError` mapping: database unavailable, customer not found, customer-by-user not found, duplicate email, duplicate user link, validation, and profile unavailable.

## Correlation-safe bounded context

Every Customer owner read event now retains only correlation-id character length. The raw correlation id is not recorded by admission, list-validation, tenant-parser, or owner-error diagnostics.

Other context remains represented only through bounded facts:

- tenant and actor-ID character lengths;
- a closed actor-kind label;
- claim and role counts;
- optional channel, causation, traceparent, and idempotency presence/length;
- locale length and deadline.

Raw tenant, actor, channel, locale, causation, traceparent, idempotency, and request identity values are not recorded.

## Admission, validation, and parser diagnostics

A rejected read policy still returns the exact `PortError` produced by `PortContext::require_policy`. The warning event retains stable code, retryability, a closed `PortErrorKind` label, message presence/length, operation, boundary, and bounded context shape.

The existing `customer.page_invalid`, `customer.per_page_invalid`, and `customer.context_invalid` outcomes retain their messages, validation order, warning severity, and request behavior. List events retain only numeric pagination bounds plus search presence/length. Tenant parsing retains only a static parse-failure fact and bounded context.

## Owner-error diagnostics

Owner failures retain only:

- exact owner operation and stable code;
- a closed static error variant;
- text-field count and total character length;
- UUID-field count and non-nil count;
- opaque-payload presence for database and profile failures;
- bounded context shape and correlation-id character length.

Database/Profile errors, validation messages, email values, customer IDs, user IDs, complete `CustomerError` payloads, and the raw correlation id are not recorded.

## Preserved behavior

- database and profile failures remain error severity;
- admission, list-validation, tenant-parser, not-found, conflict, and validation failures remain warning severity;
- all public codes, messages, kinds and retryability are unchanged;
- all four owner service calls and their arguments are unchanged;
- read policy, list validation, and tenant parsing still execute in their previous order;
- Customer FFA/FBA status is unchanged.

## Evidence and remaining work

- `crates/rustok-customer/contracts/evidence/customer-owner-error-diagnostic-safety-source.json`
- `crates/rustok-customer/contracts/evidence/customer-owner-error-diagnostic-safety-source-review.json`
- `scripts/verify/verify-customer-owner-error-diagnostic-safety.mjs`
- `scripts/verify/verify-customer-read-policy-context.mjs`
- `scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs`

Compile validation, focused and aggregate verifier execution, mounted runtime evidence, write-side Customer cleanup, remote adapters, and the broader ecommerce cleanup remain open.

No test, verifier, formatter, Cargo, workflow or CI command was executed for this source contract.
