# SSR-first Leptos hosts with headless parity

- Date: 2026-04-24
- Status: Accepted

## Context

RusTok supports several runtime shapes:

- monolith deployments where `apps/server` embeds Leptos admin and storefront hosts;
- headless deployments where Next.js hosts and external clients use public API contracts;
- local debug profiles where Leptos UI packages are compiled as standalone CSR/WASM through Trunk.

The platform previously mixed two different goals: using Leptos `#[server]` functions as the preferred internal monolith data layer, and keeping standalone CSR builds working for local debugging. Treating CSR as the product default conflicts with the monolith goal, while treating `#[server]` as the only transport breaks headless clients and standalone module UI debugging.

## Decision

RusTok uses SSR-first Leptos hosts for product runtime:

- `apps/admin` and `apps/storefront` target SSR + hydrate as the preferred Leptos runtime for monolith deployments.
- Native Leptos `#[server]` functions are the preferred internal transport in SSR/hydrate/monolith profiles.
- GraphQL and REST remain mandatory public headless contracts and must not be removed when `#[server]` paths exist.
- Module-owned Leptos UI packages must remain CSR-capable for standalone debug and compatibility, but CSR is not the product architecture default.
- In standalone CSR, module-owned UI packages must use GraphQL/REST fallback paths and must not require `/api/fn/*`.
- Next.js hosts and external clients use GraphQL/REST and do not depend on Leptos server-function runtime.

## Rationale

This split was chosen because it matches the platform shape instead of optimizing for one host:

- SSR/hydrate is the right product default for monolith deployments: admin and storefront can run on the same origin as `apps/server`, reuse server-side auth/session/policy, avoid extra CORS/proxy assumptions, and give the storefront a better first render and SEO baseline.
- Leptos `#[server]` functions are the shortest internal Rust path for monolith UI: they can call the service layer directly without turning every internal admin/storefront action into a public GraphQL mutation.
- GraphQL/REST cannot be removed because headless is a real product mode: Next.js hosts, external clients, integrations and mobile clients need stable public contracts that do not depend on Leptos runtime.
- CSR/Trunk still has value as a debug and compatibility profile: it catches server-only dependencies leaking into module-owned UI packages and lets developers run a package locally against `apps/server`.
- The rule applies to module-owned UI packages, not only to `apps/admin` and `apps/storefront`, because admin and storefront UI is owned by modules under `crates/*/{admin,storefront}` and only mounted by host apps.

## Consequences

- New Leptos admin/storefront work should be designed native-first for SSR/hydrate, with GraphQL/REST parity where the surface is headless-relevant or CSR-debuggable.
- Module-owned admin and storefront packages must compile without server-only dependencies in CSR profiles.
- `#[server]` endpoints and `/api/graphql` are parallel surfaces; either one can be preferred by a runtime profile, but neither cancels the other.
- Local `trunk serve` is a debug profile and must be documented as such, not as the canonical production path.
- CI and verification should keep separate checks for server/SSR and wasm CSR compatibility where affected UI packages are touched.
