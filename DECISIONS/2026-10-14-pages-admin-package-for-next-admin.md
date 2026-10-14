# Pages module UI for the classic Next admin (`@rustok/pages-admin`)

Date: 2026-10-14
Status: accepted
Scope: `apps/next-admin` (new `packages/pages`, host wiring), Pages module
platform connection.

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

## Decisions

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

## Verification

No browser, full Next build, or Rust toolchain is available in this sandbox, so
`next build`/`tsc --noEmit` over the app and `cargo` were not run. What was run:
TypeScript transpile-level syntax checks (TypeScript 5, `transpileModule`) over
all new files — clean; JSON validation of `package.json`/`package-lock.json`
(the lock entries for the workspace link were added by hand in npm's shape;
maintainers should confirm with one `npm install`). The GraphQL operations were
written field-by-field from `crates/modules/rustok-pages/src/graphql` and
cross-checked against neighbouring clients (`@rustok/blog-admin`) for
conventions (`UUID`, camelCase, tenant handling).
