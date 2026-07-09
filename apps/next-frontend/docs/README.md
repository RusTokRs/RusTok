# Next Frontend Documentation

> **MANDATORY FOR AI AGENTS — Read these guides BEFORE any code changes:**
>
> **FSD Architecture:** This app follows **Feature-Sliced Design** (FSD) orientation: `src/app`, `src/modules`, `src/shared`, `src/components`.
>
> **Module Ownership:** Module storefront UI must NOT be placed inside host-owned routes. Use proper module composition through `src/modules` and `src/shared`.
>
> **IMPORTANT RULES:**
> - **DO NOT duplicate transport/auth** — use shared contracts in `src/shared/lib` and `packages/*`
> - **DO NOT invent custom i18n** — use middleware locale and `next-intl`
> - **DO NOT write custom SEO runtime** — use `SeoPageContext` from backend + Next Metadata API adapter
> - **DO NOT hardcode `/en|/ru` in middleware** — get supported locales from message loaders
>
> **Related Guides for Rust Module UI Packages:**
> - [Module UI Package Guide](../../../docs/UI/module-package-architecture.md) — applies to Rust/Leptos module packages, not this Next.js host
> - [Storefront Contract](../../../docs/UI/storefront.md) — transport/auth/i18n parity requirements

Local documentation for `apps/next-frontend`.

## Purpose

`apps/next-frontend` is the Next.js storefront host for RusToK. It provides the React/Next storefront path, runs in parallel with `apps/storefront`, and must maintain parity with the Leptos storefront at the transport/auth/i18n/module contracts level.

Architecture classification: `apps/next-frontend` is a Next.js composition host, not an FFA host and not a module-owned UI package. It maintains route/context/transport parity through normal Next.js package ownership and shared contracts.

## Responsibility boundaries

- own the Next.js storefront host and its route composition;
- use shared frontend contracts for GraphQL, auth, forms and state;
- assemble the storefront through `src/app`, `src/modules`, `src/shared` and `src/components`;
- mount module-owned Next storefront surfaces from `packages/*` (currently `rustok-blog`,
  `rustok-product` and `search`);
- not duplicate transport/auth code across pages;
- not replace module-owned storefront UI contracts.

## Runtime contract

- host follows the FSD orientation of `src/app`, `src/modules`, `src/shared`, `src/components`;
- shared integration gateways live in `src/shared/lib/*`;
- backend API goes through `apps/server`;
- auth and transport contracts are reused through shared packages, not through ad-hoc clients;
- storefront host must stay synchronized with `apps/storefront` on route/i18n/auth contracts.
- locale-aware middleware should match the entire storefront surface without hardcoded `/en|/ru`
  filter; supported locales are taken from host-owned message loaders.
- query semantics for module-owned storefront surfaces must remain in parity with `apps/storefront`;
  host does not invent a separate schema/policy on top of backend and Leptos host contract.
- Search storefront composition lives in `src/features/search`: host passes route locale, tenant slug,
  enabled modules and the shared GraphQL executor, product-owned `packages/rustok-product` reads the
  public `storefrontCatalogSearchOptions(locale: String!)`, and `packages/search` receives only
  host-provided category/attribute option props.
- Blog storefront composition lives in `packages/rustok-blog`; it receives the host GraphQL executor
  and tenant context through the registry and must not use a package-local client or env-based tenant
  resolution/header construction.

## Frontend contract

- GraphQL contract goes through shared storefront transport layer;
- auth/session contract goes through shared auth package boundary;
- forms/state contract reuses shared frontend packages;
- i18n route/layout contract must match platform storefront expectations.
- if a module-owned storefront surface uses query-driven state, Next host must maintain
  the same key semantics and canonical behavior as the Leptos storefront.
