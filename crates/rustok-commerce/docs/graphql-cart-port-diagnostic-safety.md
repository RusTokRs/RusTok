# GraphQL cart port diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens `cart_port_error` in the Commerce GraphQL storefront cart helper facade.

The mapper accepts `PortError` values originating from the Cart or Pricing owner boundary. It already selected stable public GraphQL messages, codes, and retryability from `PortErrorKind`, but its diagnostic event emitted the complete owner error, including its message.

## Bounded projection

The mapper now completes public policy selection and source-owner classification before replacing the original owner error with `StorefrontCartPortDiagnosticError`.

The diagnostic object retains only:

- exact owner code;
- typed owner kind;
- owner retryability;
- owner-message `empty` / `present` shape;
- owner-message byte length.

Its custom `Debug` implementation always emits `redacted`, so `error = ?error` no longer serializes the original Cart/Pricing `PortError`.

The event continues to retain the static Commerce boundary owner, classified source owner, operation, public code, public retryability, shared boundary, and static event message.

## Preserved GraphQL behavior

This work does not change:

- Cart/Pricing source-owner classification from the stable code prefix;
- validation, not-found, conflict, forbidden, unavailable/timeout, or invariant public policy;
- public GraphQL messages;
- public GraphQL codes;
- public retryability;
- the existing error-level diagnostic severity;
- the `public_graphql_error` envelope;
- the already-hardened customer mapper;
- helper routing or owner calls.

The existing broad cart-helper verifier markers remain valid because `owner_code`, typed `owner_kind`, owner retryability, and source-owner classification are still present, but they now refer to bounded facts rather than the original error payload.

## Remaining helper work

This slice does not close the complete storefront cart helper diagnostic boundary.

Still open:

- `legacy_graphql_error`, which logs the complete GraphQL error and raw tenant/resource UUIDs;
- typed line-item diagnostics, which log typed source payloads and raw tenant/product/variant context;
- storefront shared and cart-shipping mappers;
- tax, promotion, native transport, and remaining owner-adapter cleanup.

## Evidence

- `crates/rustok-commerce/contracts/evidence/graphql-cart-port-diagnostic-safety-source-review.json`
- `scripts/verify/verify-commerce-graphql-cart-port-diagnostic-safety.mjs`
- `scripts/verify/verify-commerce-graphql-cart-helper-error-safety.mjs`

## Validation disclosure

No tests, Node verifiers, formatting, Cargo commands, mounted GraphQL scenarios, workflows, or CI were run. No compile, runtime, FFA, or FBA status is promoted.
