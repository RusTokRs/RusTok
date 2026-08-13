# Next Storefront App — Implementation Plan

## Focus

Develop `apps/next-frontend` as the primary Next.js storefront with clear API/UI contracts, observable performance, and safe client-side scenario handling.

## Improvements

### Architecture debt

- Harden the modular structure of `src/modules`/`src/shared` with strict responsibility boundaries.
- Eliminate transport/auth logic duplication across routes via shared gateways.
- Optimize SSR/ISR strategy and cache invalidation for storefront content.

### API/UI contracts

- Stabilize the storefront GraphQL query and error contract for UI components.
- Align UX states with `apps/storefront` (loading, empty, partial, failure).
- Standardize i18n and URL-based locale routing contracts.

### Observability

- Introduce web-vitals + business metrics for key storefront funnels.
- Add distributed tracing for frontend -> server requests.
- Configure alerts for frontend error growth and Core Web Vitals degradation.

### Security

- Strengthen validation and sanitization of query/input parameters on storefront pages.
- Define a secure cookie/session and third-party scripts policy.
- Add abuse-traffic protection for public filters/search (rate/throttle hints).

### Test coverage

- Expand e2e scenarios for catalog, search, cart, and checkout pre-steps.
- Add contract tests for i18n routing and API response mapping.
- Introduce visual/regression checks for key user screens.

## Stack parity (Leptos/Next.js)

- Any feature for admin/storefront is planned, decomposed, and tracked for both implementations (Leptos and Next.js) in the same delivery cycle.

### Storefront search metadata update (2026-07-02)

- [x] `src/features/search` registered as host-owned composition for the `search` storefront module.
- [x] Product-owned `packages/rustok-product::fetchCatalogSearchOptions` reads public GraphQL `storefrontCatalogSearchOptions(locale: String!)`.
- [x] Blog-owned storefront surface moved from the host feature layer to `packages/rustok-blog`; the
  package consumes the host-provided GraphQL executor through `StorefrontRenderContext`.
- [x] Removed the duplicate host-local GraphQL client; module packages and host composition use
  `src/shared/lib/graphql.ts`.
- [x] Route locale, tenant slug, and enabled modules are passed via registry render context; search package receives only category/attribute option props.

### Feature readiness checklist

- [ ] Implemented in Leptos variant.
- [ ] Implemented in Next.js variant.
- [ ] API/UI contracts match.
- [ ] Navigation and RBAC behavior are equivalent.

### Current richtext status (Blog/Forum/Comments)

- Target contract: the
  [central Richtext plan](../../../docs/modules/rich-text-implementation-plan.md).
- **Admin (Leptos, `apps/admin`)**: [~] Blog and Forum topic/reply authoring use
  the shared frame and native `#[server]` paths. Comments moderation is
  intentionally read-only and uses the shared server projection.
- **Admin (Next.js, `apps/next-admin`)**: [~] Blog and Forum topic/reply
  authoring use the same `@rustok/richtext` frame. Owner-copy i18n and mounted
  browser parity remain open.
- **Storefront (Leptos SSR, `apps/storefront`)**: [~] Blog articles and Forum
  topics/replies render owner-provided, server-sanitized HTML projections.
  Blog composes the Comments-owned reusable editor with Blog-bound native and
  parallel GraphQL commands; mounted evidence remains open.
- **Storefront (Next.js, `apps/next-frontend`)**: [~] The selected Blog detail
  surface renders the server-owned canonical HTML projection and approved
  comment previews, and composes the Comments-owned React editor with the
  Blog-bound GraphQL mutation. The host passes route locale and query state
  through `StorefrontRenderContext`; the browser never selects an arbitrary
  Comments target. Authenticated Chromium evidence now covers SSR, the shared
  iframe and toolbar, canonical document submission, and moderation-pending
  PostgreSQL persistence. Forum storefront, full comment-body rendering, and
  rejection/reload/cross-browser evidence remain open.
- Pages body remains Page Builder/Fly and is outside the richtext body
  migration. A future embedded Page component property is a separate opt-in.

The Blog/Comments Next source slice was verified locally on 2026-08-13 with
`npm run typecheck` and an authenticated mounted Chromium submission against a
fresh PostgreSQL schema and the built API server. Earlier lint and production
build evidence remains valid for the unchanged frame asset route.
The generated route table includes `/richtext/frame` and its immutable asset
route. Save/reload, rejection, and cross-browser evidence remain open.

Server-rendered module GraphQL uses `NEXT_PUBLIC_API_URL`, then
`RUSTOK_API_URL`, then the local server default `http://localhost:5150`.
Browser calls use `NEXT_PUBLIC_API_URL` only when an explicitly public API
origin is configured; otherwise they use same-origin `/api/graphql` and the
Next host proxies `/api/*` to `RUSTOK_API_URL`. Module packages do not invent
another endpoint resolver.
