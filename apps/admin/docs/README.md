# Документация `apps/admin`

Локальная документация для Leptos-admin host-приложения. Этот файл фиксирует только живой host-level contract; подробные планы, UI-каталоги и rollout-заметки вынесены в отдельные документы.

## Назначение

`apps/admin` является host/composition root для административного интерфейса RusToK. Preferred product runtime для Leptos admin — SSR/hydrate в monolith deployment, при этом standalone CSR сохраняется как debug/compatibility профиль. Приложение:

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

- `apps/admin` поддерживает три разных runtime-профиля, которые нельзя смешивать:
  `csr` для standalone Trunk/WASM, `hydrate` для клиентской половины SSR и `ssr` для server-side
  половины/monolith.
- Preferred product path для Leptos admin — `ssr` + `hydrate` поверх `apps/server` как same-origin backend. В этом профиле native `#[server]` transport является preferred internal data-layer.
- В `csr` profile базовый transport не должен требовать Leptos `#[server]`: GraphQL, auth и REST идут
  напрямую в `apps/server` через `/api/graphql`, `/api/auth/*` и module-owned REST endpoints. Локальный
  `trunk serve` обязан проксировать `/api/*` в `http://localhost:5150/api/*`. Этот профиль нужен для debug/compatibility, а не как production default.
- В `hydrate`/`ssr` и monolith profile native `#[server]` endpoints `/api/fn/*` считаются доступными
  на том же backend origin и могут быть preferred path для surfaces, где нужен server-side runtime.
- Если surface поддерживает dual-path модель, fallback в GraphQL/REST обязан реально работать в `csr`;
  `#[server]` не может быть единственным critical transport для standalone debug.
- GraphQL и native Leptos `#[server]` path должны сосуществовать параллельно; `#[server]` не заменяет `/api/graphql`.
- Причина split-а: monolith admin выигрывает от same-origin SSR/hydrate, server-side auth/session/policy и короткого Rust-пути через `#[server]`, но headless и standalone debug требуют живой GraphQL/REST fallback.
- Текущий data-layer для admin поддерживает dual-path модель: host сначала использует native `#[server]` surface там, где он уже есть, и только затем откатывается к GraphQL или legacy REST, если это предусмотрено конкретной поверхностью.
- `rustok-pricing/admin` теперь относится к таким dual-path surfaces: pricing package
  по умолчанию ходит в native `#[server]` pricing runtime, оставляя GraphQL
  fallback, и показывает operator-side effective price context для
  `currency + optional region_id + optional price_list_id + optional quantity`,
  включая pricing-owned selector активных price lists, а также выполняет base-price
  variant updates через module-owned server-function transport.
- `apps/admin` не считается CSR-first host. CSR остаётся обязательным standalone debug профилем, но архитектурный target для Leptos admin — SSR-first host с headless GraphQL/REST parity.
- WebSocket transport `/api/graphql/ws` остаётся действующим путём для live update сценариев, включая build/progress и subscription-based surfaces.
- Host-owned `/install` является Leptos wizard-слоем для гибридного установщика.
  Он не содержит собственной bootstrap-логики: экран собирает `InstallPlan`,
  вызывает `/api/install/preflight`, запускает `/api/install/apply`, poll-ит
  `/api/install/jobs/{job_id}` и показывает persisted receipts из
  `/api/install/sessions/{session_id}/receipts`. CLI `rustok-server install ...`
  остаётся canonical automation/operator path, а web слой работает как thin
  facade поверх `apps/server` и `rustok-installer`. Этот route доступен до
  обычной admin-auth, потому что первый install ещё может не иметь созданного
  superadmin; mutating install-запросы защищаются setup-token guard на
  `/api/install/*`. Wizard не подставляет sample admin password и admin
  PostgreSQL URL по умолчанию: production-like secret values должны приходить
  через secret refs, а database creation является явным opt-in с обязательным
  `pg_admin_url`.
