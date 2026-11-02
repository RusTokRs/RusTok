# apps/next-frontend / CRATE_API

## 1. Public Interfaces & Architecture

`@rustok/next-frontend` (`apps/next-frontend`) is the Next.js storefront customer-facing application host and composition root for RusToK. It operates in parallel with `apps/storefront` (Leptos) to deliver a modern, multi-locale, tenant-isolated e-commerce and content portal.

### Architectural Orientation (FSD)
- **`src/app`**: Next.js App Router root layouts, locale segments (`/[locale]/...`), SEO endpoints (`robots.ts`, `sitemap.ts`), and route handlers (`/api/draft`, `/api/revalidate`, `/richtext/frame`).
- **`src/modules`**: Storefront slot registry and module composition root (`registerStorefrontModule`, `getModulesForSlot`).
- **`src/features`**: Cross-cutting composite feature integrations (e.g. `src/features/search` wiring product catalog search options to search UI).
- **`src/shared`**: Reusable integration primitives:
  - `shared/lib`: Transport gateway (`storefrontGraphql`), client auth (`getClientAuth`), and enabled module contexts.
  - `shared/api`: Backend query callers (`fetchEnabledModules`, `getStorefrontTenantSlug`).
  - `shared/seo`: Canonical metadata and JSON-LD structured data generators backed by backend `SeoPageContext`.
  - `shared/ui`: Reusable presentation components (`TableOfContents`, UI primitives).

### Storefront Slot System
```typescript
export type StorefrontSlot = "home:afterHero";

export type StorefrontRenderContext = {
  locale: string;
  searchParams: Record<string, string | string[] | undefined>;
  enabledModules: string[];
  tenantSlug: string | null;
  tenantId: string | null;
  graphql: typeof storefrontGraphql;
};

export type StorefrontModule = {
  id: string;
  moduleSlug?: string;
  slot: StorefrontSlot;
  order?: number;
  render: (context: StorefrontRenderContext) => ReactNode;
};

export function registerStorefrontModule(module: StorefrontModule): void;
export function getModulesForSlot(slot: StorefrontSlot, enabledModules?: Iterable<string>): StorefrontModule[];
```

---

## 2. Key Contracts, Routes & Layouts

### Route Parity
- **Home:** `/[locale]` — Hero section, dynamic slot modules (`home:afterHero`), and JSON-LD structured data.
- **Product Catalog:** `/[locale]/products`, `/[locale]/products/[slug]` — Product listings, facets, variants, and exact decimal pricing.
- **Cart & Checkout:** `/[locale]/checkout` with global `CartDrawer` and `CartTrigger` embedded in `LocaleLayout`.
- **Blog:** `/[locale]/blog`, `/[locale]/blog/[slug]`, `/[locale]/blog/feed.xml` — Editorial articles, threaded comments, and RSS syndication.
- **Forum:** `/[locale]/forum/t/[...slug]`, `/[locale]/modules/forum` — Topic feeds, discussions, quote tooltips, and draft preservation.
- **Static Pages:** `/[locale]/pages/[slug]` — Page Builder landing pages rendered via Server Components with scoped CSS.
- **Customer Account:** `/[locale]/account`, `/[locale]/account/orders`, `/[locale]/orders/[id]` — Order history, details, and tracking.
- **Search:** `/[locale]/search` — Instant search autocomplete and filter matrix.

### Layout Hierarchy
1. **`RootLayout` (`src/app/layout.tsx`)**: Global HTML wrapper, theme styles, `FluentProvider` with Project Fluent catalogs, and `EnabledModulesProvider`.
2. **`LocaleLayout` (`src/app/[locale]/layout.tsx`)**: Enforces active locale via `setRequestLocale(locale)` and wraps children in tenant-scoped `CartProvider`, `CartTrigger`, and `CartDrawer`.

---

## 3. Integration, Transport & Security Boundary

### GraphQL Transport (`storefrontGraphql`)
- **Apollo Client 4 Integration**: Uses `HttpLink` with `fetchPolicy: "no-cache"` and per-request `fetchOptions: { cache: "no-store" }` when an auth token is present.
- **Error Preservation**: Evaluates `CombinedGraphQLErrors.is(result.error)` under `errorPolicy: "all"`, ensuring permission denials and validation errors are never masked as empty results.
- **Token Isolation**: Anonymous requests never leak or borrow authorization headers from previous viewer requests.
- **Tenant Context**: Propagates `X-Tenant-Slug` and `X-Tenant-ID` headers to `apps/server`.

### SEO & Structured Data
- **Backend-Driven Contract**: Maps backend `SeoPageContext` directly to Next.js `Metadata` via `buildSeoMetadata` and JSON-LD scripts via `buildSeoStructuredDataScripts`.
- **Dynamic Endpoints**: `sitemap.ts` is explicitly marked `dynamic = "force-dynamic"` to ensure newly private or unlisted content is omitted without waiting for redeployment.

### Preview & Revalidation Security
- `/api/draft` and `/api/revalidate` verify caller tokens using timing-safe buffer comparison (`timingSafeEqual`) to prevent timing attacks.

---

## 4. Verification & Testing Evidence

- **Typecheck**: `npm run typecheck` (`next-fluent typegen`, `next typegen`, `tsc --noEmit`) passes with 0 errors.
- **Linter**: `npm run lint` (`eslint .`) passes with 0 warnings/errors.
- **Transport Tests**: `src/shared/lib/graphql.test.mjs` verifies Apollo 4 error handling and token isolation.
- **Host FFA Contract**: `npm run verify:frontend:host-ffa-contract` confirms strict FSD layer boundaries.
- **Route Parity Gate**: `npm run verify:storefront:routes` verifies locale-aware route helpers across storefront packages.
- **SEO Runtime Fixtures**: `npm run verify:seo-runtime-fixtures` verifies contract shape, fallback semantics, route ownership, and JSON-LD schemas.
- **End-to-End Test Suite**: Supported by Playwright (`tests/e2e`).

---

## 5. Anti-Patterns & Operational Invariants

1. **Zero Module UI in Host Routes**: Module UI must be imported from `@rustok/*-frontend` packages or composed through `src/modules` / `src/features`. Host routes must never contain hardcoded module internal views.
2. **No Hardcoded Locale Filter**: Locales are resolved dynamically through `src/i18n-config.ts` and Project Fluent loaders; never hardcode `/en|/ru` in middleware or route matchers.
3. **No Host-Local SEO Source-of-Truth**: Canonical SEO metadata originates from `apps/server` via `SeoPageContext`. The host acts strictly as an adapter layer and must not invent local schema.org classifiers.
4. **No Swallowed GraphQL Failures**: Field denials and network errors must be surfaced to the user with retry affordances rather than silently rendering empty lists.
5. **Exact Decimal Monetary Math**: Price and discount values must always be processed and rendered as exact major-unit decimal strings without IEEE 754 floating-point conversion.
