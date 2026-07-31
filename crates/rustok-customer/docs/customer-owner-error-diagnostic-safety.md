# Customer read-boundary diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This contract closes the currently identified payload-diagnostic gaps in `crates/rustok-customer/src/ports.rs` across:

- read-policy admission;
- list request validation;
- tenant UUID parsing;
- post-delegation `customer_error_to_port_error` mapping.

The four `CustomerReadPort` operations, request/response DTOs, admission and validation order, tenant parsing result, owner service delegation, pagination and enrichment behavior remain unchanged.

All seven current `CustomerError` variants retain their existing public `PortError` mapping:

- database unavailable;
- customer not found;
- customer by user not found;
- duplicate email;
- duplicate user link;
- validation;
- profile unavailable.

## Admission diagnostics

A rejected read policy still returns the exact `PortError` produced by `PortContext::require_policy`. The warning event retains:

- correlation id, owner operation and boundary;
- stable code and retryability;
- a closed static `PortErrorKind` label;
- message presence and character length;
- bounded context shape such as tenant/actor lengths, actor kind, claim/role counts, optional metadata presence/length and deadline.

The complete `PortError`, public message text, raw tenant, actor, channel, locale, causation id and traceparent are not recorded.

## List-validation diagnostics

The existing `customer.page_invalid` and `customer.per_page_invalid` codes, messages, warning severity and validation order remain unchanged.

Events retain only the static validation field, numeric page/per-page bounds, search presence/length and bounded context shape. Search text and raw context values are not recorded.

## Tenant-parser diagnostics

Invalid tenant input still returns `customer.context_invalid` with `customer request context is invalid`.

The parser event retains only `tenant_id_parse_failed = true`, tenant input length, bounded context shape, operation, code and boundary. The UUID parser error and raw tenant value are not recorded.

## Owner-error diagnostics

The owner mapper records correlation id, owner operation, boundary, stable code and bounded context shape. Owner failures retain only:

- a closed static error variant;
- text-field count and total character length;
- UUID-field count and non-nil count;
- opaque-payload presence for database and profile failures.

Database/Profile errors, validation messages, email values, customer IDs, user IDs and complete `CustomerError` debug/display payloads are not recorded.

## Preserved behavior

- database and profile failures remain error severity;
- admission, list-validation, tenant-parser, not-found, conflict and validation failures remain warning severity;
- all public codes, messages, kinds and retryability are unchanged;
- all four owner service calls and their arguments are unchanged;
- read policy, list validation and tenant parsing still execute in their previous order.

## Evidence

- `crates/rustok-customer/contracts/evidence/customer-owner-error-diagnostic-safety-source.json`
- `crates/rustok-customer/contracts/evidence/customer-owner-error-diagnostic-safety-source-review.json`
- `scripts/verify/verify-customer-owner-error-diagnostic-safety.mjs`
- `scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs`

Compile validation, focused and aggregate verifier execution, mounted runtime evidence and the broader ecommerce cleanup remain open.

No test, verifier, formatter, Cargo, workflow or CI command was executed for this source contract.
