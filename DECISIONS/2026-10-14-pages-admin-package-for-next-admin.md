# Pages module UI for the classic Next admin (`@rustok/pages-admin`)

- Date: 2026-10-14
- Decision status: Accepted
- Implementation status: Implemented
- Owners: `apps/next-admin`, `rustok-pages`
- Extends: 2026-10-14-admin-parity-program.md
- Supersedes: None
- Superseded by: None

## Context

The platform model, restated by the product owner: the Rust admin is FFA with a
single connector (Leptos); the Next admin is a classic application; **both
consume the same GraphQL**, and **each module keeps its own UI**. The builder's
UI therefore lives in the builder/pages modules themselves — on the Rust side
that is `rustok-pages-admin` + `rustok-page-builder-admin`, already composed by
`apps/admin` through module routes and the intent endpoint.

The Next side had no equivalent: module packages such as `@rustok/blog-admin`
and `@rustok/commerce-admin` register their navigation and render their surfaces
through the shared GraphQL, but there was no `@rustok/pages-admin` at all — the
Pages module (and with it the builder) was simply absent from the Next admin,
even though `rustok-pages` exposes a complete GraphQL surface (`pages`,
`createPage`, `savePageDocument`, `publishPage`, `unpublishPage`, …).

## Decision

1. **Add the module-owned package `@rustok/pages-admin`** under
   `apps/next-admin/packages/pages`, mirroring the existing module packages: it
   registers its navigation through the host module registry, keeps its GraphQL
   client in `src/api/pages.ts` (schema-faithful camelCase operations), and
   exports a `PagesPage` surface (list, create draft, unpublish, delete). The
   host only wires it: a `file:` dependency, one registry import, and the
   `/dashboard/pages` route passing session token/tenant through.
2. **No second visual editor, no fake publish.** The builder UI is the Rust
   module's own UI; this surface deep-links to it per page ("Open in builder",
   `/modules/pages?pageId=…`, origin from `NEXT_PUBLIC_RUST_ADMIN_URL`, hidden
   when unconfigured). Publishing is not offered from the list: publish requires
   the builder's reviewed runtime (`expected_body_revisions`, `review_hash`),
   and `createPage` rejects `publish: true` server-side. Unpublish and delete
   are plain GraphQL mutations and are offered.
3. **GraphQL is the only transport**, as with every other module package; no
   REST calls, no server-function coupling to the Rust admin's monolith intent
   endpoint. The two admins stay symmetric at the contract level while each
   keeps its own framework-idiomatic UI.

## Sources of truth and ownership

- Canonical owner of Pages backend and GraphQL contract: `rustok-pages`.
- Canonical owner of the Next Pages admin package: `apps/next-admin/packages/pages`.
- Canonical visual builder: `rustok-page-builder-admin` in `apps/admin`.

## Invariants

- Next admin packages consume the backend strictly through GraphQL operations.
- Direct database or internal server function calls from Next admin packages are forbidden.
- Pages publication requires reviewed runtime evidence and cannot be bypassed from the list view.

## Non-goals

- Implementing a second React/TypeScript visual builder in Next admin.
- Bypassing the server GraphQL schema with custom REST routes.

## Data, transaction, and concurrency boundary

- Mutations execute against the authoritative `rustok-pages` GraphQL API inside backend transaction boundaries.
- Concurrent updates to page drafts utilize optimistic revision checks (`expected_revision`).

## Context dimensions

- Tenant isolation is propagated through GraphQL request headers and session tokens.
- Language/locale is resolved from the active session context.

## Events and projections

- Page lifecycle operations (create, unpublish, delete) emit canonical domain events through the transactional outbox in `rustok-pages`.

## Failure semantics

- GraphQL errors map to typed user-facing problem diagnostics in the UI package.
- Network and authentication errors trigger standard session revalidation without data loss.

## Migration and cutover

- `@rustok/pages-admin` is registered as a peer module package alongside `@rustok/blog-admin` and `@rustok/commerce-admin`.
- Existing Rust admin Pages functionality remains unchanged.

## Alternatives considered

- Duplicating the full visual builder in React: rejected as violating the single-builder owner model and causing massive feature divergence.
- Omitting Pages from Next admin: rejected by the admin parity mandate.

## Verification

TypeScript transpile checks (`tsc --noEmit`), module package registration verification, and GraphQL operation parity validation against `rustok-pages` schema.

## Consequences

- Next admin achieves functional management parity for the Pages module.
- Visual page design remains cleanly delegated to the canonical Page Builder.
