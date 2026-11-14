# RusToK Next Storefront

## Purpose

`apps/next-frontend` owns the Next.js storefront application for RusToK.

## Responsibilities

- Provide the React/Next storefront host for customer-facing experiences.
- Keep shared frontend transport/auth/i18n contracts aligned with the Leptos storefront.
- Organize storefront composition through `src/app`, `src/modules`, and shared integration layers.

## Entry points

- `src/app/*`
- `src/modules/*`
- `src/shared/lib/*`
- `src/shared/ui/*` (shared cross-domain UI components like `TableOfContents`)
- Next.js App Router entrypoints and layouts

## Interactions

- Uses `apps/server` as the backend/API provider.
- Works in parallel with `apps/storefront` for storefront parity at the contract level.
- Reuses shared frontend contracts instead of duplicating auth and transport logic per page.
- Consumes the canonical SEO contract from `rustok-seo` through a Next Metadata adapter while Rust hosts use `rustok-seo-render`.
- Module packages under `packages/*` read UI copy from the app-level Fluent catalogs
  `messages/{locale}.ftl`. `packages/rustok-forum` uses the `Forum` namespace (catalog ids
  `Forum-<key>`), so a copy change is made in both catalogs in one change and
  `pnpm --filter @rustok/next-frontend typegen:i18n` regenerates `src/types/i18n.d.ts`.
- `packages/rustok-forum` shows the owner topic status token through a localized
  `TopicStatusLabel` badge, and unrecognized tokens stay verbatim. It exposes no vote,
  subscribe, or report mutation, so those owner capabilities remain unreachable in this surface.
- `packages/rustok-forum` takes topic and reply page sizes from the forum module's
  `topics_per_page` / `replies_per_page` settings by omitting `perPage` on the GraphQL storefront
  queries, so a tenant that configured a page size is not overridden by the package. A feed shows one
  server page and offers no paging control. A GraphQL-level failure (denied field, validation,
  authorization) is reported in a localized banner with a retry action instead of rendering as an
  empty forum or a missing topic. The shared Apollo 4 adapter maps both GraphQL and non-GraphQL
  `result.error` values into `GraphqlResponse.errors`; exceptions thrown by the transport also
  propagate. Each caller remains responsible for displaying an appropriate failure state.
- `packages/rustok-forum` opens the post a link addressed: `?reply=<uuid>` on `/modules/forum`
  (notification and mention links) or on the canonical topic route (search results) scrolls to that
  reply and marks it, paging forward through the owner cursor within 20 pages, and the surface says
  so when the post is outside what loaded. The topic page therefore lives at the owner's canonical
  prefix `src/app/[locale]/forum/t/[...slug]`: the owner publishes that path in search documents, SEO
  targets and canonical redirects, so mounting it under `modules/` instead 404s every such link.
- Composes search storefront controls through host-owned `src/features/search`: product-owned catalog metadata is loaded via `packages/rustok-product` and passed to the search package as safe option props with the route locale.
- `packages/rustok-pages` (`@rustok/pages-frontend`) renders published Page Builder landings directly with scoped CSS and sanitized HTML. `FlyActionBridge` confines host actions to the landing, requests Cart controls only when enabled and navigates purchases to the Product buyer instead of calling `addItem` without a quote. Refreshed enabled-module props take effect before child components commit; a new authoritative prop also resets local overrides even when its values are unchanged. The [package contract](packages/rustok-pages/README.md#shared-purchase-handoff) describes the shared Next/Leptos payload. The Pages route is `src/app/[locale]/pages/[slug]/page.tsx`.

## Fly browser checks

`npm run test:e2e:fly` runs the focused asset and mounted-component contracts. The separate
`npm run test:e2e:fly-host` builds the production Next app and exercises landing SSR/hydration,
Cart controls, Product handoff and confirmation, and back navigation in Chromium against a fake
local API. Install Playwright's Chromium first, or set `PLAYWRIGHT_CHROMIUM_EXECUTABLE` to a
compatible browser. The host check rebuilds `.next`; do not run it concurrently with another build
or dev server. It uses ports 3311 and 3301 by default; override them with `FLY_HOST_API_PORT` and
`FLY_HOST_NEXT_PORT` if needed. It does not test the real backend, deployment, or CSP.

The workspace JSX type bridge in `src/types/react-jsx-runtime.d.ts` exposes the installed React
types to packages outside this app's `node_modules`. Keep `react/jsx-runtime` itself unaliased in
`tsconfig.json`: pointing that runtime specifier at a declaration file can build successfully but
produce a broken browser bundle.

## Docs

- [App docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)

## Dependency verification — 2026-10-17

Use the checked-in npm lockfile: `npm ci`, `npm run typecheck`, `npm run lint`.
Build/verify `packages/richtext` with its own npm lock before using its shared
frame assets. Next is pinned to 16.4.0 after the latest-main reconciliation; React/React DOM and their types share
19.3.0, ESLint is 9.39.4 and TypeScript is 6.0.3. Tiptap's shared package uses
one 3.30.5 cohort. No `--force` or legacy peer install flags are required, and
Next build-time type errors are no longer ignored. Full host typecheck/lint and
scoped Forum tests pass on the installed graph; browser/runtime acceptance is
separate. See the central richtext plan for dependency evidence and limits.
