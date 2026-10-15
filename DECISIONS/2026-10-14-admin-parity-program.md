# Both admins stay: functional parity over the shared GraphQL

- Date: 2026-10-14
- Decision status: Accepted
- Implementation status: In progress
- Owners: `apps/admin`, `apps/next-admin`
- Extends: 2026-10-14-pages-admin-package-for-next-admin.md
- Supersedes: None
- Superseded by: None

## Context

RusToK historically maintained two admin interfaces: the Rust Fluid Frontend Architecture (FFA)
admin (`apps/admin`) and the classic Next.js admin (`apps/next-admin`). There was ambiguity about
whether one would replace the other. Functional parity between the two applications was uneven,
with modules implemented in one admin but missing from the other.

## Decision

RustTok keeps two admins — the Rust FFA admin (single Leptos connector,
`apps/admin`) and the classic Next admin (`apps/next-admin`). Neither replaces
the other. Functional consistency is defined as: every user-facing module
capability is reachable in both admins through the **module-owned** UI of that
admin (Rust FFA packages in `crates/modules/*/admin`, Next packages in
`apps/next-admin/packages/*`), over the **same GraphQL schema and permissions**.
`docs/modules/admin-parity.md` is the parity matrix and update contract.

First steps (2026-10-14):
After `@rustok/pages-admin`, the same pattern lands `@rustok/media-admin`
(library list/usage/delete — the assets builder pages reference) and
`@rustok/forms-admin` (the submission inbox with the `new/read/handled/spam`
triage machine), wired identically: host dependency, module registration in
`src/modules/index.ts`, and a session-fed `/dashboard/<module>` route.

## Sources of truth and ownership

- Canonical admin parity matrix: `docs/modules/admin-parity.md`.
- Rust admin composition: `apps/admin`.
- Next admin composition: `apps/next-admin`.
- Shared backend capabilities: Module-owned GraphQL endpoints (`crates/modules/*`).

## Invariants

- Both admins operate against the identical backend GraphQL schema and permissions model.
- No admin-private endpoints: any administrative capability required by either admin must be exposed via a canonical module GraphQL operation.
- Deep-linked asymmetry is allowed and documented per surface (e.g. the visual Page Builder remains hosted in Rust FFA, while Next admin manages pages and links into it).

## Non-goals

- Forcing identical component implementations or sharing rendering code across Leptos and React.
- Deprecating or eliminating either admin application.

## Data, transaction, and concurrency boundary

- Both frontends execute mutations against backend GraphQL services that own database transactions.
- Concurrency tokens and optimistic locking apply equally to operations originating from either admin.

## Context dimensions

- Tenant, principal role, and permissions are authenticated via canonical session tokens and propagated across GraphQL requests.
- Localization is negotiated per-session.

## Events and projections

- Backend administrative operations triggered by either UI emit canonical audit and domain events through transactional outboxes.

## Failure semantics

- GraphQL network and execution errors are rendered via framework-idiomatic error boundaries and toast notifications.

## Migration and cutover

- Modules missing Next.js UI packages are systematically closed according to the priority matrix in `docs/modules/admin-parity.md`.

## Alternatives considered

- Single-admin consolidation: rejected to support both high-performance compiled Rust operators and classic Next.js enterprise deployments.
- REST adapters for Next admin: rejected in favor of GraphQL schema parity.

## Verification

TypeScript syntax checks, GraphQL schema introspection verification, and module catalog parity sweeps.

## Consequences

- Operator capability parity is measurable and enforceable across both admin distributions.
- Backend modules maintain a single, cohesive public GraphQL surface.
