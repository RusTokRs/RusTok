# Module Manifest (WordPress/NodeBB-style rebuilds)

Этот документ описывает **манифест модулей** — файл, который фиксирует состав
модулей для сборки RusToK и позволяет **устанавливать/удалять модули через
пересборку** (в стиле WordPress/NodeBB: админка → rebuild → новый бинарник).

Манифест — единый источник правды о том, какие модули входят в конкретную
сборку. Изменение манифеста = новая сборка и деплой.

## Зачем нужен манифест

- **Динамичность состава модулей**: можно добавлять/удалять модули без
  долгосрочной “жесткой сборки”.
- **Сборка = набор модулей**: каждый артефакт соответствует списку модулей.
- **Админка как в NodeBB**: установка/удаление модуля инициирует rebuild.
- **План внедрения**: подробная дорожная карта находится в
  `docs/modules/module-system-plan.md`.

## Где используется

1. **Админка** изменяет манифест (install/uninstall).
2. **Build-service** читает манифест, собирает новый бинарник.
3. **Deploy** обновляет приложение.
4. **Registry** в рантайме уже содержит только модули из новой сборки.

## Формат манифеста (TOML)

Рекомендуемый формат — TOML, чтобы удобно использовать в CI и в Rust-скриптах.

```toml
schema = 2
app = "rustok-server"

[build]
target = "x86_64-unknown-linux-gnu"
profile = "release"

[build.server]
embed_admin = true           # Встроить Leptos admin в бинарник сервера
embed_storefront = true      # Встроить Leptos storefront в бинарник сервера

[build.admin]
stack = "leptos"             # "leptos" | "next"

[[build.storefront]]
id = "default"
stack = "leptos"             # "leptos" | "next"

[modules]
content = { crate = "rustok-content", source = "crates-io", version = "0.1" }
commerce = { crate = "rustok-commerce", source = "git", git = "ssh://git/commerce.git", rev = "abc123" }
blog = { crate = "rustok-blog", source = "path", path = "../modules/rustok-blog" }

[settings]
default_enabled = ["content", "commerce", "pages"]
```

### Поля

| Поле | Тип | Обязательное | Описание |
| --- | --- | --- | --- |
| `schema` | int | да | Версия формата манифеста (текущая: 2). |
| `app` | string | да | Целевое приложение/бинарник. |
| `build.target` | string | нет | Целевой triple сборки. |
| `build.profile` | string | нет | Профиль сборки (`release`/`debug`). |
| `build.server.embed_admin` | bool | нет | Встроить Leptos admin в сервер (default: false). |
| `build.server.embed_storefront` | bool | нет | Встроить Leptos storefront в сервер (default: false). |
| `build.admin.stack` | string | нет | UI-стек админки: `"leptos"` \| `"next"`. |
| `[[build.storefront]]` | array | нет | Список storefront'ов (мультисайт). |
| `build.storefront[].id` | string | да | Уникальный ID storefront'а. |
| `build.storefront[].stack` | string | да | UI-стек: `"leptos"` \| `"next"`. |
| `modules` | table | да | Карта `slug -> module spec`. |
| `settings.default_enabled` | array | нет | Какие модули включать по умолчанию после сборки. |

> `settings.default_enabled` в `modules.toml` — это **platform-level build/runtime setting**.
> Tenant-specific настройки модулей живут отдельно, в `tenant_modules.settings`,
> и обновляются через GraphQL mutation `updateModuleSettings(moduleSlug, settings)`.

### Module spec