- Для целей `module-system` `/modules` считается закрытым repo-side operator surface: установка, удаление, upgrade/deploy модулей и progress feedback доступны из Admin UI без отдельного ручного backend workflow.
- Host-owned `/modules` governance UI не держит локальные policy-эвристики: `registryLifecycle` остаётся summary/read-model, но actor-agnostic `governanceActions` там теперь сведены только к release-management hints (`owner-transfer`, `yank`), а authoritative request-level contract для interactive governance читается отдельным bearer-auth fetch к `GET /v2/catalog/publish/{request_id}`; `reason` / `reason_code` и request-level availability берутся только из этого статуса.
- `/modules` больше не читает legacy registry audit shape: lifecycle/event read-side работает только с typed payload (`stage_key`, nested `owner_transition`, structured principal objects) и не парсит historical `*_actor` keys.
- Для `apps/admin` это считается конечным repo-side contract: дальше здесь не нужен новый client-owned lifecycle, а только targeted verification mapping и периодическая сверка `/modules` UX с server-driven policy surface.

## Локальный debug-запуск

Для локальной отладки без Docker используйте `localhost`, а не `127.0.0.1`: на Windows loopback через `127.0.0.1`
может принимать TCP-соединение и не отдавать HTTP-ответ. Рабочий профиль:

```powershell
# backend уже должен слушать http://localhost:5150
$env:RUSTOK_MODULES_MANIFEST = (Resolve-Path ..\..\modules.local.toml)
$env:PATH="$env:USERPROFILE\.rustok\tools\trunk;$env:PATH"
trunk serve --address ::1 --port 3001
```

Для этого профиля backend запускается с `modules.local.toml`, где embedded admin/storefront отключены. Корневой
`modules.toml` описывает monolith/release composition и требует `embed-admin`; в текущем Windows debug-окружении
SSR-сборка embedded `apps/admin` падает по памяти (`rustc-LLVM ERROR: out of memory`), поэтому локальный debug
разделяет backend и внешний Trunk host.

`apps/admin` в standalone debug работает как CSR host, поэтому Trunk должен собирать именно binary artifact
`rustok-admin`, а не library artifact `rustok_admin`. Это зафиксировано в `index.html` через
`data-target-name="rustok-admin"`: binary запускает `main()` и монтирует shell в `body`.

Tailwind CSS для этого debug-профиля собирается Trunk post-build hook-ом `scripts\tailwind-build.cmd`.
Hook пишет `output.css` в `TRUNK_STAGING_DIR`, поэтому CSS переживает очистку `dist` внутри Trunk pipeline.
Локально команду можно прогнать отдельно только для быстрой проверки CSS:

```powershell
npm.cmd install
npm.cmd run tw:build
```

`apps/admin/input.css` использует Tailwind v4 `@import "tailwindcss"` и явные `@source` entries. `tailwind.config.js`
должен включать `apps/admin/src`, shared Leptos UI crates и module-owned admin UI packages
`crates/**/admin/src/**/*.rs`. Если `dist/output.css` отсутствует или source globs не покрывают модульные UI-пакеты,
shell загрузится частично или без стилей. Это не меняет production target: архитектурный путь для Leptos admin остаётся
SSR/hydrate поверх `apps/server`, а CSR нужен для standalone debug и проверки module-owned UI packages.

Leptos admin не должен визуально расходиться с Next admin как отдельный продукт. Auth shell, navigation shell,
route-selection UX и контейнеры module-owned UI должны следовать общему admin UI contract. Next admin остаётся
параллельным React/Next host, а Leptos admin — canonical operator surface для SSR/monolith пути; найденные
расхождения оформляются как parity debt и чинятся точечно.

## Contract для module-owned admin UI

