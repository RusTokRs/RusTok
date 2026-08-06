# GraphQL query diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This boundary protects the Commerce GraphQL safe-query resolver source without editing the resolver file itself.

The original diagnostic slice converted owned `String` failures and typed Commerce, Fulfillment, Order, Payment, and database failures into stable public GraphQL envelopes. A separate borrowed-message compatibility path still accepted `&str` and constructed a raw `async_graphql::Error`, bypassing the boundary code and retryability extensions.

## Bounded diagnostics

The boundary uses one diagnostic error type whose `Debug` output is always `redacted`.

The seven diagnostic projection points now cover:

- owned `String` compatibility errors;
- borrowed `&str` compatibility errors;
- SeaORM database errors;
- Commerce owner errors;
- Fulfillment owner errors;
- Order owner errors;
- Payment owner errors.

Owned and borrowed messages are represented only by `empty` / `present` plus their byte length. Message content, validation details, database causes, owner internals, provider details, transition details, and resource identifiers are not emitted by this boundary.

Borrowed messages now return:

- message: `Commerce query could not be completed safely`;
- code: `COMMERCE_QUERY_OPERATION_FAILED`;
- retryable: `false`.

## Preserved GraphQL contract

This work does not change:

- `BoundaryError::Graphql` pass-through for an already constructed typed `async_graphql::Error`;
- `impl From<Error> for BoundaryError`;
- authentication, permission, module, tenant, and channel errors constructed by the shared GraphQL helpers;
- Commerce, Fulfillment, Order, Payment, and database public policies;
- temporary-unavailable retryability;
- non-retryable payment reconciliation;
- product-service error mapper delegation;
- resolver source inclusion or resolver behavior.

Only direct construction through `BoundaryError::new(&str)` changes. It can no longer bypass the stable public envelope.

## Static guards

The existing broad verifier now isolates the borrowed mapper separately and requires:

- message presence and length projection;
- the borrowed source owner and error kind;
- redacted diagnostic ordering;
- the stable message, code, and retryability;
- absence of `BoundaryError::Graphql(Error::new(self))` and raw message logging;
- seven diagnostic shadows and seven redacted error fields;
- continued typed GraphQL pass-through.

A focused verifier additionally checks the exact borrowed mapper and confirms that the unchanged resolver source has no direct borrowed literal constructor site.

The verifiers were updated or added but were not executed.

## Remaining boundary

The conversion traits still do not receive request-scoped tenant, actor, channel, locale, correlation, or deadline context. Adding truthful query context without duplicating resolver policy remains open.

The shared storefront HTTP mappers, inventory, customer, tax, promotion, native transport, remaining adapters, and the broad ecommerce correlation-safe cleanup also remain open.

## Evidence

- `crates/rustok-commerce/contracts/evidence/graphql-query-diagnostic-safety-source-review.json`
- `scripts/verify/verify-commerce-graphql-query-error-boundary.mjs`
- `scripts/verify/verify-commerce-graphql-borrowed-message-envelope-safety.mjs`

## Validation disclosure

No tests, Node verifiers, formatting, Cargo commands, mounted GraphQL scenarios, workflows, or CI were run. No compile or runtime status is claimed.