| Поле | Тип | Обязательное | Описание |
| --- | --- | --- | --- |
| `crate` | string | да | Имя crate модуля. |
| `source` | string | да | `crates-io` \| `git` \| `path`. |
| `version` | string | нет | Версия для `crates-io`. |
| `git` | string | нет | Git URL. |
| `rev` | string | нет | Commit SHA/таг. |
| `path` | string | нет | Локальный путь (monorepo или vendor). |
| `depends_on` | array | нет | Грубый dependency graph для сборки и toggle flow. |
| `dependency_version_reqs` | table | нет | Semver-требования к зависимостям (`slug -> version_req`). Может приезжать из `rustok-module.toml`. |
| `conflicts_with` | array | нет | Явные конфликтующие модули. Может приезжать из `rustok-module.toml`. |
| `entry_type` | string | нет | Нормализованный Rust path к типу модуля для `ModuleRegistry` (`rustok_blog::BlogModule`). Нужен прежде всего для external/non-path crate-ов. |
| `graphql_query_type` | string | нет | Нормализованный Rust path к GraphQL query type модуля. |
| `graphql_mutation_type` | string | нет | Нормализованный Rust path к GraphQL mutation type модуля. |
| `http_routes_fn` | string | нет | Нормализованный Rust path к HTTP route factory функции модуля. |
| `http_webhook_routes_fn` | string | нет | Нормализованный Rust path к optional webhook route factory функции модуля. |
| `features` | array | нет | Фичи для конкретного модуля. |

### `[settings]` в `rustok-module.toml`

Tenant-specific settings schema живёт в `rustok-module.toml` самого модуля и
сейчас валидируется на сервере в `ManifestManager::validate_module_settings()`.
`updateModuleSettings(moduleSlug, settings)` принимает JSON object, отбрасывает
неизвестные ключи как ошибку, проверяет типы/диапазоны и автоматически
дополняет отсутствующие поля значениями `default`.

Минимальный формат секции:

```toml
[settings]
postsPerPage = { type = "integer", default = 20, min = 1, max = 100 }
showAuthor = { type = "boolean", default = true }
heroTitle = { type = "string", min = 3, max = 80 }
```

Более богатый nested-контракт тоже поддерживается:

```toml
[settings]
seo = { type = "object", properties = {
  metaTitle = { type = "string", max = 80, description = "Tenant-specific SEO title override." },
  indexable = { type = "boolean", default = true }
} }

contentBlocks = { type = "array", items = {
  type = "object",
  properties = {
    kind = { type = "string", options = ["hero", "grid", "faq"] },
    enabled = { type = "boolean", default = true }
  }
} }
```

Поддерживаемые `type`:

- `string`
- `integer`
- `number`
- `boolean`
- `object`
- `array`
- `json` / `any`

`min` / `max` применяются к числам, а для `string` и `array` трактуются как
ограничения длины. `/modules` в Leptos Admin уже умеет рендерить generated
form для всей текущей schema: scalar-поля (`string` / `integer` / `number` /
`boolean`) идут typed controls, `object` / `array` получают top-level
structured editor и deep nested editor с create-actions, key rename и array
reorder на каждом object/array уровне, а `json` / `any` редактируются как
per-field JSON editors с inline summary и helper-actions (`Format JSON`,
`Reset`, `Add property/item`). Raw JSON editor остаётся только
fallback-режимом для модулей без `[settings]`, а сервер в любом случае
сохраняет единую валидацию и normalizing defaults.

> Нормативный путь для optional-модулей: identity, semver-зависимости и конфликты
> живут в `rustok-module.toml` самого модуля. `apps/server` не должен содержать
> модульно-специфичные правила — он только читает и валидирует этот generic-контракт.

> Текущий composition-root contract для `apps/server`: `apps/server/build.rs` читает `modules.toml`
> и генерирует optional module registry, GraphQL schema fragments и HTTP routes в `OUT_DIR`.
> Явный server entry-point contract теперь поддерживается в двух формах:
> path-модули объявляют его в `rustok-module.toml` через `[crate]`, `[provides.graphql]` и `[provides.http]`,
> а external/non-path модули могут хранить уже нормализованные Rust paths прямо в `modules.toml`
> (`entry_type`, `graphql_query_type`, `graphql_mutation_type`, `http_routes_fn`, `http_webhook_routes_fn`).
> Naming conventions (`<PascalSlug>Module`, `<PascalSlug>Query`, `<PascalSlug>Mutation`, `controllers::routes`,
> optional `webhook_routes`) остаются только как backward-compatible fallback для path-модулей без явного контракта.
>
> Для Leptos-host приложений foundation тоже уже поднят частично:
> `apps/admin/build.rs` генерирует dashboard/nav/page registry wiring из `[provides.admin_ui].leptos_crate`
> и соглашения `<PascalSlug>Admin`, а `apps/admin` монтирует module root pages через generic routes
> `/modules/:module_slug` и `/modules/:module_slug/*module_path`, прокидывая module-agnostic `UiRouteContext`
> и manifest-driven secondary nav из optional `[[provides.admin_ui.pages]]`. `apps/storefront/build.rs` генерирует slot/page wiring
> из `[provides.storefront_ui].leptos_crate`, optional `slot`, `route_segment`, `page_title`
> и соглашения `<PascalSlug>View`. `apps/storefront` рендерит эти async surface-ы через streaming SSR
> и прокидывает module-agnostic `UiRouteContext` (locale, route segment, subpath, query params), а модули `pages`,
> `blog`, `commerce`, `forum` и `workflow/templates` теперь служат рабочими exemplar-ами для module-owned Leptos UI.


