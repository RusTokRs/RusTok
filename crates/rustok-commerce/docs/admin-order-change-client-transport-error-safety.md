# Commerce Admin order-change client transport error safety

Status: **source-ready / unvalidated**

## Boundary

The public Commerce Admin order-change facade selects GraphQL only for the existing
WASM non-hydrate profile. All other profiles call the shared native adapter. The
GraphQL branches already use the correlation-aware `map_graphql_error` policy, while
the native branches previously returned the shared `ApiError` unchanged.

That shared compatibility error stores `ServerFn(String)`. A framework, transport,
serialization, or unexpected server-function string could therefore become the
operator-visible error after leaving an otherwise sanitized native server boundary.

## Covered operations

- `fetch_order_changes`
- `apply_order_change`
- `cancel_order_change`

Each native branch now creates `OrderChangeClientErrorContext` before the unchanged
adapter call and maps only the final returned `ApiError`.

The final public message is always:

`Commerce admin order-change request could not be completed`

## Private diagnostics

The mapper retains the original typed compatibility error only in structured tracing.
It records:

- owner operation and a per-call correlation ID;
- stable code and transport boundary;
- token presence;
- tenant slug and tenant ID presence or character length;
- order ID, order-change ID, and status presence or character length;
- action payload presence.

It does not record token, tenant slug, tenant ID, order ID, order-change ID, status,
or action-draft values.

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

The existing server-side source policy remains separately covered by
`verify-commerce-admin-order-change-native-error-safety.mjs`.

## Validation still required

Per maintainer instruction, no tests, Node verifiers, Cargo commands, formatting,
workflows, or CI were run for this slice. Maintainer execution should include the
focused client and native guards, the aggregate Commerce/ecommerce guards, hydrate
and SSR compilation, and mounted failure injection for all three operations.

This source result does not promote Commerce FFA/FBA, browser, hydrate, SSR, mounted
runtime, workflow, CI, or production status. The broad ecommerce correlation-safe
mapper cleanup remains open for other owner and non-`PortError` envelopes.
