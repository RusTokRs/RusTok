# Commerce Admin shared GraphQL transport error safety

Status: **source-ready / unvalidated**

## Boundary

Commerce Admin order-change and shipping-profile facades use the shared
`map_graphql_error` policy for GraphQL transport failures. The mapper already
returned bounded public `ApiError::Graphql` messages, but its private tracing events
recorded the complete raw compatibility error, the complete parsed
`GraphqlHttpError` or parse-failure string, and the tenant ID value.

`GraphqlHttpError::Graphql(String)` and `GraphqlHttpError::Http(String)` carry
arbitrary server or transport text. A parse failure also embeds the original value in
its error string. Those values are not safe diagnostic attributes.

## Covered callsites

The unchanged callers are:

- three order-change operations: fetch, apply, and cancel;
- seven shipping-profile operations: bootstrap, list, detail, create, update,
  deactivate, and reactivate.

Commerce Admin promotion transport is not part of this shared GraphQL boundary and
retains its separate native client error policy.

## Private diagnostics

The complete GraphQL transport error is not logged. Both warning and error events now
retain only:

- error payload presence and character length;
- whether `GraphqlHttpError::from_str` succeeded;
- parsed GraphQL or HTTP detail presence and character length;
- stable error kind and public code;
- operation and per-call correlation ID;
- tenant ID presence, character length, UUID validity, and non-nil shape;
- tenant slug presence and character length;
- consumer and transport-boundary identifiers.

They do not record raw GraphQL, HTTP, parse-failure, tenant ID, or tenant-slug text.

## Preserved behavior

This source slice preserves:

- pass-through of non-`ApiError::Graphql` variants;
- unauthorized as a warning with the authentication-required message;
- network and HTTP failures as errors with the temporarily-unavailable message;
- GraphQL rejections as warnings with the request-could-not-be-completed message;
- unknown parse failures as errors with the request-could-not-be-completed message;
- all existing public codes;
- `ApiError::Graphql(public_message.to_string())` as the public envelope;
- all order-change and shipping-profile facades, arguments, DTOs, and adapter calls;
- promotion, native server, SSR, permissions, request-context, and owner behavior.

## Evidence

- `contracts/evidence/admin-graphql-transport-error-safety-source.json`
- `contracts/evidence/admin-graphql-transport-error-safety-source-review.json`
- `scripts/verify/verify-commerce-admin-graphql-transport-error-safety.mjs`

## Validation still required

Per maintainer instruction, no tests, Node verifiers, Cargo commands, formatting,
workflows, or CI were run. Maintainer validation should include the focused verifier,
aggregate Commerce/ecommerce guards, WASM/hydrate/SSR compilation, and mounted failure
injection for representative unauthorized, network, HTTP, GraphQL rejection, and
unknown parse-failure cases.

This source result does not promote Commerce FFA/FBA, browser, hydrate, SSR, mounted
runtime, workflow, CI, or production status. The broad ecommerce correlation-safe
mapper cleanup remains open for other adapters and non-`PortError` envelopes.