## UI-контракты модулей в манифесте и сборке

- **Leptos UI** поставляется из суб-крейтов модуля: `crates/rustok-<module>/admin/` и `crates/rustok-<module>/storefront/`.
- **Next.js UI** поставляется из npm-пакетов приложений: `apps/next-admin/packages/<module>/` и `apps/next-frontend/packages/<module>/`.

Тот же package-based ownership contract применяется и к capability-слоям вне runtime module registry.
Текущий пример: `rustok-ai` поставляет Leptos UI из `crates/rustok-ai/admin/`, а Next.js UI из
`apps/next-admin/packages/rustok-ai/`, при этом host-приложения остаются только composition root.

Рантайм-контракты и entry-points:

- `leptos_crate` (в `rustok-module.toml`) — указывает имя Rust-субкрейта для админки или витрины.
- `route_segment` (optional, только для `[provides.admin_ui]`) — относительный segment для host route `/modules/:module_slug`; по умолчанию равен `module.slug`.
- `nav_label` (optional, только для `[provides.admin_ui]`) — подпись generated nav item; по умолчанию равна `module.name`.
- `[[provides.admin_ui.pages]]` (optional) — декларативный список nested admin subpages для generic host secondary nav.
- `provides.admin_ui.pages[].subpath` — относительный subpath внутри `/modules/:module_slug/*module_path`, который пакет сам обрабатывает через `UiRouteContext`.
- `provides.admin_ui.pages[].title` / `nav_label` — optional metadata для secondary nav и заголовков host-обвязки.
- `slot` (optional, только для `[provides.storefront_ui]`) — host storefront slot (`home_after_hero`, `home_after_catalog`, `home_before_footer`); по умолчанию `home_after_hero`.
- `route_segment` (optional, для `[provides.storefront_ui]`) — segment для generic storefront route `/modules/{route_segment}`; по умолчанию равен `module.slug`.
- `page_title` (optional, только для `[provides.storefront_ui]`) — заголовок generic storefront module page; по умолчанию равен `module.name`.
- `next_package` (в `rustok-module.toml`) — указывает имя npm-пакета.
- `[provides.*_ui.i18n]` (optional) — manifest-level контракт для native i18n bundle-путей конкретного surface.
- `provides.*_ui.i18n.default_locale` — канонический default locale surface-пакета; должен входить в `supported_locales`.
- `provides.*_ui.i18n.supported_locales` — список locale tags (`en`, `ru`, `pt-BR`), для которых модуль реально поставляет bundle files.
- `provides.*_ui.i18n.leptos_locales_path` — относительный к модулю путь до Leptos bundle-директории вида `locales/*.json`.
- `provides.*_ui.i18n.next_messages_path` — относительный к модулю путь до Next bundle-директории вида `messages/*.json`; путь может выходить в host package workspace, но должен резолвиться внутри корня репозитория.

Важно:

- `[provides.*_ui.i18n]` описывает только расположение package-owned translation bundles и их декларативный shape.
- Этот блок **не** разрешает модулю вводить собственную locale negotiation policy, отдельные query/header/cookie fallback-цепочки или свою трактовку effective locale.
- Канонический выбор locale остаётся за host/runtime contract: server выбирает effective locale, host прокидывает его в UI surface, а пакет только потребляет уже выбранный язык.

Если `[provides.*_ui.i18n]` объявлен, `ManifestManager` валидирует:

