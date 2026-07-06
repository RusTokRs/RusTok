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

### Current rich-text status (blog/forum/pages)

- **Admin (Leptos, `apps/admin`)**: [ ] Not started / in sync process with Next.js implementation.
- **Admin (Next.js, `apps/next-admin`)**: [~] Partially implemented (Tiptap/Page Builder routes connected, needs real entity ID work and parity-check with Leptos).
- **Storefront (Leptos SSR, `apps/storefront`)**: [ ] Not started (rich-text rendering parity for blog/forum/pages planned).
- **Storefront (Next.js, `apps/next-frontend`)**: [ ] Not started (rich-text rendering parity for blog/forum/pages planned).
