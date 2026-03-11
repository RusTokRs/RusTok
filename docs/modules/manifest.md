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
  `docs/modules/module-rebuild-plan.md`.

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

### Module spec

| Поле | Тип | Обязательное | Описание |
| --- | --- | --- | --- |
| `crate` | string | да | Имя crate модуля. |
| `source` | string | да | `crates-io` \| `git` \| `path`. |
| `version` | string | нет | Версия для `crates-io`. |
| `git` | string | нет | Git URL. |
| `rev` | string | нет | Commit SHA/таг. |
| `path` | string | нет | Локальный путь (monorepo или vendor). |
| `features` | array | нет | Фичи для конкретного модуля. |

> Сами метаданные модуля (slug/name/description/version/deps) всё равно берутся
> из `RusToKModule` во время сборки и регистрации в `ModuleRegistry`.


## UI-контракты модулей в манифесте и сборке

Для `ModuleKind::Optional` модулей действует правило композиции UI через модульные пакеты:

- UI (экраны, меню, nav items, guards, редакторы) поставляется из `crates/rustok-<module>/ui/*`.
- Приложения (`apps/admin`, `apps/next-admin`, `apps/storefront`, `apps/next-frontend`) подключают эти пакеты через единый модульный контракт/registry, без хардкода optional-domain UI внутри приложений.

Рекомендуемая структура и entry points:

- `crates/rustok-<module>/ui/admin` → экспорт `adminNavItems` (или эквивалент контрактов для `registerAdminModule` / `AdminComponentRegistration`).
- `crates/rustok-<module>/ui/frontend` → экспорт `frontendNavItems` (или эквивалент контрактов для `registerStorefrontModule` / `StorefrontComponentRegistration`).

Референсный образец в текущем репозитории: UI-пакеты blog-модуля (`crates/rustok-blog/ui/admin`, `crates/rustok-blog/ui/frontend`).

Исключение:

- Core-модули `index`, `tenant`, `rbac`.
- Платформенные core crate'ы (`rustok-core`, `rustok-outbox`, `rustok-telemetry`) и инфраструктурные слои.

Эти компоненты могут оставаться на отдельном UI-подходе и не обязаны реализовывать `ui/admin`/`ui/frontend` пакеты.

Операционное требование для корректной сборки пакетов:

- host-приложения должны явно зависеть от модульных UI-пакетов (workspace/file dependency), а не от временных локальных импортов;
- отсутствие ожидаемого UI entry point для установленного optional-модуля считается несовместимой конфигурацией release и должно блокировать включение модуля по умолчанию до исправления контракта.

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

## Blueprint: API для admin rebuild

Ниже — минимальная схема API, которую можно подключить к админке, чтобы запускать
пересборки и показывать прогресс.

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
3. **Cargo build**: `cargo build -p rustok-server --release`.
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
