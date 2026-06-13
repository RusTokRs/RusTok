# Документация Next Frontend

Локальная документация для `apps/next-frontend`.

## Назначение

`apps/next-frontend` является Next.js storefront host для RusToK. Он даёт React/Next storefront path, работает параллельно с `apps/storefront` и должен сохранять parity с Leptos storefront на уровне transport/auth/i18n/module contracts.

## Границы ответственности

- владеть Next.js storefront host и его route composition;
- использовать shared frontend contracts для GraphQL, auth, forms и state;
- собирать storefront через `src/app`, `src/modules`, `src/shared` и `src/components`;
- не дублировать transport/auth code по страницам;
- не подменять собой module-owned storefront UI contracts.

## Runtime contract

- host следует FSD-ориентиру `src/app`, `src/modules`, `src/shared`, `src/components`;
- shared integration gateways живут в `src/shared/lib/*`;
- backend API идёт через `apps/server`;
- auth и transport contracts переиспользуются через shared packages, а не через ad-hoc clients;
- storefront host должен оставаться синхронизированным с `apps/storefront` по route/i18n/auth contracts.
- locale-aware middleware должен матчить весь storefront surface без hardcoded `/en|/ru`
  filter; поддерживаемые locales берутся из host-owned message loaders.
- query semantics для module-owned storefront surfaces должны оставаться в parity с `apps/storefront`;
  host не inventит отдельную schema/policy поверх backend и Leptos host contract.

## Frontend contract

- GraphQL contract идёт через shared storefront transport layer;
- auth/session contract идёт через shared auth package boundary;
- forms/state contract переиспользует shared frontend packages;
- i18n route/layout contract должен совпадать с platform storefront expectations.
- если module-owned storefront surface использует query-driven state, Next host обязан держать
  те же key semantics и canonical behavior, что и Leptos storefront.
- SEO runtime не дублируется в host: canonical source of truth живёт в `rustok-seo`, а Next host выступает только adapter-слоем поверх `SeoPageContext = route + document`.
- runtime transport policy для SEO в Next host: `REST-first + GraphQL fallback` с typed semantic error mapping (`BAD_USER_INPUT`, `PERMISSION_DENIED`, `NOT_FOUND`, transport failures), без blanket `catch {}`.
- built-in Next Metadata API считается основным render target для SEO head; shared metadata builder маппит туда typed robots, Open Graph, Twitter, verification и alternates без собственного SEO source-of-truth в host.
- `robots.ts` и `sitemap.ts` работают в runtime-driven режиме через SEO runtime source; host-local static правила допустимы только как аварийный fallback или rollout guard.
- Rollout guard для runtime robots/sitemap задаётся флагом `NEXT_PUBLIC_SEO_NEXT_RUNTIME_SITEMAP_ENABLED` (или `SEO_NEXT_RUNTIME_SITEMAP_ENABLED` в server env).
- `SeoStructuredDataBlock` в shared TypeScript contract сохраняет backend-provided `schemaKind`, `schemaType`, legacy `kind`, `source` и payload; Next host не классифицирует schema.org types локально и рендерит JSON-LD blocks как runtime-provided scripts.
- Rust-host путь при этом вынесен в отдельный support crate `rustok-seo-render`; Next host остаётся TypeScript adapter-слоем и не пытается делить с ним source-of-truth.

## Взаимодействия

- `apps/server` — backend/API provider;
- `apps/storefront` — параллельный Leptos storefront host для contract parity;
- `crates/rustok-*` и module-owned surfaces подключаются через backend и frontend integration layer, а не через host-local business logic.

## Проверка

- lint/typecheck прогоны по `apps/next-frontend`
- storefront route/i18n contract checks
- сверка shared contract с `docs/UI/storefront.md` и `docs/modules/manifest.md`

## Связанные документы

- [App README](../README.md)
- [Storefront docs](../../../docs/UI/storefront.md)
- [Контракт manifest-слоя](../../../docs/modules/manifest.md)
- [Карта документации](../../../docs/index.md)

## SEO runtime parity evidence

- Быстрый fixture baseline для D7 живёт в `contracts/seo/runtime-parity-fixtures.json` и покрывает четыре fallback сценария SEO runtime: `module_disabled`, `not_found`, `permission_denied`, `transport_failure`.
- В этом же fixture закреплена route ownership matrix для owner modules `rustok-pages`, `rustok-product`, `rustok-blog`, `rustok-forum`: каждая строка связывает Next route pattern, Rust storefront route и canonical `targetKind`.
- Минимальный non-home smoke baseline сейчас фиксирует два owner route: `/modules/product?slug=demo-product` и `/modules/blog?slug=release-notes`; эти маршруты проверяют metadata adapter assertions для canonical, robots, social metadata и JSON-LD blocks.
- Allowlist допустимых long-tail differences ограничен host-level деталями: `metadataBase`, request-local CSP nonce и whitespace-only JSON-LD serialization differences; semantic payload equality остаётся обязательной.
- Лёгкая проверка без компиляции запускается командой `npm run verify:seo-runtime-fixtures` из `apps/next-frontend`.