- что `supported_locales` не пустой;
- что все locale tags валидны;
- что `default_locale` входит в `supported_locales`;
- что `leptos_locales_path` не объявляется без `leptos_crate`;
- что `next_messages_path` не объявляется без `next_package`;
- что каждая объявленная bundle-директория существует и содержит `<locale>.json` для всех `supported_locales`.

Пример server entry-point contract в `rustok-module.toml`:

```toml
[crate]
entry_type = "BlogModule"

[provides.graphql]
query = "graphql::BlogQuery"
mutation = "graphql::BlogMutation"

[provides.http]
routes = "controllers::routes"

[provides.storefront_ui]
leptos_crate = "rustok-blog-storefront"
slot = "home_after_catalog"
route_segment = "blog"
page_title = "Blog"

[provides.storefront_ui.i18n]
default_locale = "en"
supported_locales = ["en", "ru"]
leptos_locales_path = "storefront/locales"
next_messages_path = "../../apps/next-frontend/packages/blog/messages"
```

Для path-модулей эти значения считаются относительными к crate и нормализуются сервером в полные Rust paths
вроде `rustok_blog::graphql::BlogQuery`. Для external/non-path crate-ов тот же контракт должен быть уже
сохранён в `modules.toml` полями `entry_type`, `graphql_query_type`, `graphql_mutation_type`,
`http_routes_fn`, `http_webhook_routes_fn`.

Leptos-host приложения (`apps/admin`, `apps/storefront`) подключают модульные пакеты через свои `build.rs`, а `BuildExecutor` затем собирает реальные host-артефакты по manifest-derived plan. Приложения Next.js требуют ручного добавления зависимостей в `package.json` и ручной пересборки.

Референсные образцы для Leptos: модуль `workflow` (`crates/rustok-workflow/admin/` как root-page package поверх legacy detail flow), модуль `pages` (`crates/rustok-pages/admin/` и `crates/rustok-pages/storefront/`) как рабочий end-to-end exemplar для page-driven surfaces, модуль `blog` (`crates/rustok-blog/admin/` и `crates/rustok-blog/storefront/`) как рабочий exemplar для обычного content CRUD/read-path, модуль `commerce` (`crates/rustok-commerce/admin/` и `crates/rustok-commerce/storefront/`) как exemplar для catalog CRUD/public catalog read-path поверх собственного GraphQL contract и модуль `forum` (`crates/rustok-forum/admin/` и `crates/rustok-forum/storefront/`) как exemplar для NodeBB-inspired admin/storefront surfaces поверх собственного GraphQL/REST contract.

Исключение:

- Core-модули платформы (`auth`, `cache`, `email`, `index`, `outbox`, `tenant`, `rbac`) могут не поставлять собственные UI-пакеты, если их роль ограничена server/runtime infrastructure.
- Shared library и support crates (`rustok-core`, `rustok-events`, `rustok-telemetry` и другие инфраструктурные слои) не входят в taxonomy `Core` / `Optional` platform modules и не обязаны реализовывать `ui/admin` / `ui/frontend` пакеты.

Операционное требование для корректной сборки пакетов:

- host-приложения должны явно зависеть от модульных UI-пакетов (workspace/file dependency), а не от временных локальных импортов;
- отсутствие ожидаемого UI entry point для установленного optional-модуля считается несовместимой конфигурацией release и должно блокировать включение модуля по умолчанию до исправления контракта.
- если модуль заявлен как dual-stack (Next + Leptos), отсутствие entry point хотя бы для одного runtime также считается несовместимой конфигурацией release.

## Deployment profiles (composable layers)

Подробное описание — в ADR [`2026-03-07-deployment-profiles-and-ui-stack.md`](../../DECISIONS/2026-03-07-deployment-profiles-and-ui-stack.md).

Профиль вычисляется из `embed_admin` + `embed_storefront`:

| `embed_admin` | `embed_storefront` | Profile | Описание |
|---|---|---|---|
| true | true | **Monolith** | 1 бинарник: Axum + Leptos admin + storefront (как WordPress) |
| true | false | **ServerWithAdmin** | Axum + Leptos admin; storefront(s) отдельно |
| false | true | **ServerWithStorefront** | Axum + Leptos storefront; admin отдельно |
| false | false | **HeadlessApi** | Чистый API; admin и storefront(s) — отдельные процессы |

### Мультисайт

