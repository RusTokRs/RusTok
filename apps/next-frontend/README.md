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
  empty forum or a missing topic. A transport failure stays indistinguishable from an empty result:
  `src/shared/lib/graphql.ts` maps only `graphQLErrors` into `GraphqlResponse.errors` and drops
  `networkError`, so surfacing connection failures is an app-shell change that affects every module
  package and needs its own decision.
- `packages/rustok-forum` opens the post a link addressed: `?reply=<uuid>` on `/modules/forum`
  (notification and mention links) or on the canonical topic route (search results) scrolls to that
  reply and marks it, paging forward through the owner cursor within 20 pages, and the surface says
  so when the post is outside what loaded. The topic page therefore lives at the owner's canonical
  prefix `src/app/[locale]/forum/t/[...slug]`: the owner publishes that path in search documents, SEO
  targets and canonical redirects, so mounting it under `modules/` instead 404s every such link.
- Composes search storefront controls through host-owned `src/features/search`: product-owned catalog metadata is loaded via `packages/rustok-product` and passed to the search package as safe option props with the route locale.
- `packages/rustok-pages` (`@rustok/pages-frontend`) renders published Page Builder landing pages directly through Server Components (`PageBuilderContent`) with inline scoped `<style>` and sanitized HTML without an `<iframe>`. Interactive actions (`data-fly-action`) are handled by `FlyActionBridge` (Islands Architecture), bridging custom `fly:action` events into `@rustok/cart-frontend` (`addItem`/`openCart`) and Next.js client-side navigation. Canonical route lives at `src/app/[locale]/pages/[slug]/page.tsx` with dynamic SEO metadata resolution (`resolveSeoPageContextForRoute`).

## Docs

- [App docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)

## Dependency verification — 2026-10-17

Use the checked-in npm lockfile: `npm ci`, `npm run typecheck`, `npm run lint`.
Build/verify `packages/richtext` with its own npm lock before using its shared
frame assets. Next is pinned to 16.3.8; React/React DOM and their types share
19.3.0, ESLint is 9.39.4 and TypeScript is 6.0.3. Tiptap's shared package uses
one 3.30.5 cohort. No `--force` or legacy peer install flags are required, and
Next build-time type errors are no longer ignored. Full host typecheck/lint and
scoped Forum tests pass on the installed graph; browser/runtime acceptance is
separate. See the central richtext plan for dependency evidence and limits.
