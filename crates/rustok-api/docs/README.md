# rustok-api docs

This folder contains the local documentation for `crates/rustok-api`.

## Scope

`rustok-api` is the shared API adapter layer that sits between `rustok-core` and application-specific wiring in `apps/server`. It owns reusable request/auth/tenant/GraphQL primitives, while module-specific resolvers and controllers are migrated into module crates incrementally.

## Architectural Boundary

- `rustok-api` stays intentionally thin.
- It is the single shared host/API layer for request, tenant, auth, channel, and GraphQL helper contracts.
- It now also owns the typed `ChannelResolutionSource` contract that lets hosts surface why a request was attached to a specific channel, including the reserved `Policy` branch for the next `rustok-channel` resolution phase.
- Generic UI host contracts such as `UiRouteContext` may live here too, as long as they stay module-agnostic and do not pull module behavior into the shared layer. Today this includes shared route segment, nested subpath, locale, query param context, and locale-aware module route helpers for module-owned Leptos UI.
- Do not introduce a second parallel implementation of the same layer inside `apps/server` or in per-module helper crates.
- If a helper is module-specific, keep it inside that module. If it becomes a shared host contract, move it into `rustok-api`.
