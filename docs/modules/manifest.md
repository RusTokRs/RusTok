# Контракт `modules.toml` и `rustok-module.toml`

Этот документ описывает два связанных слоя модульного контракта RusToK:

- `modules.toml` в корне репозитория задаёт состав платформенных модулей для конкретной сборки.
- `rustok-module.toml` внутри path-модуля задаёт publish/runtime/UI-контракт самого модуля.

`modules.toml` отвечает за composition root. `rustok-module.toml` отвечает за identity, surface wiring, UI-пакеты и publish-ready metadata.

## Где какой контракт живёт

### `modules.toml`

Корневой manifest фиксирует:

- список платформенных модулей, попадающих в сборку;
- источник каждого модуля: `path`, `git`, `crates-io`/`registry`;
- coarse-grained зависимости через `depends_on`;
- platform-level settings, включая `settings.default_enabled`.

Это runtime/build-level контракт всей платформы, а не отдельного crate.

### `rustok-module.toml`

Локальный manifest path-модуля фиксирует:

- `module.slug`, `module.name`, `module.version`, `module.description`;
- `module.ui_classification`;
- `[crate].entry_type` для runtime-модуля;
- runtime-точки входа модуля;
- admin/storefront UI wiring;
- module-owned schema настроек;
- marketplace/publish metadata;
- зависимости и конфликты, относящиеся к самому модулю.

Для path-модулей из `modules.toml` наличие `rustok-module.toml` обязательно.

### `module.ui_classification`

`module.ui_classification` обязателен для каждого path-модуля и должен совпадать с фактическим UI wiring.

Поддерживаемые значения:

- `dual_surface`
- `admin_only`
- `storefront_only`
- `no_ui`
- `capability_only`
- `future_ui`

Практическое правило для текущего platform scope:

- модуль с `[provides.admin_ui]` и `[provides.storefront_ui]` должен иметь `dual_surface`;
- модуль только с `[provides.admin_ui]` должен иметь `admin_only`;
- модуль только с `[provides.storefront_ui]` должен иметь `storefront_only`;
- модуль без UI может использовать `no_ui`, `capability_only` или `future_ui`, но не должен одновременно объявлять UI sub-crates.

### `[crate].entry_type`

Если crate реализует `RusToKModule`, `rustok-module.toml` обязан содержать `[crate].entry_type`, совпадающий с реальным runtime entry type из `src/lib.rs`.

Практическое правило:

- `pub struct BlogModule;` + `impl RusToKModule for BlogModule` требуют `entry_type = "BlogModule"`;
- capability crate без `RusToKModule` может не объявлять `entry_type`.

### Синхронизация runtime metadata

Если crate реализует `RusToKModule`, значения `module.slug`, `module.name` и `module.description`
в `rustok-module.toml` должны совпадать с `slug()`, `name()` и `description()` в `src/lib.rs`.

### `provides.graphql` и `provides.http`

Если `rustok-module.toml` объявляет:

- `[provides.graphql].query`
- `[provides.graphql].mutation`
- `[provides.http].routes`
- `[provides.http].webhook_routes`

то соответствующие type/function symbols должны реально существовать внутри `src/**/*.rs`
модуля. Manifest не должен ссылаться на декоративные или уже удалённые transport surfaces.

## Обязательный минимум для path-модуля

Каждый path-модуль из `modules.toml` должен иметь:

- `Cargo.toml`;
- корневой `README.md` на английском;
- `docs/README.md` на русском;
- `docs/implementation-plan.md` на русском;
- `rustok-module.toml`.

Корневой `README.md` считается частью acceptance contract и должен содержать:

- `## Purpose`
- `## Responsibilities`
- `## Entry points`
- `## Interactions`
- ссылку на локальный `docs/README.md`

Локальные docs нужны даже для модулей без admin/storefront UI.

### Минимальный контракт документации

Для path-модуля контракт документации считается закрытым, только если соблюдены оба слоя:

- корневой `README.md` на английском с разделами `Purpose`, `Responsibilities`, `Entry points`, `Interactions` и ссылкой на `docs/README.md`;
- локальный `docs/README.md` на русском как живой runtime/module contract;
- локальный `docs/implementation-plan.md` на русском как живой план доведения модуля до целевого состояния.

Минимальный каркас локального `docs/README.md`:

- `## Назначение`
- `## Зона ответственности`
- `## Интеграция`
- `## Проверка`
- `## Связанные документы`

Минимальный каркас локального `docs/implementation-plan.md`:

- `## Область работ`
- `## Текущее состояние`
- `## Этапы`
- `## Проверка`
- `## Правила обновления`

Дополнительные разделы допустимы, но этот минимум должен сохраняться.

