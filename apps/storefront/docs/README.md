# Документация Leptos Storefront

Локальная документация для `apps/storefront` как SSR-host приложения витрины.

## Текущий runtime contract

- Инвариант: GraphQL transport не удаляется; native `#[server]` functions добавляются как параллельный internal path и должны сосуществовать с GraphQL.
- Host storefront рендерит shell, домашнюю страницу и generic module pages по семейству маршрутов `/modules/{route_segment}` и `/{locale}/modules/{route_segment}`.
- Shared data access поддерживает оба пути: Leptos `#[server]` boundary и direct GraphQL HTTP.
- Для storefront сейчас заведены прямые server functions:
  - `/api/fn/storefront/list-enabled-modules`
  - `/api/fn/storefront/resolve-canonical-route`
  - `/api/fn/pages/storefront-data`
  - `/api/fn/blog/storefront-data`
  - `/api/fn/commerce/storefront-data`
  - `/api/fn/forum/storefront-data`
  - `/api/fn/search/storefront-search`
  - `/api/fn/search/storefront-filter-presets`
  - `/api/fn/search/storefront-suggestions`
  - `/api/fn/search/storefront-track-click`
- Server-side реализация этих функций берёт `AppContext` из `leptos_axum` context и идёт прямо в `rustok-tenant::TenantService` / `rustok-content::CanonicalUrlService`.
- Рядом сохранён GraphQL transport в `shared/api`, а в `shared/context` доступны оба варианта вызова: `*_server` и `*_graphql`.
- Runtime default для `enabled_modules` и canonical-route lookup: сначала native `#[server]`, затем automatic fallback на GraphQL при недоступности native path.
- Такой же native-first + GraphQL-fallback path теперь используется и в module-owned storefront packages `rustok-pages-storefront` и `rustok-blog-storefront`; GraphQL в этих пакетах не удаляется.
- Такой же native-first + GraphQL-fallback path теперь заведён и для `rustok-commerce-storefront`, `rustok-forum-storefront`, `rustok-search-storefront`; GraphQL во всех module-owned storefront пакетах сохраняется.
- По умолчанию storefront сейчас использует server-fn preflight `resolve_canonical_route`, но GraphQL-вариант остаётся валидным и не удаляется.
- Если server возвращает alias-hit, storefront отдаёт HTTP redirect на canonical URL до рендера страницы.
- Host storefront теперь поддерживает locale-prefixed SSR routes: `/`, `/{locale}`, `/modules/{route_segment}` и `/{locale}/modules/{route_segment}`.
- Для canonical lookup legacy-параметр `lang` не входит в route key: effective locale сначала берётся из path prefix, затем из `?lang=` как backward-compatible fallback.
- Если redirect произошёл по alias внутри locale-prefixed route, storefront сохраняет locale в path (`/ru/...`), а не возвращает старую query-only форму.
- Locale lookup внутри canonical preflight идёт с shared fallback policy из `rustok-content`, поэтому alias,
  записанный для `en`, корректно резолвится и для запросов вроде `en-us`, если более точного locale нет.
- Enabled modules резолвятся отдельно и фильтруют storefront registry до рендера.
- Host прокидывает `UiRouteContext` (`locale`, `route_segment`, `query params`) в module-owned storefront packages.
- Module-owned storefront packages обязаны строить внутренние ссылки на свои generic pages через `UiRouteContext::module_route_base()`, а не через hardcoded `/modules/{route_segment}`.
- Module-owned storefront packages не должны вводить собственную locale negotiation policy: path/query/header/cookie resolution остаётся host/runtime contract, а пакет использует уже переданный `UiRouteContext.locale`.
- SSR идёт через in-order HTML streaming, чтобы async module-owned surfaces могли честно получать данные на сервере.

## Generated module UI wiring

- `apps/storefront/build.rs` читает `modules.toml` и модульные `rustok-module.toml`, затем генерирует registry wiring в `OUT_DIR`.
- Publishable storefront UI по-прежнему подключается через `[provides.storefront_ui].leptos_crate`.
- Live generic route family `/modules/{route_segment}` + `/{locale}/modules/{route_segment}` остаётся точкой входа для `blog`, `commerce`, `forum`, `pages`, `search` и других publishable storefront packages.
- Лёгкая верификация этого contract теперь выполняется через `npm run verify:storefront:routes`.

## Canonical routing

- Canonical и alias state хранится не в storefront, а в `rustok-content`.
- Storefront не знает о `content_canonical_urls` / `content_url_aliases`; lookup инкапсулирован в `CanonicalUrlService`.
- Redirect flow может идти через внутренний server-fn слой или через GraphQL; server-fn path сейчас выбран как default internal path.

## Ограничения

- Nested storefront routing и более богатые page-layouts для модулей остаются отдельным слоем поверх текущего generic root-page contract.
- Для внешних crate-ов вне workspace всё ещё нужен publishable storefront package плюс явная server-side install story.

## Рабочие exemplar-ы

- `rustok-blog-storefront` — module-owned blog surface с native `#[server]` read-path через `PostService` и обязательным GraphQL fallback.
- `rustok-commerce-storefront` — catalog/package surface с native `#[server]` boundary и GraphQL fallback для списка и selected product detail.
- `rustok-forum-storefront` — module-owned forum surface с native `#[server]` boundary и GraphQL fallback для categories/topics/replies.
- `rustok-pages-storefront` — page-driven surface с native `#[server]` read-path через `PageService` и обязательным GraphQL fallback.
- `rustok-search-storefront` — search/package surface с native `#[server]` boundary и GraphQL fallback для preview, presets, suggestions и click tracking.
- `rustok-forum-storefront` — forum read-path без storefront-specific логики в host.
- `rustok-commerce-storefront` — public catalog read-path, теперь подключённый через `[provides.storefront_ui]`.
- `rustok-search-storefront` — storefront slot/page exemplar с manifest-driven route и search-specific UX внутри пакета модуля.

## Связанные документы

- [План реализации](./implementation-plan.md)
- [Заметки по storefront UI](../../../docs/UI/storefront.md)
- [Карта документации](../../../docs/index.md)
