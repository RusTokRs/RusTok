# Документация Leptos Storefront

Локальная документация для `apps/storefront` как SSR-host приложения витрины.

## Текущий runtime contract

- Инвариант: GraphQL transport не удаляется; native `#[server]` functions добавляются как параллельный internal path и должны сосуществовать с GraphQL.
- Host storefront рендерит shell, домашнюю страницу и generic module pages по маршруту `/modules/{route_segment}`.
- Shared data access поддерживает оба пути: Leptos `#[server]` boundary и direct GraphQL HTTP.
- Для storefront сейчас заведены прямые server functions:
  - `/api/fn/storefront/list-enabled-modules`
  - `/api/fn/storefront/resolve-canonical-route`
- Server-side реализация этих функций берёт `AppContext` из `leptos_axum` context и идёт прямо в `rustok-tenant::TenantService` / `rustok-content::CanonicalUrlService`.
- Рядом сохранён GraphQL transport в `shared/api`, а в `shared/context` доступны оба варианта вызова: `*_server` и `*_graphql`.
- Runtime default для `enabled_modules` и canonical-route lookup: сначала native `#[server]`, затем automatic fallback на GraphQL при недоступности native path.
- По умолчанию storefront сейчас использует server-fn preflight `resolve_canonical_route`, но GraphQL-вариант остаётся валидным и не удаляется.
- Если server возвращает alias-hit, storefront отдаёт HTTP redirect на canonical URL до рендера страницы.
- Для canonical lookup параметр `lang` не входит в route key: locale передаётся в query отдельно.
- Если redirect произошёл по alias, storefront сохраняет явно запрошенный `lang` в target URL, чтобы не терять locale SSR.
- Locale lookup внутри canonical preflight идёт с shared fallback policy из `rustok-content`, поэтому alias,
  записанный для `en`, корректно резолвится и для запросов вроде `en-us`, если более точного locale нет.
- Enabled modules резолвятся отдельно и фильтруют storefront registry до рендера.
- Host прокидывает `UiRouteContext` (`locale`, `route_segment`, `query params`) в module-owned storefront packages.
- SSR идёт через in-order HTML streaming, чтобы async module-owned surfaces могли честно получать данные на сервере.

## Generated module UI wiring

- `apps/storefront/build.rs` читает `modules.toml` и модульные `rustok-module.toml`, затем генерирует registry wiring в `OUT_DIR`.
- Publishable storefront UI по-прежнему подключается через `[provides.storefront_ui].leptos_crate`.
- Live generic route `/modules/{route_segment}` остаётся точкой входа для `blog`, `commerce`, `forum`, `pages`, `search` и других publishable storefront packages.

## Canonical routing

- Canonical и alias state хранится не в storefront, а в `rustok-content`.
- Storefront не знает о `content_canonical_urls` / `content_url_aliases`; lookup инкапсулирован в `CanonicalUrlService`.
- Redirect flow может идти через внутренний server-fn слой или через GraphQL; server-fn path сейчас выбран как default internal path.

## Ограничения

- Nested storefront routing и более богатые page-layouts для модулей остаются отдельным слоем поверх текущего generic root-page contract.
- Для внешних crate-ов вне workspace всё ещё нужен publishable storefront package плюс явная server-side install story.

## Рабочие exemplar-ы

- `rustok-blog-storefront` — content read-path с generic module page и data-driven публикациями.
- `rustok-pages-storefront` — page-driven surface поверх того же host contract.
- `rustok-forum-storefront` — forum read-path без storefront-specific логики в host.
- `rustok-commerce-storefront` — public catalog read-path, теперь подключённый через `[provides.storefront_ui]`.
- `rustok-search-storefront` — storefront slot/page exemplar с manifest-driven route и search-specific UX внутри пакета модуля.

## Связанные документы

- [План реализации](./implementation-plan.md)
- [Заметки по storefront UI](../../../docs/UI/storefront.md)
- [Карта документации](../../../docs/index.md)
