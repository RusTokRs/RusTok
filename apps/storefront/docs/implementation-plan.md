# Storefront App (Leptos SSR) — Implementation Plan

## Host composition update (2026-07-02)

- [x] Generated search renderer uses `SearchStorefrontComposition`, connecting public product catalog option DTO/helper and search-owned props.
- [x] Host checks tenant enablement of the `product` module and passes only effective locale from `UiRouteContext`; no local locale fallback.
- [x] Product storefront metadata uses native `#[server]` first and parallel public GraphQL `storefrontCatalogSearchOptions(locale: String!)`; fast boundary guardrails fix wiring without long Rust compilation.

## Tenant module-state trust update (2026-07-30)

- [x] Native `storefront/list-enabled-modules` no longer accepts a client-provided tenant slug.
- [x] The SSR adapter extracts the middleware-resolved `rustok_api::TenantContext` and reads module state only for `tenant.id`.
- [x] The configured tenant slug remains limited to the GraphQL transport request where it is a host-routing hint, not native write/read authority.
- [x] `verify-tenant-fba.mjs` rejects `tenant_slug`, `get_tenant_by_slug`, or a slug argument in the native adapter and requires the trusted context extraction.
- [ ] Same-SHA storefront compilation and native/GraphQL parity execution remain required before the storefront wave can be completed.

## Focus

Develop `apps/storefront` as a stable SSR storefront with predictable performance, safe user input handling, and unified contracts with the backend.

## Improvements

### Architecture debt

- Formalize boundaries between SSR orchestration, shared integrations, and feature modules.
- Reduce UI/business scenario duplication with `apps/next-frontend` through shared contracts.
- Optimize data fetching and caching strategy for SSR pages.

### API/UI contracts

- Stabilize storefront API contracts (catalog, content blocks, filters, pagination).
- Standardize UI states for errors/empty data/partial responses.
- Synchronize i18n and locale routing with backend expectations.

### Observability

- Add web-vitals and SSR latency metrics for key pages.
- Introduce request tracing from storefront -> server via correlation id.
- Define alerts for TTFB increase / rendering errors.

### Security

- Improve sanitization of user/content HTML before SSR.
- Add abuse protection for public filters and search parameters.
- Define policy for cookie/session interaction with backend auth.

### Test coverage

- Add integration/e2e scenarios for catalog, product card, and search.
- Expand SSR hydration consistency and i18n fallback tests.
- Introduce regression tests for critical storefront routes.

## Stack parity (Leptos/Next.js)

- Any feature for admin/storefront is planned, decomposed, and tracked for both implementations (Leptos and Next.js) in the same delivery cycle.

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
  Blog composes the Comments-owned reusable editor as an isolated authenticated
  WASM island with a Blog-bound native command and retains the parallel GraphQL
  command. Selected article SSR stays inert; a CSP-nonced bootstrap loads the
  editor only for an active browser session, without hydrating the storefront.
  Content serves the canonical manifest-selected frame assets from the same
  origin. The wasm32 artifact build passes; mounted persistence/reload evidence
  remains open. Approved lists still use bounded projections.
- **Storefront (Next.js, `apps/next-frontend`)**: [~] The selected Blog detail
  renders the canonical projection and composes the same Comments-owned editor.
  Authenticated mounted submission and moderation-pending PostgreSQL persistence
  pass; rejection/reload and broader Forum parity remain open.
- Pages body remains Page Builder/Fly and is outside the richtext body
  migration. A future embedded Page component property is a separate opt-in.
