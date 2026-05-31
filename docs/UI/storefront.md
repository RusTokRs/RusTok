# Витрина: host и contract

RusToK поддерживает два web storefront host-приложения и отдельный mobile storefront host:

- `apps/storefront` — основной Leptos SSR-first host;
- `apps/next-frontend` — параллельный Next.js host;
- `rustok_mobile/apps/rustok_frontend_mobile` — Flutter customer storefront mobile host.

Все реализации должны сохранять единый backend, routing, locale и module contract. Leptos storefront остаётся основным Rust SSR/hydrate host путём, Next.js storefront — headless-параллелью, Flutter storefront mobile — mobile/headless host без Flutter-only backend API.

## Host contract

- Host рендерит shell и generic module pages.
- Module-owned storefront packages подключаются через manifest-driven wiring.
- Generic storefront routes живут в семействе `/modules/{route_segment}` с locale-aware вариантом там, где это требуется host runtime.
- Module-owned packages обязаны строить внутренние ссылки через host route context, а не через hardcoded route strings.
- Для Leptos storefront packages query/state reads тоже должны идти через общий helper layer
  `leptos-ui-routing`; storefront не заводит второй package-local route helper поверх `UiRouteContext`.

## Data-layer contract

- Для Leptos storefront путь по умолчанию в product runtime: `UI -> local API -> #[server] -> service layer`.
- Внешний GraphQL contract `/api/graphql` остаётся обязательным и поддерживаемым параллельным путём.
- Host сначала использует native `#[server]` surface там, где он уже есть, и только затем откатывается к GraphQL, если это предусмотрено runtime contract.
- Новый module-owned storefront UI не должен проектироваться как GraphQL-only, если может жить через `#[server]`.
- Standalone CSR для Leptos storefront package считается debug/compatibility профилем: такой package должен иметь GraphQL/REST fallback и не должен требовать `/api/fn/*`.
- Module-owned storefront packages не должны схлопывать typed business snapshots до summary-only UI state:
  если backend уже отдаёт typed adjustments, delivery ownership или другие language-agnostic business keys,
  package API и UI обязаны сохранять эти поля, а не отбрасывать `scope`/metadata на последней миле.

## Canonical routing и locale

- Canonical URL policy и alias storage живут в backend/domain слое, а не в storefront host.
- Storefront использует backend preflight для canonical route resolution до рендера страницы.
- Effective locale выбирается runtime/host слоем один раз и затем прокидывается в UI surface.
- Query-based locale fallback допустим только как backward-compatible path; module-owned UI не должен вводить свою fallback-цепочку.
- Route/query parity между `apps/storefront`, `apps/next-frontend` и `rustok_frontend_mobile` должна соблюдаться на уровне
  key semantics и host contract, даже если конкретные helper implementations различаются.

## Parity с Next.js и Flutter storefront

- `apps/next-frontend` обязан сохранять parity с `apps/storefront` по route, auth, i18n и backend contracts.
- `rustok_frontend_mobile` обязан использовать тот же customer-facing storefront contract и не смешиваться с admin/operator mobile UX.
- Поверхности Flutter catalog/cart живут в `rustok_mobile/packages/rustok_catalog_mobile` и монтируются host-ом через repository boundary; package не создаёт собственный GraphQL client, tenant resolver или locale fallback chain.
- Next.js и Flutter storefront не должны дублировать storage или canonical-routing логику во frontend слое.
- Source of truth для transport и canonical routing остаётся на backend стороне.

## Проверка

- `npm.cmd run verify:storefront:routes`
- точечные storefront contract и smoke checks для затронутых module-owned surfaces
- сверка с [контрактом manifest-слоя](../modules/manifest.md) при изменении UI wiring

## Связанные документы

- [Leptos storefront docs](../../apps/storefront/docs/README.md)
- [Next.js storefront docs](../../apps/next-frontend/docs/README.md)
- [Flutter storefront mobile docs](../../rustok_mobile/apps/rustok_frontend_mobile/README.md)
- [Flutter package catalog/cart](../../rustok_mobile/packages/rustok_catalog_mobile/README.md)
- [Контракты manifest-слоя](../modules/manifest.md)
- [ADR: SSR-first Leptos hosts with headless parity](../../DECISIONS/2026-04-24-ssr-first-leptos-hosts-with-headless-parity.md)
- [UI index](./README.md)
- [Карта документации](../index.md)
