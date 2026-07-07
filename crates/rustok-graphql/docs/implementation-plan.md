# `rustok-graphql` Implementation Plan

## Focus

Keep `rustok-graphql` as the canonical framework-agnostic GraphQL HTTP client
boundary for Leptos, future Dioxus adapters and host/shared transport code.

## Current State

- The crate owns GraphQL request/response/error types, persisted-query extension
  payloads and HTTP execution.
- Current module GraphQL transport adapters import `rustok_graphql` directly.
- Leptos reactive hooks live in the sibling `rustok-graphql-leptos` adapter crate.

## Improvements

- Add `rustok-graphql-dioxus` only when Dioxus enters the workspace and a real
  Dioxus hook/context integration is needed.
- Keep verification strict so new transport adapters do not reintroduce raw HTTP
  clients or framework-specific GraphQL core dependencies.

## Non-Goals

- No Leptos, Dioxus, Next.js or `async-graphql` schema ownership.
- No native `#[server]` fallback policy.
- No module-specific query documents or DTO mapping.
