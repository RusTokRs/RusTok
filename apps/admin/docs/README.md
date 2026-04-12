# Документация `apps/admin`

Локальная документация для Leptos-admin host-приложения. Этот файл фиксирует только живой host-level contract; подробные планы, UI-каталоги и rollout-заметки вынесены в отдельные документы.

## Назначение

`apps/admin` является host/composition root для административного интерфейса RusToK. Приложение:

- монтирует host-owned экраны и module-owned admin surfaces;
- держит единый shell, навигацию, RBAC-aware routing и search entrypoint;
- использует `apps/server` как backend surface для GraphQL, Leptos `#[server]` и связанных runtime APIs.

`apps/admin` не должен становиться владельцем бизнес-логики модулей. Если модуль поставляет собственный admin UI, эта поверхность остаётся рядом с модулем и подключается через manifest-driven contract.

## Границы ответственности

`apps/admin` отвечает за:

- host routing, layout, navigation shell и глобальные UI capabilities;
- wiring module-owned admin pages через generated registry;
- host-level locale propagation, auth/session UX и permission-gated navigation;
- интеграцию host-owned операторских сценариев, которые не принадлежат отдельному модулю.

`apps/admin` не отвечает за:

- перенос module-specific CRUD и domain workflows в host-код;
- собственную locale negotiation цепочку внутри module-owned пакетов;
- замену GraphQL transport только потому, что появился Leptos `#[server]` path.

## Runtime contract

- GraphQL и native Leptos `#[server]` path должны сосуществовать параллельно; `#[server]` не заменяет `/api/graphql`.
- Текущий data-layer для admin поддерживает dual-path модель: host сначала использует native `#[server]` surface там, где он уже есть, и только затем откатывается к GraphQL или legacy REST, если это предусмотрено конкретной поверхностью.
- `rustok-pricing/admin` теперь относится к таким dual-path surfaces: pricing package
  по умолчанию ходит в native `#[server]` pricing runtime, оставляя GraphQL
  fallback, и показывает operator-side effective price context для
  `currency + optional region_id + optional price_list_id + optional quantity`,
  включая pricing-owned selector активных price lists, а также выполняет base-price
  variant updates через module-owned server-function transport.
- `apps/admin` остаётся CSR-first host; наличие feature-профилей `hydrate` и `ssr` не означает, что runtime уже перешёл на полноценный SSR/service-layer contract.
- WebSocket transport `/api/graphql/ws` остаётся действующим путём для live update сценариев, включая build/progress и subscription-based surfaces.
- Для целей `module-system` `/modules` считается закрытым repo-side operator surface: установка, удаление, upgrade/deploy модулей и progress feedback доступны из Admin UI без отдельного ручного backend workflow.
- Host-owned `/modules` governance UI не держит локальные policy-эвристики: `registryLifecycle` остаётся summary/read-model, но actor-agnostic `governanceActions` там теперь сведены только к release-management hints (`owner-transfer`, `yank`), а authoritative request-level contract для interactive governance читается отдельным actor-aware fetch к `GET /v2/catalog/publish/{request_id}` по текущему полю `Actor`; `reason` / `reason_code` и request-level availability берутся только из этого статуса.
- Для `apps/admin` это считается конечным repo-side contract: дальше здесь не нужен новый client-owned lifecycle, а только targeted verification mapping и периодическая сверка `/modules` UX с server-driven policy surface.

## Contract для module-owned admin UI

- Источник правды для подключения UI-модулей: `modules.toml` плюс `rustok-module.toml`.
- `apps/admin/build.rs` читает manifest-слой и генерирует wiring в `OUT_DIR`.
- Publishable Leptos admin surface обязан объявлять `[provides.admin_ui].leptos_crate`; наличие `admin/Cargo.toml` само по себе не считается интеграцией.
- Host монтирует module-owned страницы через `/modules/:module_slug` и nested variant `/modules/:module_slug/*module_path`.
- Host прокидывает effective locale через `UiRouteContext.locale`; module-owned Leptos packages обязаны использовать это значение и не должны вводить собственную query/header/cookie fallback-цепочку.
- Core modules с UI подчиняются тому же ownership rule, что и optional modules: наличие UI не делает host владельцем модульной поверхности.
- Route-selection contract тоже host-owned: `apps/admin` санитизирует query по typed schema из
  `rustok-api`, отдаёт модульным пакетам уже canonical route context и предоставляет generic
  Leptos query plumbing через `leptos-ui-routing`.
- Для module-owned admin pages selection state живёт только в URL; отсутствие валидного key ведёт к
  empty state, а invalid/missing entity не должен оставлять stale detail/form state.

## Взаимодействия

- С [документацией `apps/server`](../../server/docs/README.md): backend runtime, GraphQL, `#[server]`, auth/session, registry и health surfaces.
- С [контрактом manifest-слоя](../../../docs/modules/manifest.md): module registration, UI ownership и settings schema.
- С [реестром модулей и приложений](../../../docs/modules/registry.md): карта platform modules, support crates и host applications.
- С module-owned admin packages: host знает только registration contract, route context и secondary nav metadata; внутренний sub-routing и domain UI остаются внутри пакета.

## Проверка

Минимальный локальный путь для изменения `apps/admin`:

- `cargo xtask module validate <slug>` для модулей, чьи admin surfaces затронуты;
- точечные `cargo check` или `cargo test` для затронутых Leptos crates;
- `npm.cmd run verify:i18n:ui` и related contract checks, если затронуты locale bundles или host-provided translations;
- точечная проверка host routing и permission-aware navigation для затронутых экранов.

## Связанные документы

- [План реализации](./implementation-plan.md)
- [Контракты manifest-слоя](../../../docs/modules/manifest.md)
- [Реестр модулей и приложений](../../../docs/modules/registry.md)
- [Каталог Rust UI-компонентов](../../../docs/UI/rust-ui-component-catalog.md)
- [Карта документации](../../../docs/index.md)
