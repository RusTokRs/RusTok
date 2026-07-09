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

### Current rich-text status (blog/forum/pages)

- **Admin (Leptos, `apps/admin`)**: [ ] Not started / in sync process with Next.js implementation.
- **Admin (Next.js, `apps/next-admin`)**: [~] Partially implemented (Tiptap/Page Builder routes connected, needs real entity ID work and parity-check with Leptos).
- **Storefront (Leptos SSR, `apps/storefront`)**: [ ] Not started (rich-text rendering parity for blog/forum/pages planned).
- **Storefront (Next.js, `apps/next-frontend`)**: [ ] Not started (rich-text rendering parity for blog/forum/pages planned).
