# Storefront App (Leptos SSR) — Implementation Plan

## Host composition update (2026-07-02)

- [x] Generated search renderer uses `SearchStorefrontComposition`, connecting public product catalog option DTO/helper and search-owned props.
- [x] Host checks tenant enablement of the `product` module and passes only effective locale from `UiRouteContext`; no local locale fallback.
- [x] Product storefront metadata uses native `#[server]` first and parallel public GraphQL `storefrontCatalogSearchOptions(locale: String!)`; fast boundary guardrails fix wiring without long Rust compilation.

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
- **Admin (Leptos, `apps/admin`)**: [ ] Target shared framed editor and owner
  native `#[server]` paths are not implemented.
- **Admin (Next.js, `apps/next-admin`)**: [~] A Blog-local legacy Tiptap
  prototype exists, but it is not the target shared runtime and incorrectly
  contains Forum UI.
- **Storefront (Leptos SSR, `apps/storefront`)**: [ ] Blog/Forum/Comments still
  need the canonical server-rendered HTML projection instead of raw payload
  summaries.
- **Storefront (Next.js, `apps/next-frontend`)**: [ ] Matching SSR projection,
  locale, and route coverage are not implemented.
- Pages body remains Page Builder/Fly and is outside the richtext body
  migration. A future embedded Page component property is a separate opt-in.
