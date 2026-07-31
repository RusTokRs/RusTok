# Commerce Admin order-change client transport error safety

Status: **source-ready / unvalidated**

## Boundary

The public Commerce Admin order-change facade selects GraphQL only for the existing
WASM non-hydrate profile. All other profiles call the shared native adapter. The
GraphQL branches retain the correlation-aware `map_graphql_error` policy.

The shared compatibility `ApiError` stores either `Graphql(String)` or
`ServerFn(String)`. The public native branches already replace the final error with a
static compatibility envelope, but their client diagnostic mapper previously wrote the
complete `ApiError` to tracing. That could retain framework, transport, serialization,
or server-function message text after it left an otherwise sanitized server boundary.

## Covered operations

- `fetch_order_changes`
- `apply_order_change`
- `cancel_order_change`

Each native branch continues to create `OrderChangeClientErrorContext` before the
unchanged adapter call and maps only the final returned `ApiError`.

The final public message remains:

`Commerce admin order-change request could not be completed`

## Private diagnostics

The complete `ApiError` is not logged. The mapper records only:

- error variant: `graphql` or `server_fn`;
- error-message presence and character length;
- owner operation and a per-call correlation ID;
- stable code and transport boundary;
- token presence;
- tenant slug and tenant ID presence or character length;
- order ID, order-change ID, and status presence or character length;
- action payload presence.

It does not record error message text, token, tenant slug, tenant ID, order ID,
order-change ID, status, or action-draft values.

## Preserved behavior

This source slice does not change:

- GraphQL transport selection or GraphQL error mapping;
- the shared default or SSR native adapters;
- mounted endpoints or server functions;
- authentication, tenant, permission, or request-context policy;
- order owner calls, transitions, or response mapping;
- request or response DTOs;
- the `Result<..., ApiError>` facade contract;
- Commerce Admin promotion transport.

## Evidence

- `contracts/evidence/admin-order-change-client-transport-error-safety-source.json`
- `contracts/evidence/admin-order-change-client-transport-error-safety-source-review.json`
- `scripts/verify/verify-commerce-admin-order-change-client-transport-error-safety.mjs`

The server-side source policy remains separately covered by
`verify-commerce-admin-order-change-native-error-safety.mjs`.

## Validation still required

Per maintainer instruction, no tests, Node verifiers, Cargo commands, formatting,
workflows, or CI were run for this slice. Maintainer execution should include the
focused client and native guards, aggregate Commerce/ecommerce guards, hydrate and SSR
compilation, and mounted failure injection for all three operations.

This source result does not promote Commerce FFA/FBA, browser, hydrate, SSR, mounted
runtime, workflow, CI, or production status. The broad ecommerce correlation-safe
mapper cleanup remains open for other owner and non-`PortError` envelopes.
