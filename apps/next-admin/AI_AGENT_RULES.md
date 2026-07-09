# AI Agent Rules for `apps/next-admin`

## READ THESE GUIDES FIRST

Before making ANY changes to Next.js admin code:

1. **[Next Admin docs/README.md](./docs/README.md)** — host boundaries, FSD architecture, module ownership
2. **[FFA Architecture Guide](../../docs/UI/module-package-architecture.md)** — background on FFA for Rust packages
3. **[Storefront Contract](../../docs/UI/storefront.md)** — transport/auth/i18n parity

## Critical Rules

### 1. FSD Architecture

✅ **ALWAYS follow Feature-Sliced Design** layers:

- `app` — Next.js App Router, routes, layouts
- `shared` — shared contracts (api, lib, ui components)
- `entities` — domain entities
- `features` — business features
- `widgets` — composite UI blocks

❌ **NEVER place module business UI** in `apps/next-admin/src/` — use `packages/*` or `@rustok/*-admin`

### 2. DO NOT Duplicate Module UI in Host

✅ Module admin UI belongs in `apps/next-admin/packages/*` or `@rustok/*-admin` packages
❌ **NEVER place** blog/product/commerce/search/AI admin workflows in host routes

**Examples of WRONG placement:**

- Creating `/app/blog/posts/page.tsx` with blog CRUD
- Creating `/app/products/catalog/page.tsx` with product management
- Creating `/app/ai/prompts/page.tsx` with AI prompt editor

**Examples of RIGHT placement:**

- Module UI in `packages/rustok-blog-admin`
- Host composition in `src/features/search` that calls module packages
- Shell/navigation in `src/widgets/app_shell`

### 3. DO NOT Write Custom Components Without Checking

✅ **ALWAYS check first:** `src/shared/ui` and existing `packages/*`

If component doesn't exist, check if it's available in:

- `@radix-ui/*` (primitives)
- `shadcn/ui` patterns
- Internal packages

### 4. DO NOT Invent Custom i18n

✅ **ALWAYS use:** host-provided `x-rustok-effective-locale` + `next-intl`
❌ **NEVER create** package-local cookie/header/query locale fallback chains

User locale selection:

- Host cookie: `rustok-admin-locale`
- Middleware normalizes: `?locale` → cookie → `x-rustok-effective-locale` → `Accept-Language` → `en`
- UI uses dropdown in header and auth screens

### 5. DO NOT Duplicate Transport/Auth

✅ **ALWAYS use:** shared contracts in `src/shared/api` and `src/shared/lib`
❌ **NEVER create** ad-hoc GraphQL clients or auth wrappers per page

### 6. DO NOT Create Starter-Only Routes

❌ **MUST return `notFound()`:** `billing`, `exclusive`, `workspaces`, `workspaces/team`

These are not part of the RusTok admin contract. Do not expose placeholder UI.

### 7. DO NOT Hardcode Module Navigation

✅ Module nav items are **registry-driven** and **filtered by enabled module slug**
❌ **NEVER hardcode** module links in shell navigation

### 8. Route Selection Contract Parity

✅ **MUST match** `apps/admin` typed `snake_case` query keys:

- Use: `product_id`, `cart_id`, `order_id`, `tab`, `slug`
- Never: legacy `id`, camelCase aliases
- No auto-select-first as source of truth

Local Next helpers must NOT invent separate schema on top of `rustok-api` contract.

## Verification Commands

After ANY change:

```powershell
npm run typecheck
npm run lint
npm run verify:i18n:ui
npm run verify:i18n:contract
```

## SEO Operator Contract

Shared API helper: `src/shared/api/seo.ts`

Provides typed access to:

- `seoTargets` — registry-backed target descriptors
- Diagnostics
- Sitemap status/jobs
- Bulk jobs and job detail

Strategy: **REST primary (rollout-gated) + GraphQL secondary path**

Semantic error taxonomy:

- `BAD_USER_INPUT`
- `PERMISSION_DENIED`
- `NOT_FOUND`
- Transport failures

This is canonical for Next hosts and reused in Next storefront.

## Common Mistakes to Avoid

| ❌ WRONG                           | ✅ RIGHT                                     |
| ---------------------------------- | -------------------------------------------- |
| Module UI in `src/features/blog`   | Package `@rustok/blog-admin`                 |
| Hardcoded `/billing` route         | Return `notFound()`                          |
| Custom GraphQL client per page     | Use `src/shared/api`                         |
| Package-local `use_cookie("lang")` | Use host `x-rustok-effective-locale`         |
| Hardcoded module nav links         | Registry-driven, filtered by enabled modules |
| CamelCase query keys               | Typed `snake_case` per contract              |
| Host-local SEO target mapping      | Use GraphQL `seoTargets`                     |

## Related to Rust Packages

If you're working on **Rust module UI packages** (`crates/rustok-*/admin`), those follow different rules:

- [Architecture Guide](../../docs/UI/module-package-architecture.md) — FFA, `core/transport/ui` split
- [Implementation Guide](../../docs/UI/module-package-implementation.md) — internal libraries, i18n, file structure
- [Verification Guide](../../docs/UI/module-package-verification.md) — verification commands

## Full Documentation

- [apps/next-admin/docs/README.md](./docs/README.md) — host-level documentation
- [docs/UI/README.md](../../docs/UI/README.md) — UI documentation index
- [docs/index.md](../../docs/index.md) — platform documentation map
