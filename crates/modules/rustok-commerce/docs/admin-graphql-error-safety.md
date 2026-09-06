# Commerce admin GraphQL error safety

Date: 2026-07-30

Status: `commerce_admin_graphql_error_safety_source_unvalidated`

## Problem

The Commerce admin package has a private low-level GraphQL adapter shared by shipping-profile
screens and the explicit browser-only order-change fallback. That adapter receives the typed
`rustok_graphql::GraphqlHttpError`, converts it to its display string, and previously allowed the
public transport wrappers to return that raw text through `ApiError::Graphql`.

The raw value can contain HTTP status detail, GraphQL resolver messages, decoding/fallback detail,
or other implementation-specific text. It must remain diagnostic context rather than become the
stable UI envelope.

## Covered public operations

Shipping-profile and bootstrap wrappers:

- `fetch_bootstrap`
- `fetch_shipping_profiles`
- `fetch_shipping_profile`
- `create_shipping_profile`
- `update_shipping_profile`
- `deactivate_shipping_profile`
- `reactivate_shipping_profile`

Explicit browser GraphQL order-change fallback:

- `fetch_order_changes`
- `apply_order_change`
- `cancel_order_change`

The native order-change path remains selected everywhere it was selected before this change.

## Public envelopes

The wrapper reparses the private adapter display value as `GraphqlHttpError` and maps it to a
stable public message:

| Typed outcome | Public message |
| --- | --- |
| `Unauthorized` | `Commerce admin authentication is required` |
| `Network` | `Commerce admin service is temporarily unavailable` |
| `Http(_)` | `Commerce admin service is temporarily unavailable` |
| `Graphql(_)` | `Commerce admin request could not be completed` |
| unrecognized display value | `Commerce admin request could not be completed` |

The existing `ApiError::Graphql(String)` shape is preserved. Only its public message is sanitized.

## Internal diagnostics

Every GraphQL wrapper creates a unique correlation ID before the request. Failure diagnostics
retain:

- the raw private adapter error;
- parsed `GraphqlHttpError` outcome when recognized;
- consumer and operation;
- correlation ID;
- tenant ID when the operation already has it;
- tenant-slug presence and length, but not the slug value;
- stable public code and error kind;
- the `commerce_admin_graphql_transport` boundary.

Bearer tokens are never passed to the mapper or logged. Request payloads, shipping-profile drafts,
search text, metadata, order-change drafts, and GraphQL variables are not logged.

Network, HTTP, and unknown failures log at error level. Authentication and GraphQL rejection
outcomes log at warning level.

## Preserved behavior

This source slice does not change:

- GraphQL endpoint resolution;
- query or mutation documents;
- variables and tenant headers;
- bearer-token forwarding;
- DTO serialization/deserialization;
- shipping-profile public function signatures;
- order-change public function signatures;
- explicit GraphQL/native transport selection;
- native order-change admission, permissions, owner mapping, or public envelopes.

`uuid` is now a transport-neutral package dependency because correlation IDs are created in both
browser GraphQL and SSR/native builds.

## Validation boundary

Added source guard:

```bash
node scripts/verify/verify-commerce-admin-graphql-error-safety.mjs
```

Suggested owner validation:

```bash
node scripts/verify/verify-commerce-admin-graphql-error-safety.mjs
node scripts/verify/verify-commerce-admin-order-change-native-error-safety.mjs
node scripts/verify/verify-commerce-admin-boundary.mjs
cargo check -p rustok-commerce-admin
cargo check -p rustok-commerce-admin --features hydrate
cargo check -p rustok-commerce-admin --features ssr
```

None of these commands, tests, browser scenarios, SSR scenarios, workflows, or CI jobs were run by
the implementation agent. The focused source guard is present but unexecuted. No FFA, FBA,
browser, SSR, transport, runtime, or production status is promoted.

The ecommerce master mapper-cleanup item remains open for other adapters and non-`PortError`
envelopes.