## Что проверяет `cargo xtask module validate`

`cargo xtask module validate <slug>` работает только для slug из `modules.toml` и валидирует фактический scoped contract:

`cargo xtask module validate` без slug проходит все локальные `source = "path"` модули из `modules.toml`. Это не auto-discovery по `crates/`: новый crate становится платформенным модулем только после добавления в `[modules]`.

- slug существует в `modules.toml`;
- для `source = "path"` задан `path`;
- `rustok-module.toml` существует по ожидаемому пути;
- `module.slug` совпадает со slug из `modules.toml`;
- `module.version` в `rustok-module.toml` совпадает с версией из `Cargo.toml`;
- `module.ui_classification` существует, использует поддерживаемое значение и согласован с реальными UI surfaces;
- если crate реализует `RusToKModule`, `[crate].entry_type` существует и совпадает с runtime entry type;
- если crate реализует `RusToKModule`, `module.slug`, `module.name` и `module.description` совпадают с runtime metadata в `src/lib.rs`;
- если модуль помечен как `required = true` в `modules.toml`, runtime-тип явно возвращает `ModuleKind::Core`; optional-модуль не объявляет `ModuleKind::Core`;
- если crate реализует `RusToKModule`, его `permissions()` не содержит дублей, использует только существующие `Permission::*` constants или валидные пары `Resource::*/Action::*` из `rustok-core`, и покрывает минимальный runtime RBAC surface там, где этот минимум уже зафиксирован platform contract-ами;
- для модулей, у которых event-driven behavior уже переведён в module-owned runtime path (`index`, `search`, `workflow`), `src/lib.rs` публикует listeners через `register_event_listeners(...)`, а не откатывается к скрытому host-owned wiring;
- для `workflow` webhook ingress остаётся module-owned surface: модуль держит `controllers::webhook_routes()`, а `apps/server` только реэкспортирует его через shim; cron path не смешивается с webhook/event listener wiring;
- boundary `index != search` остаётся жёстким runtime contract: `index` публикует indexing/read-model substrate и module-owned listeners, а `search` публикует `SearchEngineKind`, `PgSearchEngine`, `SearchIngestionHandler`, `search_documents` и search UX/diagnostics surfaces, не смешивая эти слои в одном модуле;
- `search` удерживает operator-plane contract как часть module surface: `SearchDiagnosticsService`, `SearchAnalyticsService`, `SearchSettingsService`, `SearchDictionaryService`, documented control-plane markers в `README.md` и локальный `docs/observability-runbook.md` не считаются опциональным шумом и не должны теряться при рефакторинге;
- если объявлены `[provides.graphql]` или `[provides.http]`, соответствующие symbols реально существуют в коде модуля;
- если для модуля существует server shim в `apps/server/src/controllers/<slug>/`, то он экспортирует `pub routes()` и/или `pub webhook_routes()` для всех объявленных HTTP surfaces;
- `package.license` резолвится через `Cargo.toml` или workspace inheritance;
- `module.description` достаточно полон для publish readiness;
- `depends_on` из `modules.toml`, `[dependencies]` в `rustok-module.toml` и `RusToKModule::dependencies()` не расходятся;
- optional-модуль имеет feature `mod-<slug>` в `apps/server/Cargo.toml`, этот feature резолвится в реальный `ModuleRegistry` entry, а его `mod-*` зависимости совпадают с `depends_on` из `modules.toml`;
- для `capability_only` ghost module допустим always-linked server dependency path: `mod-<slug>` может быть пустым feature-guard'ом для registry/codegen wiring, если сам crate уже подключён в `apps/server` как shared capability dependency;
- `required = true` модуль регистрируется напрямую в `apps/server/src/modules/mod.rs`, а optional-модуль не попадает туда в обход feature/codegen wiring;
- `settings.default_enabled` перечисляет только optional-модули; required/core-модули туда не включаются и считаются всегда активными;
- `settings.default_enabled` образует dependency-closed optional graph: если optional-модуль включён по умолчанию, его optional-зависимости тоже присутствуют в `default_enabled`;
- каждый slug из `settings.default_enabled` присутствует в default feature-set сервера как `mod-<slug>`;
- root `README.md`, `docs/README.md` и `docs/implementation-plan.md` присутствуют и соответствуют минимальному формату;
- wiring для `admin/` и `storefront/` согласован с `[provides.admin_ui]` и `[provides.storefront_ui]`;
- если UI sub-crate объявлен в manifest, его `Cargo.toml` реально существует и версия совпадает с версией основного модуля.
- `[provides.admin_ui]` требует не только `leptos_crate`, но и непустые `route_segment`, `nav_label` и `[provides.admin_ui.i18n]` с `default_locale`, `supported_locales`, `leptos_locales_path`.
- `[provides.admin_ui].nav_group` и `nav_order` являются optional navigation metadata для host sidebar. Если они не заданы, `apps/admin` использует стандартные группы `Content`, `Commerce`, `Runtime`, `Governance`, `Automation`, `Other`.
- `[[provides.admin_ui.child_pages]]` является canonical metadata для nested admin navigation: каждый пункт объявляет `subpath`, `title`, `nav_label` и монтируется под `/modules/:route_segment/:subpath`. Старое имя `[[provides.admin_ui.pages]]` допускается только как compatibility alias.
- Наличие `[settings]` в `rustok-module.toml` не создаёт module-owned settings page. Host показывает contextual settings link в `/modules?module_slug=<slug>` и использует существующий tenant settings editor.
- `[provides.storefront_ui]` требует не только `leptos_crate`, но и непустые `slot`, `route_segment`, `page_title` и `[provides.storefront_ui.i18n]` с `default_locale`, `supported_locales`, `leptos_locales_path`.
- если UI sub-crate объявлен в manifest, соответствующий host (`apps/admin` или `apps/storefront`) реально подключает его как dependency и прокидывает обязательные host feature links (`/hydrate`, `/ssr`) там, где sub-crate их экспортирует.
- host dependency на UI sub-crate указывает на канонический путь модуля (`crates/<module>/admin` или `crates/<module>/storefront`), а не на произвольный совместимый crate с тем же именем.
- если модуль публикует `admin_ui` или `storefront_ui`, host-композиция включает UI-поверхности его прямых модульных зависимостей для того же surface, когда эти зависимости тоже публикуют такой UI.
- `apps/admin` и `apps/storefront` не содержат orphaned first-party UI dependencies: path-зависимость на `crates/*/admin` или `crates/*/storefront` допустима только если соответствующий `rustok-module.toml` действительно объявляет этот crate как `admin_ui` или `storefront_ui`.
- `apps/admin` и `apps/storefront` не содержат orphaned host feature entries: `hydrate`/`ssr` не ссылаются на `crate/feature` для first-party module UI crate, если этот crate больше не объявлен в module manifest или уже не подключён как dependency host-а.
- central navigation не отстаёт от manifest-wiring: `docs/modules/_index.md` содержит docs/plan links модуля, а `docs/modules/UI_PACKAGES_INDEX.md` перечисляет объявленные admin/storefront UI-поверхности.