`[[build.storefront]]` — массив. Можно иметь несколько storefront'ов
с разными стеками и в разных регионах:

```toml
[[build.storefront]]
id = "site-eu"
stack = "next"

[[build.storefront]]
id = "site-us"
stack = "next"
```

## Жизненный цикл install/uninstall

### Установка
1. Админка добавляет модуль в манифест.
2. Build-service запускает сборку.
3. Деплой выкатывает новый бинарник.
4. Registry содержит новый модуль, а `tenant_modules` управляет его включением.

### Удаление
1. Админка удаляет модуль из манифеста.
2. Build-service пересобирает приложение без модуля.
3. Новый бинарник больше не содержит код модуля.

## Админка в стиле NodeBB

UI шаги:
1. Выбрать модуль из каталога (или указать URL/путь).
2. Нажать **Install / Uninstall**.
3. Админка показывает статус сборки (очередь → build → deploy).
4. После деплоя модуль доступен для включения на уровне tenant.

## Минимальные гарантии

- **Консистентность**: если сборка прошла, модуль гарантированно присутствует.
- **Безопасность**: нет runtime-подгрузки нативного кода.
- **Воспроизводимость**: манифест фиксирует точный состав и версии.
- **Namespace safety**: GraphQL-visible типы модулей обязаны иметь уникальные имена в общей schema; module-specific enum/object names не должны конфликтовать с типами других модулей.

## Blueprint: API для admin rebuild

Ниже — минимальная схема API, которую можно подключить к админке, чтобы запускать
пересборки и показывать прогресс.

## Как делать новый модуль по текущему механизму

Нормативный путь для нового path-модуля теперь такой:

1. Создать основной crate модуля в `crates/rustok-<slug>/`.
2. Добавить `rustok-module.toml` как канонический manifest модуля.
3. Явно описать server entry points через `[crate]`, `[provides.graphql]`, `[provides.http]`, если модуль публикует runtime surface.
4. Если модуль даёт Leptos admin UI или storefront UI, создать соответствующий sub-crate и одновременно объявить его в manifest через `[provides.admin_ui]` и/или `[provides.storefront_ui]`.
5. Если UI-пакет поставляет собственные переводные bundle-файлы, объявить их через `[provides.*_ui.i18n]`, чтобы locale bundles валидировались вместе с surface wiring.
6. Если модуль даёт tenant settings, описать их в `[settings]`, а не в host-коде.
7. Если модуль должен быть виден в marketplace/registry, заполнить `[marketplace]`.
8. Добавить path-entry модуля в `modules.toml`.
9. Проверить модуль через `cargo xtask module validate <slug>` и `cargo xtask module test <slug>`.

### Минимальный checklist

- У модуля есть `Cargo.toml`.
- У модуля есть `README.md`.
- У path-модуля есть `rustok-module.toml`.
- `module.slug`, `module.version`, `crate.name` и версии UI sub-crate-ов согласованы.
- `package.license` заполнен.
- Если модуль поставляет package-owned переводы, `[provides.*_ui.i18n]` объявлен и реально указывает на существующие bundle files.
- `description`, `publisher`, `category`, `tags` и визуальная metadata заполнены настолько, чтобы модуль не выглядел как workspace-only заготовка.

### Минимальный пример `rustok-module.toml`

```toml
[module]
slug = "blog"
name = "Blog"
version = "0.1.0"

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

[[provides.admin_ui.pages]]
subpath = "posts"
title = "Posts"
nav_label = "Posts"

[provides.storefront_ui]
leptos_crate = "rustok-blog-storefront"
route_segment = "blog"
page_title = "Blog"
slot = "home_after_catalog"

[provides.storefront_ui.i18n]
default_locale = "en"
supported_locales = ["en", "ru"]
leptos_locales_path = "storefront/locales"

[marketplace]
category = "content"
tags = ["blog", "editorial"]
publisher = "rustok"
description = "Blog module with admin and storefront surfaces."
icon = "https://modules.rustok.dev/assets/blog/icon.svg"

[settings]
postsPerPage = { type = "integer", default = 20, min = 1, max = 100, options = [10, 20, 50] }
```

### Инварианты для UI sub-crate-ов

