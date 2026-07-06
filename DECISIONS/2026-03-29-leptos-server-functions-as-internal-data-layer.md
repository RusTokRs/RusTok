# Leptos `#[server]` functions as the internal data layer for Leptos applications

- Date: 2026-03-29
- Status: Accepted, amended on 2026-04-02
- Supersedes: `2026-03-07-deployment-profiles-and-ui-stack.md` (in part regarding transport between Leptos UI and server)

## Context

The RusToK Leptos path now supports two transport paths simultaneously:

- GraphQL HTTP (`/api/graphql`);
- Leptos server functions (`/api/fn/*`).

The initial ADR formulation was too strict and treated `#[server]`
as a complete replacement for GraphQL for Leptos UI. In fact, the code and platform rule went in
a different direction: the native path is added **in parallel**, while GraphQL remains
a live transport contract.

This is needed for three reasons:

1. GraphQL is already an external contract for Next.js, mobile clients, and integrations.
2. Migration of Leptos host applications and module-owned UI crates happens in stages, coverage is not complete everywhere.
3. Even after introducing the native path, the platform does not want to lose GraphQL as a compatible transport and fallback.

## Decision

### Principle

**Leptos `#[server]` functions become the primary internal data-layer for
Leptos UI, but do not replace GraphQL at the platform level.**

That is:

- for `apps/admin`, `apps/storefront` and module-owned Leptos UI packages, the default path is:

```text
UI -> local api -> #[server] -> service layer -> DB
```

- GraphQL remains a mandatory parallel transport:

```text
client -> /api/graphql -> GraphQL resolver -> service layer -> DB
```

### What this means for Leptos UI

#### Monolith / SSR

```text
HTTP request -> Axum -> Leptos SSR -> #[server] fn -> service layer -> DB
```

In many SSR scenarios this is an in-process path without a GraphQL resolver layer.

#### Hydration / client navigation / standalone Leptos

```text
browser -> POST /api/fn/* -> server -> service layer -> DB
```

This is the native Leptos transport via `leptos_axum`.

#### Parallel GraphQL path

GraphQL remains:

- the external API for `apps/next-admin`, `apps/next-frontend`, mobile and integrations;
- a fallback branch for Leptos UI where native coverage is not yet complete;
- a transport surface for older modules and persisted-query scenarios.

### Rule for new modules

If a new module ships a Leptos UI:

- it must not be designed as a GraphQL-only data path if a native `#[server]` layer is possible;
- it must add a local API boundary and native-first calls;
- the GraphQL query/mutation path must not be removed if it already exists or is needed for external clients.

## Deployment consequences

### Monolith

```text
browser -> Axum -> Leptos SSR -> #[server] fn -> service layer -> DB
```

The native path minimizes internal transport overhead, but `/api/graphql`
remains up and accessible.

### Headless

```text
Leptos -> POST /api/fn/*
Next.js / external clients -> POST /api/graphql
```

Both transport surfaces live side by side.

## Consequences

### Positive

- Monolith gets a short native data path for Leptos UI.
- GraphQL is not lost as a public contract.
- Migration of host applications and module-owned UI crates is possible incrementally.
- The same module can serve both Leptos UI and external headless clients.

### Negative

- Dual-path documentation and verification must be maintained.
- Leptos code needs an explicit local API layer, rather than direct transport calls from the view.
- Old GraphQL operations cannot be blindly removed, even if `#[server]` already exists alongside them.

## Follow-up

1. Documentation must everywhere record the dual-path rule: native `#[server]` first, GraphQL parallel.
2. `apps/server` must keep both `/api/graphql` and `/api/fn/*`.
3. New module-owned Leptos UI crates must follow the same scheme.
4. Verification plans must check not only GraphQL, but also Leptos server functions transport.