Если slug отсутствует в `modules.toml`, `xtask` возвращает `Unknown module slug`.

## Что проверяет `cargo xtask validate-manifest`

`cargo xtask validate-manifest` проверяет центральный composition contract:

- `modules.toml` парсится и использует поддерживаемую schema version;
- `default_enabled` ссылается только на реально объявленные модули;
- `depends_on` не содержит отсутствующих slug;
- `source`-спецификация валидна для каждого модуля;
- `apps/server` держит module-owned event runtime path: общий `module_event_dispatcher`, без legacy `index/search` dispatchers и без ручного wiring `WorkflowTriggerHandler` в host runtime;
- все path-модули действительно содержат `rustok-module.toml`.

Этот шаг не заменяет `cargo xtask module validate <slug>`, а дополняет его.

Описание самого workspace-инструмента, его зон ответственности и operator entrypoints живёт в [`xtask/README.md`](../../xtask/README.md).

## Минимальный пример `modules.toml`

```toml
schema = 2
app = "rustok-server"

[modules]
blog = { crate = "rustok-blog", source = "path", path = "crates/rustok-blog", depends_on = ["content"] }
content = { crate = "rustok-content", source = "path", path = "crates/rustok-content" }

[settings]
default_enabled = ["content", "blog"]
```

## Минимальный пример `rustok-module.toml`

```toml
[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
description = "Blog module with admin and storefront surfaces."
ownership = "first_party"
trust_level = "verified"

[crate]
entry_type = "BlogModule"

[provides.graphql]
query = "graphql::BlogQuery"
mutation = "graphql::BlogMutation"

[provides.http]
routes = "controllers::routes"

[provides.admin_ui]
leptos_crate = "rustok-blog-admin"
route_segment = "blog"
nav_label = "Blog"
nav_group = "Content"
nav_order = 20

[[provides.admin_ui.child_pages]]
subpath = "posts"
title = "All Blog Posts"
nav_label = "All Posts"

[[provides.admin_ui.child_pages]]
subpath = "new"
title = "Add Blog Post"
nav_label = "Add Post"

[provides.storefront_ui]
leptos_crate = "rustok-blog-storefront"
route_segment = "blog"
page_title = "Blog"
slot = "home_after_catalog"

[marketplace]
category = "content"
publisher = "rustok"
tags = ["blog", "editorial"]
description = "Blog module with admin and storefront surfaces."
```

