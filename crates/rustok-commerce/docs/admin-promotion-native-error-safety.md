# Commerce admin promotion native error safety

Status: `source-complete / unvalidated`

This slice hardens the mounted Commerce admin cart-promotion native server
functions without changing request/response contracts, permission policy, owner
calls, or cart-promotion behavior.

## Covered endpoints

- `commerce/admin/preview-cart-promotion`
- `commerce/admin/apply-cart-promotion`

The client/hydrate server-function contract remains in
`admin/src/transport/native_server_adapter.rs`. SSR continues to route through
`admin/src/transport/native_server_adapter_ssr.rs`.

## Public boundary

Auth and tenant extraction failures continue to return static availability
envelopes:

- `Commerce admin authentication context is temporarily unavailable`
- `Commerce admin tenant context is temporarily unavailable`

Transport-owned permission and promotion-input validation messages are unchanged.
The Cart owner still maps `CartError` into a sanitized typed `PortError`, and the
Commerce admin consumer still returns only `PortError.message` through
`ServerFnError`.

## Framework diagnostics

Context-extraction diagnostics retain consumer operation, context kind,
correlation id, stable code, boundary, and the Rust error type. The complete
framework extraction errors are not logged, including their debug or display
text.

The type-only `promotion_context_error`, `promotion_auth_context_error`, and
`promotion_tenant_context_error` helpers no longer require a `Debug` bound. The
error value remains unformatted and the diagnostic contract now reflects the
actual type-only implementation.

Optional `RequestContext` extraction remains attribution-only. Failure still
falls back without changing permission or operation admission, but only the error
type is retained in diagnostics.

## Owner diagnostics

Typed owner failures retain:

- Cart promotion owner and Commerce admin consumer;
- consumer and owner operation;
- correlation id and transport boundary;
- public error code, typed kind, retryability, and public-message presence/length;
- tenant, actor, cart, request tenant/user/channel UUID non-nil facts;
- request-context, channel, and locale presence/length facts;
- effective channel presence and locale length.

The complete `PortError` and identity values are not logged. Tenant, user, cart,
request tenant/user/channel UUIDs, channel slug, locale, and public error message
are not written as full structured values.

Unavailable, timeout, and invariant failures retain error severity. Ordinary
validation, not-found, conflict, and forbidden outcomes retain warning severity.

## Preserved port context

Each mounted promotion call still creates a unique transport correlation id and
keeps the two-second owner-port deadline. Apply still carries a non-empty
idempotency key.

When `RequestContext` is available, its effective locale and resolved channel
continue to cross the owner `PortContext`; tenant default locale remains the
fallback.

The independently guarded order-change boundary in the same SSR adapter retains
its bound-free type-only context helpers and typed owner-error shape contract.

## Source guard and evidence

Focused guard:

```text
scripts/verify/verify-commerce-admin-promotion-native-error-safety.mjs
```

Retained source evidence:

```text
crates/rustok-commerce/contracts/evidence/admin-promotion-native-error-safety-source.json
crates/rustok-commerce/contracts/evidence/admin-promotion-native-error-safety-source-review.json
```

No test, verifier, Cargo command, formatting command, workflow, CI job, or runtime
trace was executed for this source slice.

## Remaining work

The broad ecommerce mapper-cleanup item stays open for remaining order, payment,
fulfillment, inventory, customer, tax, promotion, adapter, and non-`PortError`
envelopes. This source slice does not promote ecommerce FFA or FBA status.
