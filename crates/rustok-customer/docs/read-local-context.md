# Customer read local outcome context

Status: **source-ready / unvalidated**

## Scope

This slice retains stable owner-local outcome context for the canonical root customer read construction:

- `CustomerReadPort::read_customer_projection`;
- `CustomerReadPort::read_customer_projection_by_user`;
- `CustomerReadPort::list_customer_projections`;
- `CustomerReadPort::list_profile_enrichment`;
- root `InProcessCustomerReadPort`;
- root `in_process_customer_read_port`.

The existing owner implementation in `ports.rs` remains unchanged. A root wrapper retains the delegated
`PortContext` and safe request facts, calls the original `CustomerService` port implementation, classifies
only exact stable returned `PortError` envelopes, and returns the same error unchanged.

## Canonical root cutover

The crate keeps `pub mod ports`, so the existing trait, request/response DTOs, service implementation, and
module-path factory remain available for compatibility. Root exports now separate contracts from canonical
construction:

- `CustomerReadPort` and its DTOs continue to come from `ports`;
- root `InProcessCustomerReadPort` and `in_process_customer_read_port` come from `read_context`.

Current Commerce consumers import the root factory, so they use the wrapper without changing transport or
orchestration source. Callers that deliberately construct through `rustok_customer::ports` remain an
explicit compatibility bypass and are not counted as covered by this slice.

## Delegation order

Each wrapper operation performs the same sequence:

1. clone the accepted `PortContext` for diagnostics;
2. retain operation-specific safe request facts;
3. delegate the original context and request to the unchanged owner implementation;
4. inspect only a returned `PortError`;
5. emit a local event only when the exact stable code and message are covered;
6. return the same `PortError` unchanged.

The persistent owner continues to own read-policy admission, tenant parsing, page validation, tenant-scoped
queries, profile enrichment, DTO construction, and public error mapping.

## Safe request facts

Covered diagnostics may retain:

- typed customer id for `read_customer_projection`;
- typed user id for `read_customer_projection_by_user`;
- page, page size, and search character length for `list_customer_projections`;
- requested and unique user-id counts for `list_profile_enrichment`.

The wrapper does not retain raw search text, customer names, email addresses, profile names, preferred
locale values, customer records, result rows, or profile payloads.

## Covered stable outcomes

The mapper requires exact `operation + code + message` matches where an outcome is operation-specific.
Unknown envelopes pass through without an additional event.

| Stable envelope | Covered operation | Local operation | Severity |
| --- | --- | --- | --- |
| `customer.context_invalid` / `customer request context is invalid` | all | `validate_tenant_context` | warning |
| `customer.page_invalid` / `customer projection page is invalid` | list customers | `validate_page` | warning |
| `customer.per_page_invalid` / `customer projection page size is invalid` | list customers | `validate_page_size` | warning |
| `customer.database_unavailable` / `customer storage is temporarily unavailable` | all | `owner_storage` | error |
| `customer.customer_not_found` / `customer was not found` | customer-id read | `load_customer` | warning |
| `customer.customer_by_user_not_found` / `customer was not found for the requested user` | user-id read | `load_customer_by_user` | warning |
| `customer.validation` / `customer request is invalid` | all | `validate_owner_request` | warning |
| `customer.profile_unavailable` / `customer profile projection is temporarily unavailable` | all | `load_profile_projection` | error |

Unavailable, timeout, and invariant kinds use error severity. Validation and not-found outcomes use warning
severity.

## Retained diagnostic context

Covered outcomes record:

- truthful owner `rustok_customer`;
- exact public owner operation;
- operation-specific local label;
- boundary `customer_read_port`;
- correlation id and tenant id;
- typed actor, channel, and locale;
- causation id and traceparent when available;
- idempotency key and deadline when available;
- safe operation-specific request facts;
- exact stable code and public-safe message;
- typed error kind and retryability;
- the complete delegated `PortError`.

## Preserved behavior

This work does not change:

- the `CustomerReadPort` trait or DTOs;
- `CustomerService` queries and profile bridge behavior;
- `PortCallPolicy::read()` admission or deadline semantics;
- tenant parsing and tenant isolation;
- page and page-size bounds;
- optional authenticated-customer not-found behavior in Commerce consumers;
- public codes, messages, kinds, or retryability;
- FBA or FFA status.

## Static evidence

`scripts/verify/verify-customer-read-local-context.mjs` guards:

- legacy module compatibility plus canonical root wrapper construction;
- context and safe-fact retention before unchanged owner delegation;
- all four operations and exact operation constants;
- exact stable code-and-message classification;
- complete `PortContext`, safe request-fact, and delegated-error fields;
- technical versus ordinary severity;
- absence of raw search, customer, email, profile, and result payload logging;
- same delegated error return.

The customer no-compile FBA guard also checks the root wrapper while retaining `ports.rs` as the owner
implementation source. The runtime-smoke typed-error matrix is synchronized with the existing
`customer.context_invalid` contract; this is evidence repair, not a public error change.

## Remaining gaps

The ecommerce correlation-safe mapper task remains open for:

- direct callers that deliberately bypass the root wrapper through `rustok_customer::ports`;
- consumer-side GraphQL, REST, native, and operator mappings not already covered by focused slices;
- customer write adapters and profile transports;
- remaining promotion, ecommerce, and non-`PortError` envelopes;
- compiled, runtime, restart, remote-profile, and cross-transport evidence.

No architecture status is promoted from source inspection alone.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-customer-read-local-context.mjs
node scripts/verify/verify-customer-read-policy-context.mjs
node scripts/verify/verify-customer-fba-no-compile.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-customer --lib
cargo check -p rustok-commerce --lib
```