## Инварианты для UI sub-crates

- Наличие `admin/Cargo.toml` без `[provides.admin_ui].leptos_crate` считается ошибкой wiring.
- Наличие `storefront/Cargo.toml` без `[provides.storefront_ui].leptos_crate` считается ошибкой wiring.
- Объявление `[provides.admin_ui].leptos_crate` без реального `admin/Cargo.toml` считается ошибкой.
- Объявление `[provides.storefront_ui].leptos_crate` без реального `storefront/Cargo.toml` считается ошибкой.
- Версии UI sub-crates должны совпадать с версией основного модуля.

Само наличие подпапки `admin/` или `storefront/` не считается доказательством интеграции. Канонический источник правды здесь — manifest wiring.

## Support и capability crates

Не каждый crate из workspace является платформенным модулем.

- Platform modules живут в `modules.toml` и проходят scoped validation через `cargo xtask module validate <slug>`.
- Foundation/shared/support/capability crates могут иметь локальные docs и собственные контракты, но не обязаны иметь slug в `modules.toml`.
- Если capability crate нужен formal runtime/module contract, его можно завести в `modules.toml` как `capability_only` ghost module. Текущие живые примеры такого паттерна: `alloy` и `flex`.

Для таких crates всё равно действует documentation minimum:

- корневой `README.md`;
- при необходимости `docs/README.md`;
- при необходимости `docs/implementation-plan.md`.

Если support/capability crate уже публикует локальные docs, для него рекомендуется тот же структурный стандарт, что и для платформенных модулей: английский root `README.md`, русский `docs/README.md`, русский `docs/implementation-plan.md`.

Но они не проходят `module validate`, пока не становятся платформенным модулем.

## Как добавить новый платформенный модуль

`xtask` узнаёт о новом платформенном модуле только из `modules.toml`. Наличие crate в `crates/` само по себе не делает его модулем.

Минимальный порядок добавления:

1. Создать crate, обычно `crates/rustok-<slug>/`, и убедиться, что он входит в Cargo workspace.
2. Добавить обязательные локальные документы: корневой `README.md`, `docs/README.md`, `docs/implementation-plan.md`.
3. Добавить `rustok-module.toml` с корректными `module.slug`, `module.version`, `module.ui_classification`, metadata зависимостей и `[crate].entry_type`, если crate реализует `RusToKModule`.
4. Добавить slug в `[modules]` внутри `modules.toml`; `required = true` использовать только для core-модулей, остальные модули оставлять optional.
5. Синхронизировать зависимости в трёх местах: `modules.toml.depends_on`, `[dependencies]` в `rustok-module.toml`, `RusToKModule::dependencies()`.
6. Для optional runtime-модуля добавить `mod-<slug>` feature и server wiring в `apps/server/Cargo.toml`.
   Для обычного optional-модуля это означает `dep:<crate>`, а для `capability_only` ghost module допустим пустой feature-guard, если crate уже always-linked как shared capability dependency сервера.
7. Для required runtime-модуля добавить прямую регистрацию в `apps/server/src/modules/mod.rs`.
8. Для module-owned UI объявлять `[provides.admin_ui]` и/или `[provides.storefront_ui]` только вместе с реальным UI sub-crate и host wiring.
9. Обновить навигацию: `docs/modules/_index.md`, `docs/modules/registry.md`, а для UI-модулей также `docs/modules/UI_PACKAGES_INDEX.md`.
10. Прогнать локальный preflight: `cargo xtask validate-manifest`, `cargo xtask module validate <slug>`, `cargo xtask module test <slug>`.

Шаблон файлов и минимальных разделов живёт в [шаблоне документации модуля](../templates/module_contract.md).

## Рекомендуемый локальный preflight

Для path-модуля перед публикацией или серьёзной доработкой используйте:

```powershell
cargo xtask module validate blog
cargo xtask module test blog
```

Если меняется весь composition contract платформы, добавляйте:

```powershell
cargo xtask validate-manifest
```

## Связанные документы

- [Как писать модуль в RusToK](./module-authoring.md)
- [Реестр модулей и приложений](./registry.md)
- [Реестр crate-ов модульной платформы](./crates-registry.md)
- [Индекс локальной документации по модулям](./_index.md)
- [Шаблон документации модуля](../templates/module_contract.md)
- [Главный README по верификации](../verification/README.md)

> Статус документа: актуальный. При изменении правил `xtask`, acceptance-контракта для модулей или состава платформенных модулей обновляйте этот файл вместе с `docs/index.md`.
