# Next Frontend Documentation

Local documentation for `apps/next-frontend`.

## Purpose

`apps/next-frontend` is the Next.js storefront host for RusToK. It provides the React/Next storefront path, runs in parallel with `apps/storefront`, and must maintain parity with the Leptos storefront at the transport/auth/i18n/module contracts level.

FFA classification: `apps/next-frontend` is an `FFA-compatible composition host`, not a module-owned UI package. Its FFA responsibility is to maintain Next storefront route/context/transport parity without transferring module-specific storefront workflows into the host.

## Responsibility boundaries

- own the Next.js storefront host and its route composition;
- use shared frontend contracts for GraphQL, auth, forms and state;
- assemble the storefront through `src/app`, `src/modules`, `src/shared` and `src/components`;
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
- Search storefront composition lives in `src/features/search`: host passes route locale, tenant slug and enabled modules, product-owned `packages/rustok-product` reads the public `storefrontCatalogSearchOptions(locale: String!)`, and `packages/search` receives only host-provided category/attribute option props.

## Frontend contract

- GraphQL contract goes through shared storefront transport layer;
- auth/session contract goes through shared auth package boundary;
- forms/state contract reuses shared frontend packages;
- i18n route/layout contract must match platform storefront expectations.
- if a module-owned storefront surface uses query-driven state, Next host must maintain
  the same key semantics and canonical behavior as the Leptos storefront.
- SEO runtime is not duplicated in the host: canonical source of truth lives in `rustok-seo`, and the Next host acts only as an adapter layer on top of `SeoPageContext = route + document`.
- runtime transport policy for SEO in Next host: `REST-first + GraphQL fallback` with typed semantic error mapping (`BAD_USER_INPUT`, `PERMISSION_DENIED`, `NOT_FOUND`, transport failures), without blanket `catch {}`.
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

- Быстрый fixture baseline для D7 живёт в `contracts/seo/runtime-parity-fixtures.json` и покрывает четыре fallback сценария SEO runtime: `module_disabled`, `not_found`, `permission_denied`, `transport_failure`.
- В этом же fixture закреплена route ownership matrix для owner modules `rustok-pages`, `rustok-product`, `rustok-blog`, `rustok-forum`: каждая строка связывает Next route pattern, Rust storefront route и canonical `targetKind`.
- Минимальный non-home smoke baseline сейчас фиксирует два owner route: `/modules/product?slug=demo-product` и `/modules/blog?slug=release-notes`; эти маршруты проверяют metadata adapter assertions для canonical, robots, social metadata и JSON-LD blocks.
- Allowlist допустимых long-tail differences ограничен host-level деталями: `metadataBase`, request-local CSP nonce и whitespace-only JSON-LD serialization differences; semantic payload equality остаётся обязательной.
- Лёгкая проверка без компиляции запускается командой `npm run verify:seo-runtime-fixtures` из `apps/next-frontend`; она дополнительно проверяет существование docs rows, targeted unit coverage inventory, integration matrix plan, live artifact manifest template и ключевые static symbols для Next SEO adapter, Rust renderer, Next Admin transport и Leptos storefront SEO runtime wiring.
- Live closeout artifact set теперь явно должен включать backend GraphQL/REST parity sample, before/after outbox/index counters, Next runtime robots/sitemap/metadata sample, Leptos `SeoPageContext` smoke, media descriptor fallback smoke и owner sign-off notes; для каждого файла fixture хранит отдельный must-capture checklist и blockers, а пока live files не приложены, D8/D9 остаются pending.
- D8/D9 closeout guardrails дополнительно source-lock-ят runbook-to-artifact crosswalk, CI attachment metadata/redaction checklist, defect triage severity matrix и owner sign-off state machine, чтобы live evidence packet нельзя было продвинуть из static seed сразу в signed без runtime artifacts и owner review; template-файлы для required artifacts лежат в `contracts/seo/live-evidence/templates/` и проверяются тем же compile-free verifier-ом.
