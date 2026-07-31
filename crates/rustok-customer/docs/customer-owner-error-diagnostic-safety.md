# Customer owner error diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This slice closes the owner-error payload gap in `customer_error_to_port_error` inside `crates/rustok-customer/src/ports.rs`.

The four `CustomerReadPort` operations, request/response DTOs, admission order, list validation, tenant parsing, owner service delegation, pagination and enrichment behavior remain unchanged.

All seven current `CustomerError` variants retain their existing public `PortError` mapping:

- database unavailable;
- customer not found;
- customer by user not found;
- duplicate email;
- duplicate user link;
- validation;
- profile unavailable.

## Retained diagnostics

The owner mapper records correlation id, owner operation, boundary, stable code and bounded context shape. Raw tenant, actor, channel, locale and other context values are not recorded by this mapper.

Owner failures retain only:

- a closed static error variant;
- text-field count and total character length;
- UUID-field count and non-nil count;
- opaque-payload presence for database and profile failures.

Database/Profile errors, validation messages, email values, customer IDs, user IDs and complete `CustomerError` debug/display payloads are not recorded by the owner mapper.

## Preserved behavior

- database and profile failures remain error severity;
- not-found, conflict and validation failures remain warning severity;
- all public codes are unchanged;
- all public messages are unchanged;
- public kinds and retryability are unchanged;
- all four owner service calls and their arguments are unchanged.

## Deliberate boundary

This source slice does not close Customer read admission, list-validation or tenant-parser diagnostics. Those paths still retain raw context or parser/error payload and remain separate bounded follow-up work.

The broad ecommerce mapper cleanup, compile validation, focused-verifier execution and mounted runtime evidence remain open.

## Evidence

- `crates/rustok-customer/contracts/evidence/customer-owner-error-diagnostic-safety-source.json`
- `crates/rustok-customer/contracts/evidence/customer-owner-error-diagnostic-safety-source-review.json`
- `scripts/verify/verify-customer-owner-error-diagnostic-safety.mjs`

No test, verifier, formatter, Cargo, workflow or CI command was executed for this source slice.
