# Витрина: host и contract

RusToK поддерживает два storefront host-приложения:

- `apps/storefront` — основной Leptos SSR host;
- `apps/next-frontend` — параллельный Next.js host.

Обе реализации должны сохранять единый backend, routing, locale и module contract. Leptos storefront остаётся основным Rust-host путём, Next.js storefront — headless-параллелью.

## Host contract

- Host рендерит shell и generic module pages.
- Module-owned storefront packages подключаются через manifest-driven wiring.
- Generic storefront routes живут в семействе `/modules/{route_segment}` с locale-aware вариантом там, где это требуется host runtime.
- Module-owned packages обязаны строить внутренние ссылки через host route context, а не через hardcoded route strings.

## Data-layer contract

- Для Leptos storefront путь по умолчанию: `UI -> local API -> #[server] -> service layer`.
- Внешний GraphQL contract `/api/graphql` остаётся обязательным и поддерживаемым параллельным путём.
- Host сначала использует native `#[server]` surface там, где он уже есть, и только затем откатывается к GraphQL, если это предусмотрено runtime contract.
- Новый module-owned storefront UI не должен проектироваться как GraphQL-only, если может жить через `#[server]`.

## Canonical routing и locale

- Canonical URL policy и alias storage живут в backend/domain слое, а не в storefront host.
- Storefront использует backend preflight для canonical route resolution до рендера страницы.
- Effective locale выбирается runtime/host слоем один раз и затем прокидывается в UI surface.
- Query-based locale fallback допустим только как backward-compatible path; module-owned UI не должен вводить свою fallback-цепочку.

## Parity с Next.js storefront

- `apps/next-frontend` обязан сохранять parity с `apps/storefront` по route, auth, i18n и backend contracts.
- Next.js storefront не должен дублировать storage или canonical-routing логику во frontend слое.
- Source of truth для transport и canonical routing остаётся на backend стороне.

## Проверка

- `npm.cmd run verify:storefront:routes`
- точечные storefront contract и smoke checks для затронутых module-owned surfaces
- сверка с [контрактом manifest-слоя](../modules/manifest.md) при изменении UI wiring

## Связанные документы

- [Leptos storefront docs](../../apps/storefront/docs/README.md)
- [Next.js storefront docs](../../apps/next-frontend/docs/README.md)
- [Контракты manifest-слоя](../modules/manifest.md)
- [UI index](./README.md)
- [Карта документации](../index.md)