- Наличие `admin/Cargo.toml` без `[provides.admin_ui].leptos_crate` — это ошибка wiring.
- Наличие `storefront/Cargo.toml` без `[provides.storefront_ui].leptos_crate` — это ошибка wiring.
- Объявление `[provides.admin_ui]` без реального `admin/Cargo.toml` — это тоже ошибка.
- Объявление `[provides.storefront_ui]` без реального `storefront/Cargo.toml` — это тоже ошибка.
- Если UI surface заявляет `[provides.*_ui.i18n]`, manifest обязан ссылаться только на реально существующие locale bundles, а `default_locale` обязан входить в `supported_locales`.
- Доказательство интеграции UI — manifest wiring, а не просто наличие подпапки или crate на диске.

### Пошаговый flow для нового publishable модуля

1. Создать crate и `README.md`.
2. Заполнить `rustok-module.toml` минимумом из `[module]`, `[crate]`, runtime/UI provides, `[settings]` и `[marketplace]` по необходимости.
3. Добавить модуль в `modules.toml` как path-entry.
4. Если есть `admin/` или `storefront/`, сразу синхронно объявить их в `[provides.admin_ui]` / `[provides.storefront_ui]`.
5. Если UI-package поставляет свои переводы, объявить `[provides.*_ui.i18n]` до первого `xtask validate`, а не после.
6. Прогнать локальный preflight:
   - `cargo xtask module validate <slug>`
   - `cargo xtask module test <slug>`
   - `cargo xtask module publish <slug> --dry-run`
7. После этого только переходить к live V2 flow или к operator UI `/modules`.

### Чего не делать

- Не считать наличие `admin/` или `storefront/` доказательством интеграции: без manifest wiring это ошибка, а не partial-ready состояние.
- Не прятать module settings, route wiring или i18n bundle paths в host-коде, если их можно выразить в manifest.
- Не обходить `xtask validate` и `xtask test` перед `publish --dry-run` и live publish.

### Как проходит publish/governance

Локальный preflight:

```powershell
cargo xtask module validate blog
cargo xtask module test blog
cargo xtask module publish blog --dry-run
cargo xtask module publish blog --registry-url http://127.0.0.1:5150
# Только если governance review должен завершиться этим же CLI-вызовом:
cargo xtask module publish blog --registry-url http://127.0.0.1:5150 --auto-approve
```

Live V2 flow идёт поверх registry:

1. `POST /v2/catalog/publish` создаёт publish request.
2. `PUT /v2/catalog/publish/{request_id}/artifact` загружает artifact.
3. `POST /v2/catalog/publish/{request_id}/validate` запускает validation.
4. После успешной automated validation request попадает в `approved` как `review-ready`, но это ещё не публикация release.
5. `POST /v2/catalog/publish/{request_id}/approve` или `POST /v2/catalog/publish/{request_id}/reject` завершает governance review.
6. `POST /v2/catalog/yank` отзывает уже опубликованный релиз.

По умолчанию live `cargo xtask module publish <slug> --registry-url ...` теперь останавливается на статусе `approved` и возвращает `review-ready` request для отдельного review-step. Автоматический финальный `approve` выполняется только при явном `--auto-approve`.

В операторском UI `/modules` этот же flow теперь уже виден и запускается интерактивно, но канонический контракт всё равно остаётся за `rustok-module.toml`, `modules.toml`, `xtask` и `/v2/catalog/*`.

### Endpoint: создать сборку

`POST /admin/builds`

```json
{
  "manifest_ref": "main",
  "requested_by": "admin@rustok",
  "reason": "install module: pages",
  "modules": {
    "content": { "source": "crates-io", "crate": "rustok-content", "version": "0.1" },
    "forum": { "source": "git", "crate": "rustok-forum", "git": "ssh://git/forum.git", "rev": "abc123" },
    "pages": { "source": "path", "crate": "rustok-pages", "path": "../modules/rustok-pages" }
  }
}
```

**Ответ:**
```json
{ "build_id": "bld_01H...", "status": "queued" }
```

### Endpoint: статус сборки

`GET /admin/builds/{build_id}`

```json
{
  "build_id": "bld_01H...",
  "status": "running",
  "stage": "build",
  "progress": 62,
  "logs_url": "https://builds/rustok/bld_01H.../logs"
}
```

