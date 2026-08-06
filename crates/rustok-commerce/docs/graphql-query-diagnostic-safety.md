# GraphQL query diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens the Commerce GraphQL safe-query error boundary used by the unchanged query resolver source.

The boundary already converted dynamic resolver failures and typed Commerce, Fulfillment, Order, Payment, and database errors into stable public GraphQL messages, codes, and retryability extensions. Its diagnostic events still emitted dynamic strings and complete typed owner/database errors.

## Bounded diagnostics

The boundary now uses one diagnostic error type whose `Debug` output is always `redacted`.

Typed errors remain available until the existing public policy is selected. Only after policy selection does each mapper shadow the original value before `tracing::error!`.

The six diagnostic projection points cover:

- dynamic `String` compatibility errors;
- SeaORM database errors;
- Commerce owner errors;
- Fulfillment owner errors;
- Order owner errors;
- Payment owner errors.

Dynamic strings are represented only by `empty` / `present` and their byte length. Message content, validation details, database causes, owner internals, provider details, transition details, and resource identifiers are not emitted by this boundary.

## Preserved GraphQL contract

This work does not change:

- `BoundaryError::Graphql` pass-through;
- static `&str` GraphQL construction;
- public messages;
- extension codes;
- retryability values;
- product-service error mapper delegation;
- resolver source inclusion or resolver behavior;
- authentication, permission, module, tenant, or channel helpers.

Temporary-unavailable policies remain retryable. Payment reconciliation remains non-retryable.

## Remaining boundary

The current conversion traits do not receive request-scoped tenant, actor, channel, locale, correlation, or deadline context. Adding truthful GraphQL query context without duplicating resolver policy remains open.

The broader ecommerce correlation-safe mapper, inventory, customer, tax, promotion, storefront, native transport, and non-`PortError` cleanup also remains open.

## Evidence

- `crates/rustok-commerce/contracts/evidence/graphql-query-diagnostic-safety-source-review.json`
- `scripts/verify/verify-commerce-graphql-query-error-boundary.mjs`

## Validation disclosure

No tests, Node verifiers, formatting, Cargo commands, mounted GraphQL scenarios, workflows, or CI were run. No compile, runtime, FFA, or FBA status is promoted.
