# `rustok-graphql-leptos` Documentation

This crate adapts `rustok-graphql` to Leptos reactive hooks.

## Responsibilities

- Provide `use_query`, `use_mutation` and `use_lazy_query`.
- Read host-provided `UiRouteContext.locale` for Leptos hook calls.
- Re-export `rustok-graphql` request, response and error types for hook users.

## Non-Responsibilities

- GraphQL HTTP request/response core ownership.
- Dioxus hooks.
- Module-specific query documents.

## Verification

- `cargo test -p rustok-graphql-leptos --lib`
