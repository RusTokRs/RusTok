# Implementation Plan for `rustok-graphql`

## Current state

`rustok-graphql` owns the framework-agnostic GraphQL HTTP client boundary:
request/response/error types, persisted-query extensions, and HTTP execution.
Currently, `execute()` instantiates a new `reqwest::Client` on each invocation,
missing connection pooling and socket reuse. More than 55 module transport
adapters duplicate the `graphql_url()` resolution logic, and transient network
retry is unconfigured or ad-hoc.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- This shared client owns neither UI nor a domain provider/consumer port. It
  must not absorb schema ownership, module query documents, DTO mapping, or
  native server-function fallback policy. It is strictly framework-agnostic.

## Open results

1. **Adopt `reqwest-middleware` and `reqwest-retry` connection pool.**
   Done when `rustok-graphql` provides a shared `ClientWithMiddleware` pipeline
   with configurable timeouts and connection reuse instead of allocating raw
   `reqwest::Client::new()` per call.
   **Depends on:** wiring workspace dependencies `reqwest-middleware` and
   `reqwest-retry`.
   **Verification:** `cargo test -p rustok-graphql --lib` proving client reuse
   and timeout mapping to `GraphqlHttpError::Timeout`.

2. **Establish idempotent GraphQL retry policy.**
   Done when transient failures (502, 503, 504, connection reset) on read-only
   GraphQL Queries are retried with exponential backoff and jitter, while
   GraphQL Mutations are strictly excluded from automatic retry to prevent
   duplicate mutations or state corruption.
   **Depends on:** custom `RetryPolicy` implementation for GraphQL operation types.
   **Verification:** unit tests for query retry and mutation non-retry.

3. **Centralize `graphql_url()` and endpoint resolution.**
   Done when a canonical `graphql_url(base_url, endpoint)` helper is exported
   by `rustok-graphql` using `url::Url` safe joining, and module transport
   adapters replace their 55+ hand-rolled copies with the shared function.
   **Depends on:** Result 1.
   **Verification:** grep search verifying elimination of duplicate
   `fn graphql_url` declarations across `crates/modules/**`.

4. **Add a Dioxus adapter only for a real host integration.**
   Done when a Dioxus host needs reactive GraphQL hooks/context and the new
   adapter keeps `rustok-graphql` framework-agnostic.
   **Depends on:** Dioxus entering the workspace with an approved consumer.
   **Verification:** targeted adapter integration tests and dependency audit.

## Verification

- `cargo test -p rustok-graphql --lib`
- `cargo check -p rustok-graphql-leptos`
- Verification of zero direct `reqwest::Client::new()` allocations in module
  GraphQL transport adapters.

## Change rules

1. Keep Leptos, Dioxus, Next.js, and `async-graphql` schema dependencies out
   of this crate.
2. Maintain parallel contracts: GraphQL is a permanent first-class transport
   alongside native server functions.
3. Update local docs and crate README whenever public client contracts change.
4. Keep module-specific query documents and DTO mapping with their owners.
