# Commerce admin promotion native error safety

Status: `source-complete / unvalidated`

This slice hardens the mounted Commerce admin cart-promotion native server
functions without changing their request/response contracts, permission policy,
or cart promotion owner behavior.

## Covered endpoints

- `commerce/admin/preview-cart-promotion`
- `commerce/admin/apply-cart-promotion`

The client/hydrate server-function contract remains in
`admin/src/transport/native_server_adapter.rs`. SSR routes through
`admin/src/transport/native_server_adapter_ssr.rs`, where the promotion
boundary is hardened. The order-change endpoints are retained in the SSR
adapter for contract parity, but their remaining error cleanup is not claimed by
this slice.

## Public boundary

Auth and tenant extraction failures no longer serialize framework rejection
text. They return static operation-independent availability envelopes:

- `Commerce admin authentication context is temporarily unavailable`
- `Commerce admin tenant context is temporarily unavailable`

Transport-owned permission and promotion-input validation messages are
unchanged. The cart owner already maps `CartError` into a sanitized typed
`PortError`; the Commerce admin consumer forwards only that safe public
`PortError.message`.

## Port context

Each mounted promotion call creates a unique transport correlation id and keeps
the two-second owner-port deadline. The write call also carries a non-empty
idempotency key.

When `RequestContext` is available, its effective locale and resolved channel
are propagated into `PortContext`. Tenant default locale remains the fallback.
Request-context extraction is attribution-only: failure is logged and does not
change permission or operation admission.

## Diagnostics

Framework extraction failures retain their original cause only in SSR logs with
consumer operation, context kind, correlation id, stable code, and boundary.

Typed owner failures are additionally logged at the Commerce admin consumer
boundary with:

- cart promotion owner and Commerce admin consumer;
- consumer and owner operation;
- correlation id, tenant, actor, and cart;
- request tenant/user/channel/locale when available;
- public error code, error kind, retryability, and boundary.

Unavailable, timeout, and invariant failures use error severity. Ordinary
validation, not-found, conflict, and forbidden outcomes use warning severity.
Promotion source ids, metadata payloads, and raw owner/database causes are not
added as structured fields.

## Source guard and evidence

The focused guard is:

```text
scripts/verify/verify-commerce-admin-promotion-native-error-safety.mjs
```

Retained source evidence is:

```text
crates/rustok-commerce/contracts/evidence/admin-promotion-native-error-safety-source.json
crates/rustok-commerce/contracts/evidence/admin-promotion-native-error-safety-source-review.json
```

The evidence remains explicitly unvalidated. No focused or aggregate verifier,
Cargo command, test, formatting command, workflow, CI job, or runtime trace was
executed for this slice.

## Remaining work

The ecommerce master mapper-cleanup item stays open. Commerce admin order-change
errors, tax, and other remaining ecommerce adapters require separate source and
runtime evidence before the broad invariant can be completed.
