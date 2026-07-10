# Implementation Plan for `rustok-graphql`

## Current state

`rustok-graphql` owns the framework-agnostic GraphQL HTTP client boundary:
request/response/error types, persisted-query extensions, and HTTP execution.
Module transport adapters use this crate directly. Leptos reactivity lives in
the separate `rustok-graphql-leptos` adapter crate.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- This shared client owns neither UI nor a domain provider/consumer port. It
  must not absorb schema ownership, module query documents, DTO mapping, or
  native server-function fallback policy.

## Open results

1. **Keep the GraphQL client boundary framework-neutral.** Done when new
   adapters reuse the shared request, error, and persisted-query contracts
   without adding raw HTTP clients or framework dependencies to this crate.
   **Depends on:** a concrete adapter change. **Verification:**
   `cargo test -p rustok-graphql --lib` and the changed adapter's focused test.
2. **Add a Dioxus adapter only for a real host integration.** Done when a
   Dioxus host needs reactive GraphQL hooks/context and the new adapter keeps
   `rustok-graphql` framework-agnostic.
   **Depends on:** Dioxus entering the workspace with an approved consumer.
   **Verification:** targeted adapter integration tests and a dependency audit.

## Verification

- `cargo test -p rustok-graphql --lib`
- Review dependency direction when changing GraphQL transport contracts.

## Change rules

1. Keep Leptos, Dioxus, Next.js, and `async-graphql` schema dependencies out
   of this crate.
2. Update the root README and local docs with a public client-contract change.
3. Keep module-specific query documents and DTO mapping with their owners.
