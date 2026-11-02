# `rustok-graphql` Documentation

`rustok-graphql` is the framework-agnostic GraphQL HTTP client boundary.

## Responsibilities

- Define `GraphqlRequest`, `GraphqlResponse`, `GraphqlError` and `GraphqlHttpError`.
- Execute GraphQL HTTP requests with auth, tenant and effective-locale headers.
- Build persisted-query extension payloads.

## Non-Responsibilities

- Leptos hooks or signals.
- Dioxus hooks or context integration.
- GraphQL schema ownership.
- Native `#[server]` fallback policy.
- Automatic request retry; operation owners must provide explicit idempotency or
  query-only retry semantics when required.

## Optional owner outcome metadata

`GraphqlError.extensions` retains typed `code`, `retryable`, `outcome` metadata when present.
`execute_with_metadata` / `execute_with_client_metadata` return `GraphqlOperationError` with
that projection; legacy `execute` / `execute_with_client` preserve `GraphqlHttpError` and discard
the extra metadata. All paths retain pooled clients and no automatic retry.

Metadata alone does not authorize retry or settlement. An operation owner must recognize its
closed business code, GraphQL error kind, nonretryable flag and explicit durable outcome.
Auth/HTTP/admission/unrecognized errors remain unknown to mutation recovery. Source scenarios
were added; they have not been executed in this continuation.

## Verification

- `cargo test -p rustok-graphql --lib`