### Endpoint: деплой/активация

`POST /admin/builds/{build_id}/deploy`

```json
{ "environment": "prod" }
```

### Endpoint: rollback

`POST /admin/builds/{build_id}/rollback`

```json
{ "target_release": "rel_2025_01_10_001" }
```

## Blueprint: build pipeline (Docker/K8s)

Пример пайплайна:

1. **Checkout + deps**: забрать repo + загрузить зависимости.
2. **Render manifest**: зафиксировать `modules.toml` в workspace.
3. **Build plan**: команда выводится из `modules.toml`:
   `cargo build -p rustok-server --release --target <build.target> --features <derived-from-build.server>`.
   Для текущих server surfaces это `embed-admin` и `embed-storefront`.
   Если manifest требует Leptos admin, в тот же execution plan добавляется `trunk build` для `apps/admin`.
   Если manifest требует Leptos storefront, в тот же execution plan добавляется `cargo build -p rustok-storefront`.
   Текущий operator path: `cargo loco task --name rebuild` или `target/debug/rustok-server.exe task rebuild`.
   Можно указать `build_id=<uuid>` для конкретной записи или `dry_run=true`, чтобы только распечатать derived command без запуска.
   Для runtime automation можно включить `settings.rustok.build.enabled=true`; тогда server поднимет background worker,
   который будет забирать queued builds и выполнять тот же manifest-derived plan.
   Дополнительно доступны `auto_release_environment` и `auto_activate_release` для локального release/deploy flow.
   В `settings.rustok.build.deployment` можно выбрать backend:
   `record_only` (только release record), `filesystem` (копирование server/admin/storefront artifacts в release bundle directory), `http` (multipart publish в удалённый deployment endpoint) или `container` (локальный `docker build`/`docker push` поверх уже собранного release bundle).
   Для filesystem backend используются `filesystem_root_dir` и опциональный `public_base_url`; для HTTP backend используются `endpoint_url` и опциональный `bearer_token`; для container backend используются `docker_bin` (по умолчанию `docker`), `image_repository` и опциональный `rollout_command`.
   Filesystem и container backend публикуют реальные `server_artifact_url`, `admin_artifact_url`, `storefront_artifact_url`; для `admin` это entrypoint `index.html` из `apps/admin/dist`, для `storefront` — отдельный SSR binary `rustok-storefront`.
   Container backend переиспользует текущий release bundle, упаковывает бинарник + `apps/server/migration` + `apps/server/config` в runtime image, публикует image в registry и заполняет `releases.container_image`.
   HTTP endpoint может дополнительно вернуть `deployment_status` (`accepted|deploying|deployed|failed`) и готовые frontend artifact URLs; сервер синхронизирует локальный release state с фактическим outcome вместо преждевременной auto-activation.
   Для container backend `rollout_command` остаётся generic hook без знания о конкретном orchestrator: сервер подставляет placeholders `{image}`, `{release_id}`, `{environment}`, `{bundle_dir}` и также прокидывает env vars `RUSTOK_RELEASE_ID`, `RUSTOK_RELEASE_ENVIRONMENT`, `RUSTOK_CONTAINER_IMAGE`, `RUSTOK_RELEASE_BUNDLE_DIR`.
   Отдельная smoke-проверка profile matrix теперь живёт в `scripts/verify/verify-deployment-profiles.sh`.
4. **Docker image**: собрать образ с готовым бинарником.
5. **Push**: загрузить в registry.
6. **Deploy**: обновить deployment (K8s) или контейнер (docker-compose).
7. **Smoke**: проверить `/health` и `/health/modules`.

## Blueprint: rollback

1. **Хранить релизы**: у каждого деплоя есть `release_id` и образ.
2. **Откат**: переключить deployment на предыдущий `release_id`.
3. **Проверка**: повторить smoke-check.
4. **Фиксация**: записать rollback в журнал событий админки.

## Blueprint: что сохраняет админка

- `build_id`, `release_id`, `status`, `started_at`, `finished_at`
- `manifest_hash`, `modules_delta`, `requested_by`, `reason`

> **Статус документа:** Актуальный. Формат манифеста (`modules.toml`) и жизненный цикл rebuild могут уточняться — фиксируйте изменения здесь и в `docs/index.md`.