- SEO runtime is not duplicated in the host: canonical source of truth lives in `rustok-seo`, and the Next host acts only as an adapter layer on top of `SeoPageContext = route + document`.
- runtime transport policy for SEO in Next host: `REST primary + GraphQL secondary path` with typed semantic error mapping (`BAD_USER_INPUT`, `PERMISSION_DENIED`, `NOT_FOUND`, transport failures), without blanket `catch {}`.
- built-in Next Metadata API is considered the primary render target for SEO head; the shared metadata builder maps typed robots, Open Graph, Twitter, verification and alternates there without its own SEO source-of-truth in the host.
- `robots.ts` and `sitemap.ts` operate in runtime-driven mode through the SEO runtime source; host-local static rules are allowed only as an emergency fallback or rollout guard.
- Rollout guard for runtime robots/sitemap is set by the `NEXT_PUBLIC_SEO_NEXT_RUNTIME_SITEMAP_ENABLED` flag (or `SEO_NEXT_RUNTIME_SITEMAP_ENABLED` in server env).
- `SeoStructuredDataBlock` in the shared TypeScript contract preserves backend-provided `schemaKind`, `schemaType`, legacy `kind`, `source` and payload; Next host does not classify schema.org types locally and renders JSON-LD blocks as runtime-provided scripts.
- The Rust-host path is extracted into a separate support crate `rustok-seo-render`; the Next host remains a TypeScript adapter layer and does not attempt to share source-of-truth with it.

## D8/D9 SEO closeout contract

- This host owns the compile-free SEO evidence fixture and verifier used for D8 lightweight gates.
- The verifier intentionally does not compile the app; it checks contract shape, fallback semantics, route ownership, smoke assertions, docs sync rows, owner sign-off rows, targeted unit coverage inventory, integration matrix plan, live artifact manifest template, concrete per-file artifact templates and the deferred live evidence plan.
- Before final closeout, this host must attach live evidence for runtime `robots.ts`, `sitemap.ts`, home metadata and at least product/blog non-home metadata routes against a running backend.

## Interactions

- `apps/server` — backend/API provider;
- `apps/storefront` — parallel Leptos storefront host for contract parity;
- `crates/rustok-*` and module-owned surfaces connect through the backend and frontend integration layer, not through host-local business logic.

## Verification

- lint/typecheck runs across `apps/next-frontend`
- storefront route/i18n contract checks
- shared contract reconciliation with `docs/UI/storefront.md` and `docs/modules/manifest.md`

## Related documents

- [App README](../README.md)
- [Storefront docs](../../../docs/UI/storefront.md)
- [Manifest layer contract](../../../docs/modules/manifest.md)
- [Documentation map](../../../docs/index.md)

## SEO runtime parity evidence

- The quick fixture baseline for D7 lives in `contracts/seo/runtime-parity-fixtures.json` and covers four SEO runtime fallback scenarios: `module_disabled`, `not_found`, `permission_denied`, `transport_failure`.
- The same fixture establishes the route ownership matrix for owner modules `rustok-pages`, `rustok-product`, `rustok-blog`, `rustok-forum`: each row links Next route pattern, Rust storefront route and canonical `targetKind`.
- The minimal non-home smoke baseline currently captures two owner routes: `/modules/product?slug=demo-product` and `/modules/blog?slug=release-notes`; these routes verify metadata adapter assertions for canonical, robots, social metadata and JSON-LD blocks.
- The allowlist of acceptable long-tail differences is limited to host-level details: `metadataBase`, request-local CSP nonce and whitespace-only JSON-LD serialization differences; semantic payload equality remains mandatory.
- The lightweight verification without compilation runs via `npm run verify:seo-runtime-fixtures` from `apps/next-frontend`; it additionally checks the existence of docs rows, targeted unit coverage inventory, integration matrix plan, live artifact manifest template and key static symbols for Next SEO adapter, Rust renderer, Next Admin transport and Leptos storefront SEO runtime wiring.
- Live closeout artifact set must now explicitly include backend GraphQL/REST parity sample, before/after outbox/index counters, Next runtime robots/sitemap/metadata sample, Leptos `SeoPageContext` smoke, media descriptor fallback smoke and owner sign-off notes; for each file the fixture stores a separate must-capture checklist and blockers, and until live files are attached, D8/D9 remain pending.
- D8/D9 closeout guardrails additionally source-lock the runbook-to-artifact crosswalk, CI attachment metadata/redaction checklist, defect triage severity matrix and owner sign-off state machine, so that the live evidence packet cannot be promoted from static seed directly to signed without runtime artifacts and owner review; template files for required artifacts are located in `contracts/seo/live-evidence/templates/` and are verified by the same compile-free verifier.
