# Commerce admin order-change native diagnostic safety

Date: 2026-08-03

Status: `source-complete / unvalidated`

This source slice hardens the mounted Commerce admin native order-change diagnostics for:

- `commerce/admin/order-changes`
- `commerce/admin/apply-order-change`
- `commerce/admin/cancel-order-change`

The endpoint names, wrapper signatures, permissions, tenant matching, parsing, DTO mapping,
owner calls, pagination, filtering, apply/cancel inputs, and persisted behavior remain
unchanged.

## Public envelopes

Auth and tenant extraction failures retain the existing static messages:

- `Commerce admin authentication context is temporarily unavailable`
- `Commerce admin tenant context is temporarily unavailable`

Missing host event-bus composition retains:

- `Commerce order-change runtime is temporarily unavailable`

Typed `rustok_order::error::OrderError` variants retain the same public mapping:

| Owner error | Public message | Severity |
| --- | --- | --- |
| `Validation` | `Order change request is invalid` | warning |
| `OrderNotFound`, `OrderReturnNotFound`, `OrderChangeNotFound` | `Order resource was not found` | warning |
| `InvalidTransition` | `Order change conflicts with the current order state` | warning |
| `Database` | `Order storage is temporarily unavailable` | error |
| `Core` | `Order change could not be completed safely` | error |

No public message or error-classification behavior is changed by this slice.

## Framework diagnostics

Auth, tenant, and optional `RequestContext` extraction diagnostics keep the consumer,
operation, context kind, correlation ID, stable code, boundary, and Rust error type. The
complete framework extraction errors are not logged; neither debug nor display text is
retained.

The order-change auth, tenant, and shared context helpers no longer require a `Debug`
implementation from the framework extraction error type. This makes the type-only policy
explicit in the function contract: the payload is accepted for ownership and immediately
discarded, while only `std::any::type_name::<E>()` is retained for diagnostics.

Optional `RequestContext` extraction remains diagnostic-only. Failure still falls back
without changing authentication, authorization, tenant matching, validation, or owner-call
admission.

## Runtime composition diagnostics

Missing `TransactionalEventBus` still produces the static runtime envelope and stable
runtime diagnostic code. The event records only:

- tenant and actor UUID non-nil facts;
- request-context presence;
- request tenant/user/channel UUID presence and non-nil facts;
- channel-slug and locale presence/length facts;
- owner, consumer, operation, correlation ID, code, and boundary.

Tenant, actor, request tenant/user/channel UUIDs, channel slug, and locale are not logged as
full values.

## Owner diagnostics

The owner mapper still covers all seven `OrderError` variants and preserves the original
warning/error split. Diagnostics retain only:

- owner, consumer, operation, correlation ID, typed error kind, stable public code, and
  boundary;
- effective tenant and actor UUID non-nil facts;
- order and order-change ID presence/non-nil facts;
- request-context identity presence/non-nil/length facts;
- validation-detail presence and length;
- not-found resource UUID presence/non-nil facts;
- transition source/target string lengths;
- database/core cause presence flags.

The complete `OrderError` and identity values are not logged. Validation detail, transition
state text, database/core causes, tenant/user/order UUIDs, channel slug, and locale remain
absent from structured values.

## Preserved correlation and orchestration

Each mounted call still creates a unique correlation identifier:

```text
commerce-admin-order-change:{operation}:{uuid}
```

Direct `OrderService` construction, `TransactionalEventBus` lookup, list/apply/cancel call
order, operation constants, and result mapping are unchanged. This slice does not claim a
host-selected typed owner port or native/REST orchestration parity.

## Promotion compatibility

The same SSR file contains the previously hardened cart-promotion boundary. Its safe
framework type-only diagnostics, promotion request-context facts, safe `PortError` shape,
endpoints, permissions, channel/locale forwarding, deadline, idempotency, public message,
and owner calls remain unchanged.

The promotion verifier now treats the independently guarded order-change type-only mapper as
a preserved compatibility marker and fails if the obsolete order-change `Debug` bound
returns. Promotion evidence and runtime behavior are not promoted or reinterpreted by this
slice.

## Evidence boundary

Focused guard:

```text
scripts/verify/verify-commerce-admin-order-change-native-error-safety.mjs
```

Compatibility guard:

```text
scripts/verify/verify-commerce-admin-promotion-native-error-safety.mjs
```

Retained evidence:

```text
crates/rustok-commerce/contracts/evidence/admin-order-change-native-error-safety-source.json
crates/rustok-commerce/contracts/evidence/admin-order-change-native-error-safety-source-review.json
```

No test, verifier, Cargo command, formatting command, workflow, CI job, mounted request, or
runtime failure-injection trace was executed for this source slice.

## Remaining work

The broad ecommerce mapper-cleanup item remains open for direct owner-port composition,
native/REST parity, remaining order, payment, fulfillment, inventory, customer, tax,
promotion and adapter boundaries, and non-`PortError` public envelopes.
