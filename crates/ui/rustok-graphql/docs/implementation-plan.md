# Implementation Plan for `rustok-graphql`

## Current state

`rustok-graphql` owns the framework-agnostic GraphQL HTTP client boundary:
request/response/error types, persisted-query extensions, and HTTP execution.
`execute()` uses one pooled `reqwest::Client` on native and browser targets.
Automatic middleware retry is intentionally absent because the shared HTTP
layer cannot safely infer whether a GraphQL `POST` contains an idempotent query
or a mutation. More than 55 module transport adapters still duplicate endpoint
resolution logic.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- This shared client owns neither UI nor a domain provider/consumer port. It
  must not absorb schema ownership, module query documents, DTO mapping, or
  native server-function fallback policy. It is strictly framework-agnostic.

## Open results

1. **Maintain one native/browser-safe pooled client.**
   The shared `ClientWithMiddleware` pipeline has configurable timeout and
   connection reuse without allocating a client per call. Client construction
   failure uses the existing fail-closed `GraphqlHttpError::Network` contract
   rather than panicking or expanding the public error vocabulary.
   **Verification:** `cargo test -p rustok-graphql --lib` and
   `cargo check -p rustok-graphql --target wasm32-unknown-unknown`.

2. **Keep retries owner-explicit and idempotency-aware.**
   The generic client must not automatically retry GraphQL `POST` requests.
   A future query-only retry API may be added only with an explicit operation
   classification and tests proving mutations are never retried.
   **Verification:** dependency and source checks proving no generic retry
   middleware wraps all GraphQL operations.

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