- Источник правды для подключения UI-модулей: `modules.toml` плюс `rustok-module.toml`.
- `apps/admin/build.rs` читает manifest-слой и генерирует wiring в `OUT_DIR`.
- Publishable Leptos admin surface обязан объявлять `[provides.admin_ui].leptos_crate`; наличие `admin/Cargo.toml` само по себе не считается интеграцией.
- Host монтирует module-owned страницы через `/modules/:module_slug` и nested variant `/modules/:module_slug/*module_path`.
- Sidebar строится из manifest-driven navigation metadata. `[provides.admin_ui].nav_group` и `nav_order` являются optional overrides; если они не заданы, host группирует first-party modules по стандартным buckets `Content`, `Commerce`, `Runtime`, `Governance`, `Automation`, `Other`.
- Canonical source для подменю модуля — `[[provides.admin_ui.child_pages]]`. Legacy `[[provides.admin_ui.pages]]` пока читается только как compatibility alias, новые manifests должны использовать `child_pages`.
- Каждый module-owned admin surface получает корневой пункт `Overview`; declared child pages становятся nested links под контейнером модуля. Host скрывает disabled tenant modules и пустые containers.
- Tenant/module settings остаются в host-owned `/modules` governance UI. Если `rustok-module.toml` содержит `[settings]`, sidebar добавляет контекстный link `/modules?module_slug=<slug>`; module-owned packages не дублируют этот editor.
- Host прокидывает effective locale через `UiRouteContext.locale`; module-owned Leptos packages обязаны использовать это значение и не должны вводить собственную query/header/cookie fallback-цепочку.
- Module-owned admin packages обязаны поддерживать тот же runtime split: `#[server]` preferred в SSR/hydrate, GraphQL/REST fallback для standalone CSR/debug. Пакет не должен становиться ни GraphQL-only для monolith, ни `#[server]`-only для headless/debug.
- Core modules с UI подчиняются тому же ownership rule, что и optional modules: наличие UI не делает host владельцем модульной поверхности.
- Route-selection contract тоже host-owned: `apps/admin` санитизирует query по typed schema из
  `rustok-api`, отдаёт модульным пакетам уже canonical route context и предоставляет generic
  Leptos query plumbing через `leptos-ui-routing`.
- `rustok-seo-admin` после cutover уже не держит entity selection/state вообще: route `seo`
  использует только `tab` для control-plane navigation, а page/product/blog/forum SEO authoring
  живёт в owner-module пакетах.
- Тот же `rustok-seo-admin` держит route/query orchestration в shell-компоненте, а bulk/redirects и
  sitemaps/robots/defaults/diagnostics рендерит через отдельные section components внутри пакета,
  не перенося этот UI split в host.
- Canonical ownership при этом зафиксирован отдельно: entity SEO authoring должно жить в owner-module
  admin packages (`pages`, `product`, `blog`, `forum`), а `rustok-seo-admin` после cutover остаётся
  только cross-cutting SEO infrastructure surface.
- Этот cutover уже начат в коде: `rustok-pages/admin`, `rustok-product/admin` и `rustok-blog/admin`
  встраивают owner-side SEO panels через `rustok-seo-admin-support`, а `rustok-forum/admin`
  держит capability slot до появления forum targets в shared runtime.
- Для module-owned admin pages selection state живёт только в URL; отсутствие валидного key ведёт к
  empty state, а invalid/missing entity не должен оставлять stale detail/form state.

## Взаимодействия

- С [документацией `apps/server`](../../server/docs/README.md): backend runtime, GraphQL, `#[server]`, auth/session, registry и health surfaces.
- С [ADR гибридного установщика](../../../DECISIONS/2026-04-26-hybrid-installer-architecture.md): installer-core, canonical CLI, HTTP adapter и thin Leptos wizard layering.
- С [контрактом manifest-слоя](../../../docs/modules/manifest.md): module registration, UI ownership и settings schema.
- С [реестром модулей и приложений](../../../docs/modules/registry.md): карта platform modules, support crates и host applications.
- С module-owned admin packages: host знает только registration contract, route context и secondary nav metadata; внутренний sub-routing и domain UI остаются внутри пакета.

## Проверка

Минимальный локальный путь для изменения `apps/admin`:

- `cargo xtask module validate <slug>` для модулей, чьи admin surfaces затронуты;
- точечные `cargo check` или `cargo test` для затронутых Leptos crates;
- `npm run verify:i18n:ui` и related contract checks, если затронуты locale bundles или host-provided translations;
- точечная проверка host routing и permission-aware navigation для затронутых экранов.

## Связанные документы

- [План реализации](./implementation-plan.md)
- [Контракты manifest-слоя](../../../docs/modules/manifest.md)
- [Реестр модулей и приложений](../../../docs/modules/registry.md)
- [Каталог Rust UI-компонентов](../../../docs/UI/rust-ui-component-catalog.md)
- [ADR: SSR-first Leptos hosts with headless parity](../../../DECISIONS/2026-04-24-ssr-first-leptos-hosts-with-headless-parity.md)
- [Карта документации](../../../docs/index.md)
