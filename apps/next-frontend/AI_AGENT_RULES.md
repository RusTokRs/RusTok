# AI Agent Rules for `apps/next-frontend`

## READ THESE GUIDES FIRST

Before making ANY changes to Next.js storefront code:

1. **[Next Frontend docs/README.md](./docs/README.md)** — host boundaries, FSD orientation, contract parity
2. **[Storefront Contract](../../docs/UI/storefront.md)** — transport/auth/i18n/module parity requirements
3. **[FFA Architecture Guide](../../docs/UI/module-package-architecture.md)** — background on FFA for Rust packages

## Critical Rules

### 1. FSD Architecture
✅ **ALWAYS follow Feature-Sliced Design** orientation:
- `src/app` — Next.js App Router, routes, layouts
- `src/modules` — module composition
- `src/shared` — shared contracts (lib, api)
- `src/components` — shared UI components

❌ **NEVER place module storefront UI** inside host-owned routes

### 2. DO NOT Duplicate Transport/Auth
✅ **ALWAYS use:** shared contracts in `src/shared/lib` and `packages/*`
❌ **NEVER create** ad-hoc GraphQL/REST clients or auth wrappers per page

Reuse shared integration gateways in `src/shared/lib/*`.

### 3. DO NOT Invent Custom i18n
✅ **ALWAYS use:** middleware locale + `next-intl`
❌ **NEVER create** package-local cookie/header/query locale fallback

Locale normalization:
- Middleware handles: `?locale` → cookie → `Accept-Language` → default
- Supported locales from message loaders, NOT hardcoded `/en|/ru`

### 4. DO NOT Write Custom SEO Runtime
✅ **ALWAYS use:** `SeoPageContext` from backend + Next Metadata API adapter
❌ **NEVER create** host-local SEO source-of-truth

SEO contract:
- Runtime-driven `robots.ts` and `sitemap.ts` through backend
- Typed semantic error mapping: `BAD_USER_INPUT`, `PERMISSION_DENIED`, `NOT_FOUND`, transport failures
- `SeoStructuredDataBlock` preserves backend `schemaKind`, `schemaType`, `source`, payload
- No host-local schema.org classifier

Rollout guard: `NEXT_PUBLIC_SEO_NEXT_RUNTIME_SITEMAP_ENABLED` or `SEO_NEXT_RUNTIME_SITEMAP_ENABLED`

### 5. DO NOT Hardcode Locale Routes
❌ **NEVER hardcode** `/en|/ru` in middleware
✅ **ALWAYS get** supported locales from message loaders

### 6. DO NOT Duplicate Module Composition Logic
✅ Module composition lives in `src/features/<module>`
- Host passes: route locale, tenant slug, enabled modules
- Module packages (e.g., `packages/rustok-product`) read public transport
- Host composes with safe option props

**Example: Search storefront**
- `src/features/search` — host composition
- `packages/rustok-product` — reads `storefrontCatalogSearchOptions(locale: String!)`
- `packages/search` — receives category/attribute option props

### 7. Query Semantics Parity
✅ **MUST match** `apps/storefront` contract
❌ **DO NOT invent** separate schema/policy on top of backend and Leptos host

Use same typed `snake_case` query keys as Rust storefront.

## Verification Commands

After ANY change:
```powershell
npm run typecheck
npm run lint
npm run verify:i18n:ui
npm run verify:seo-runtime-fixtures  # If touching SEO
```

## SEO Runtime Parity Evidence

Lightweight verification (no compilation):
```powershell
npm run verify:seo-runtime-fixtures
```

Checks:
- Contract shape
- Fallback semantics: `module_disabled`, `not_found`, `permission_denied`, `transport_failure`
- Route ownership matrix: Next route → Rust storefront route → `targetKind`
- Minimal smoke baseline: `/modules/product?slug=demo-product`, `/modules/blog?slug=release-notes`
- Docs rows, unit coverage, integration matrix, live artifact templates

Live closeout artifacts (required before final D8/D9 sign-off):
- Backend GraphQL/REST parity sample
- Before/after outbox/index counters
- Next runtime `robots.ts`/`sitemap.ts`/metadata sample
- Leptos `SeoPageContext` smoke
- Media descriptor fallback smoke
- Owner sign-off notes

Templates: `contracts/seo/live-evidence/templates/`

## Common Mistakes to Avoid

| ❌ WRONG | ✅ RIGHT |
|---------|---------|
| Ad-hoc GraphQL client per route | Use `src/shared/lib` |
| Hardcoded `/en\|/ru` in middleware | Get locales from message loaders |
| Host-local SEO source-of-truth | Use `SeoPageContext` from backend |
| Package-local i18n fallback chain | Use middleware + `next-intl` |
| Module UI inside host routes | Compose from `packages/*` and `src/modules` |
| Separate query schema from Rust | Match `apps/storefront` contract |
| Host-local schema.org mapping | Use backend `SeoStructuredDataBlock` |

## Related to Rust Packages

If you're working on **Rust module UI packages** (`crates/rustok-*/storefront`), those follow different rules:

- [Architecture Guide](../../docs/UI/module-package-architecture.md) — FFA, `core/transport/ui` split
- [Implementation Guide](../../docs/UI/module-package-implementation.md) — internal libraries, i18n, file structure
- [Verification Guide](../../docs/UI/module-package-verification.md) — verification commands

## Full Documentation

- [apps/next-frontend/docs/README.md](./docs/README.md) — host-level documentation
- [docs/UI/storefront.md](../../docs/UI/storefront.md) — storefront contract
- [docs/UI/README.md](../../docs/UI/README.md) — UI documentation index
- [docs/index.md](../../docs/index.md) — platform documentation map
