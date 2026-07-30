# Commerce admin order-change native error safety

Date: 2026-07-30

Status: `commerce_admin_order_change_native_error_safety_source_unvalidated`

This source slice hardens the mounted Commerce admin native order-change endpoints:

- `commerce/admin/order-changes`
- `commerce/admin/apply-order-change`
- `commerce/admin/cancel-order-change`

The ecommerce master mapper-cleanup item remains open for other owners, adapters, and
non-`PortError` envelopes. This document records only the bounded native order-change
source change and does not promote FFA, FBA, transport, runtime, or production status.

## Problem

The mounted SSR adapter previously converted framework and owner failures directly into
`ServerFnError` values:

- `AuthContext` and `TenantContext` extraction failures were serialized with
  `.map_err(ServerFnError::new)`;
- missing `TransactionalEventBus` exposed the host-composition implementation detail;
- `OrderService` list/apply/cancel failures were converted with
  `.map_err(ServerFnError::new)` and could carry database, core, identity, or transition
  details into the public server-function envelope;
- native order-change failures had no per-call transport correlation identifier or
  request-attributed structured diagnostics.

## Source boundary

The mounted endpoint names, public wrapper signatures, DTO mapping, permissions, tenant
matching, UUID/JSON validation, pagination, status filtering, and owner service calls are
preserved.

Each endpoint now creates a unique correlation identifier:

```text
commerce-admin-order-change:{operation}:{uuid}
```

`RequestContext` extraction is optional and diagnostic-only. Its absence does not change
authentication, authorization, tenant matching, validation, or owner-call admission.

## Public envelopes

Framework context failures use static public messages:

- `Commerce admin authentication context is temporarily unavailable`
- `Commerce admin tenant context is temporarily unavailable`

Missing host event-bus composition returns:

- `Commerce order-change runtime is temporarily unavailable`

Typed `rustok_order::error::OrderError` variants use stable public messages:

| Owner error | Public message |
| --- | --- |
| `Validation` | `Order change request is invalid` |
| `OrderNotFound`, `OrderReturnNotFound`, `OrderChangeNotFound` | `Order resource was not found` |
| `InvalidTransition` | `Order change conflicts with the current order state` |
| `Database` | `Order storage is temporarily unavailable` |
| `Core` | `Order change could not be completed safely` |

The original typed cause remains internal.

## Internal diagnostics

Structured diagnostics identify:

- owner and consumer;
- consumer operation;
- correlation ID;
- effective tenant and authenticated actor;
- order and order-change identity when known;
- request tenant, user, channel ID, channel slug, and locale when available;
- typed error kind;
- stable public code;
- native transport boundary.

Database/core failures are logged as errors. Validation, not-found, and state-conflict
failures are logged as warnings. Raw request metadata is not logged.

## Promotion compatibility

The same mounted SSR source still contains the previously hardened cart-promotion
endpoints. Their endpoint names, permission policy, request parsing, unique correlation,
request locale/channel propagation, write idempotency semantics, consumer diagnostics,
and sanitized `PortError.message` boundary are preserved.

## Explicit non-claims

This source slice does not:

- replace direct `OrderService` construction with a host-selected typed owner port;
- change apply/cancel orchestration policy;
- prove parity with the REST order-change orchestration surface;
- run Cargo, tests, static verifiers, formatting, workflows, or CI;
- retain mounted failure-injection or remote-profile evidence;
- close the broad ecommerce mapper-cleanup task.

## Owner validation

Not run by the implementation agent, per maintainer instruction.

Suggested commands:

```bash
node scripts/verify/verify-commerce-admin-order-change-native-error-safety.mjs
node scripts/verify/verify-commerce-admin-promotion-native-error-safety.mjs
node scripts/verify/verify-commerce-admin-boundary.mjs
cargo check -p rustok-commerce-admin --features ssr
```
