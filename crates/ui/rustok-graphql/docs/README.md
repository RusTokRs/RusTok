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

## Verification

- `cargo test -p rustok-graphql --lib`
