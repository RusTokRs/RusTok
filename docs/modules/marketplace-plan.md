# RusTok Module Marketplace — Архитектура и план

> Статус: RFC
> Дата: 2026-03-03
>
> Связанные документы:
> - `docs/concepts/plan-oauth2-app-connections.md` — OAuth2 AS, аутентификация клиентов
> - `docs/modules/module-rebuild-plan.md` — tenant-level toggle, build pipeline
>
> Execution split:
> - Module detail state on `/modules` is now deep-linkable in both admin apps: the selected catalog entry is mirrored into the `module` query param, so direct links reopen the same detail panel instead of relying on purely local UI state.
> - The shared backend and both admin apps now support source-aware catalog filtering: `marketplace(...)` can be narrowed by source and install-state in addition to trust and compatibility, and `/modules` exposes those controls without diverging the two admin surfaces.
> - Both admin apps now expose a dedicated `marketplaceModule(slug)` detail panel on `/modules`, so operators can review package metadata, admin-surface policy, compatibility, provenance, and version history before install or upgrade actions.
> - Both admin apps now have a shared catalog filtering baseline on `/modules`: Marketplace and Updates can be narrowed by search, category, trust level, and compatibility-only mode without diverging the two admin surfaces.
> - Этот документ описывает целевую marketplace/registry архитектуру и долгоживущие контракты.
> - Фактический статус внедрения в монорепе ведётся в `docs/modules/module-rebuild-plan.md`.
> - Текущий implementation cut в репозитории использует внутренний catalog contract платформы, собранный из `modules.toml` + `ModuleRegistry`; отдельный registry service, публикация и verification pipeline остаются следующими этапами этого RFC.
> - Catalog contract уже расширяется metadata-полями ownership/trust/admin-surface policy, чтобы first-party, third-party и showcase-only UI surfaces были видимы до появления внешнего registry.
> - Текущий implementation cut уже перенес эти поля в first-party `rustok-module.toml` для встроенных path-модулей; синхронизация с registry и publish-flow для third-party модулей остаются следующими фазами RFC.
> - Backend catalog loading now already goes through a provider abstraction with `local-manifest` as the first concrete provider, so adding `registry`, `git`, or private sources does not require очередного перелома внутреннего platform contract.
> - Legacy/current API сохранять не требуется: текущие GraphQL/catalog/build contracts считаются внутренними и свободно эволюционируют как часть новой модульной системы платформы.
> - Следующий execution step для консистентности с этим RFC: поднять внутренний `registry` provider skeleton в provider chain платформы, даже если внешний registry service еще не существует.
>
> **Важно**: Архитектура аутентификации для маркетплейса описана в Приложении A
> OAuth-плана (`plan-oauth2-app-connections.md`). Три auth-потока:
> - Admin UI → Server (OAuth PKCE прокси) → Marketplace (Platform API Key)
> - CLI автора → Marketplace (свой auth или federated)
> - Build Worker → Marketplace (Platform API Key для скачивания .crate)

---

## 1. Что такое маркетплейс модулей

### Определение

Маркетплейс модулей RusTok — это **каталог + реестр + pipeline верификации**,
который позволяет:

- **Операторам платформы** — находить, устанавливать и обновлять модули через
  админку, как плагины в WordPress.
- **Разработчикам модулей** — публиковать свои модули по единому стандарту,
  делая их доступными всем операторам RusTok.

### Чем это НЕ является

- Это **не app store** с оплатой (на первом этапе). Монетизация — отдельная тема.
- Это **не runtime-загрузчик**. Модули — Rust crate'ы, компилируются в бинарник.
- Это **не fork crates.io**. Это специализированный реестр для RusTok-модулей
  с проверкой совместимости и контракта `RusToKModule`.

### Аналоги в других экосистемах

| Платформа | Маркетплейс | Формат модуля | Установка |
|---|---|---|---|
| **WordPress** | wordpress.org/plugins | PHP-файлы + readme.txt | Скачать zip, распаковать |
| **Shopify** | Shopify App Store | SPA + OAuth + API | Установить через iframe |
| **Strapi** | Strapi Market | npm-пакет + strapi-plugin.json | `npm install` |
| **Payload CMS** | — | npm-пакет | `npm install` + config |
| **Rust (crates.io)** | crates.io | Cargo.toml | `cargo add` |
| **RusTok** | modules.rustok.dev | Rust crate + rustok-module.toml | Install → rebuild → deploy |

### Ключевое отличие RusTok

В WordPress/Strapi модуль — это интерпретируемый код (PHP/JS), который
подгружается в runtime. В RusTok модуль — **компилируемый Rust crate**,
который линкуется в бинарник. Это даёт:

- **Безопасность**: compile-time проверки, нет eval/require.
- **Производительность**: нативный код, zero-cost abstractions.
- **Надёжность**: если компилируется — работает (trait контракт).
- **Компромисс**: установка = пересборка (2-5 мин вместо мгновенно).

---

## 2. Откуда берутся модули

### Три источника модулей

```
┌─────────────────────────────────────────────────────────┐
│                  Module Sources                          │
│                                                          │
│  ┌──────────┐   ┌──────────────┐   ┌─────────────────┐ │
│  │  Built-in │   │  Marketplace │   │  Private / Git  │ │
│  │  (path)   │   │  (registry)  │   │  (git / path)   │ │
│  └─────┬────┘   └──────┬───────┘   └───────┬─────────┘ │
│        │               │                    │            │
│        ▼               ▼                    ▼            │
│  ┌──────────────────────────────────────────────────┐   │
│  │              modules.toml                         │   │
│  │  blog = { source = "path", path = "crates/..." } │   │
│  │  seo  = { source = "registry", version = "1.0" } │   │
│  │  crm  = { source = "git", git = "git@..." }      │   │
│  └──────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

#### 2.1. Built-in (path) — Встроенные модули

Модули из монорепы RusTok. Поставляются с платформой:

```toml
content = { crate = "rustok-content", source = "path", path = "crates/rustok-content" }
blog = { crate = "rustok-blog", source = "path", path = "crates/rustok-blog", depends_on = ["content"] }
```

- Всегда доступны, не требуют скачивания.
- Версия совпадает с версией платформы.
- Разрабатываются core-командой RusTok.

#### 2.2. Marketplace (registry) — Маркетплейс

Модули из публичного реестра `modules.rustok.dev`:

```toml
seo-tools = { crate = "rustok-seo", source = "registry", version = "^1.2.0" }
analytics = { crate = "rustok-analytics", source = "registry", version = "~2.0" }
```

- Проверены validation pipeline.
- Версионируются по semver.
- Metadata (иконки, описание, скриншоты) хранятся в реестре.
- Могут быть от core-команды или от сторонних разработчиков.

#### 2.3. Private / Git — Приватные модули

Кастомные модули организации, не опубликованные в маркетплейс:

```toml
# Из приватного Git-репозитория
internal-crm = { crate = "our-crm", source = "git", git = "git@github.com:our-org/crm-module.git", branch = "main" }

# Из локальной директории (dev/staging)
experiments = { crate = "experiments", source = "path", path = "../our-modules/experiments" }
```

- Не проходят публичную верификацию.
- Ответственность на операторе.
- Используются для внутренних/закрытых модулей.

---

## 3. Единый стандарт модуля

### 3.1. Манифест `rustok-module.toml`

Каждый модуль (встроенный, маркетплейс или приватный) **должен** содержать
манифест `rustok-module.toml` в корне crate. Это единый контракт:

```toml
# ──────────────────────────────────────────────────
# Идентификация
# ──────────────────────────────────────────────────
[module]
slug = "blog"                            # Уникальный ID (a-z, 0-9, -)
name = "Blog"                            # Отображаемое имя
version = "1.2.0"                        # semver
description = "Blogging engine with categories, tags, and SEO"
long_description = """
Full-featured blogging module for RusTok. Includes:
- Post editor with Markdown/WYSIWYG
- Categories and tags taxonomy
- SEO meta-tags per post
- RSS feed generation
- Scheduled publishing
"""

authors = ["RusTok Team <team@rustok.dev>"]
license = "MIT"
repository = "https://github.com/RustokCMS/rustok-blog"
homepage = "https://rustok.dev/modules/blog"
documentation = "https://docs.rustok.dev/modules/blog"

# ──────────────────────────────────────────────────
# Каталогизация (для маркетплейса)
# ──────────────────────────────────────────────────
[marketplace]
icon = "assets/icon.svg"                 # SVG иконка 64x64
banner = "assets/banner.png"             # Баннер 1200x400 (опционально)
screenshots = [
    "assets/screenshot-editor.png",
    "assets/screenshot-list.png",
]
category = "content"                     # Одна из фиксированных категорий
tags = ["blog", "cms", "seo", "markdown", "publishing"]
featured = false                         # Рекомендованный (устанавливается командой RusTok)

# ──────────────────────────────────────────────────
# Совместимость
# ──────────────────────────────────────────────────
[compatibility]
rustok_min = "0.5.0"                     # Минимальная версия платформы
rustok_max = "1.x"                       # Максимальная (wildcard)
rust_edition = "2024"                    # Rust edition

# ──────────────────────────────────────────────────
# Зависимости от других RusTok-модулей
# ──────────────────────────────────────────────────
[dependencies]
content = ">= 1.0.0"                    # Требует модуль content

# Конфликты (нельзя использовать вместе)
[conflicts]
legacy-blog = "*"                        # Несовместим с legacy-blog

# ──────────────────────────────────────────────────
# Rust crate
# ──────────────────────────────────────────────────
[crate]
name = "rustok-blog"                     # Имя crate
entry_type = "BlogModule"               # Тип, реализующий RusToKModule
# source определяется в modules.toml, не здесь

# ──────────────────────────────────────────────────
# Что модуль предоставляет платформе
# ──────────────────────────────────────────────────
[provides]
migrations = true                        # Есть миграции БД

permissions = [
    "blog:read",
    "blog:write",
    "blog:delete",
    "blog:publish",
]

events_emitted = [                       # Какие события генерирует
    "blog.post.created",
    "blog.post.published",
    "blog.post.deleted",
]

events_consumed = [                      # На какие события подписывается
    "content.item.updated",
]

# Расширения навигации в админке
[[provides.admin_nav]]
label_key = "blog.nav.posts"
href = "/posts"
icon = "pencil"
section = "content"                      # В какую секцию сайдбара

[[provides.admin_nav]]
label_key = "blog.nav.categories"
href = "/categories"
icon = "folder"
section = "content"

# Слоты в storefront
[[provides.storefront_slots]]
id = "blog-latest-posts"
slot = "HomeAfterHero"
order = 20

[[provides.storefront_slots]]
id = "blog-sidebar-widget"
slot = "BlogSidebar"
order = 10

# GraphQL расширения
[provides.graphql]
types = ["Post", "Category", "Tag"]
queries = ["posts", "post", "categories"]
mutations = ["createPost", "updatePost", "deletePost", "publishPost"]

# ──────────────────────────────────────────────────
# Настройки модуля (генерирует UI в админке)
# ──────────────────────────────────────────────────
[settings.posts_per_page]
type = "integer"
default = 10
min = 1
max = 100
label_key = "blog.settings.posts_per_page"
description_key = "blog.settings.posts_per_page.desc"

[settings.enable_comments]
type = "boolean"
default = true
label_key = "blog.settings.enable_comments"

[settings.default_status]
type = "enum"
values = ["draft", "published", "review"]
default = "draft"
label_key = "blog.settings.default_status"

[settings.excerpt_length]
type = "integer"
default = 200
min = 50
max = 1000
label_key = "blog.settings.excerpt_length"

# ──────────────────────────────────────────────────
# Локализация
# ──────────────────────────────────────────────────
[locales]
supported = ["en", "ru"]
default = "en"
# Файлы: locales/en.json, locales/ru.json в корне crate
```

### 3.2. Структура файлов модуля

```
rustok-blog/
├── rustok-module.toml          # ← Единый манифест (обязательно)
├── Cargo.toml                  # Rust crate metadata
├── src/
│   ├── lib.rs                  # pub struct BlogModule; impl RusToKModule
│   ├── entities/               # SeaORM entities (post, category, tag)
│   │   ├── mod.rs
│   │   ├── post.rs
│   │   └── category.rs
│   ├── migration/              # SeaORM migrations
│   │   ├── mod.rs
│   │   ├── m20250101_create_posts.rs
│   │   └── m20250201_add_tags.rs
│   ├── graphql/                # async-graphql types
│   │   ├── mod.rs
│   │   ├── types.rs            # Post, Category, Tag types
│   │   ├── queries.rs          # posts, post, categories
│   │   └── mutations.rs        # createPost, updatePost, etc.
│   ├── services/               # Бизнес-логика
│   │   ├── mod.rs
│   │   └── post_service.rs
│   └── events/                 # Event handlers
│       ├── mod.rs
│       └── on_content_updated.rs
├── admin/                      # Leptos-компоненты для админки (опционально)
│   ├── features/
│   │   └── blog/
│   │       ├── components/
│   │       │   ├── post_editor.rs
│   │       │   └── post_list.rs
│   │       └── api.rs
│   └── pages/
│       ├── posts.rs
│       └── categories.rs
├── storefront/                 # Компоненты для витрины (опционально)
│   └── widgets/
│       ├── latest_posts.rs
│       └── sidebar_widget.rs
├── locales/                    # i18n (обязательно минимум en)
│   ├── en.json
│   └── ru.json
├── assets/                     # Для маркетплейса (при публикации)
│   ├── icon.svg
│   ├── banner.png
│   ├── screenshot-editor.png
│   └── screenshot-list.png
└── tests/
    ├── integration.rs
    └── migration.rs
```

### 3.3. Минимальный контракт (must have)

| Требование | Файл / Trait | Проверяется |
|---|---|---|
| `rustok-module.toml` | Корень crate | Валидатором при publish |
| `impl RusToKModule` | `src/lib.rs` | Компилятором |
| `impl MigrationSource` | `src/lib.rs` | Компилятором |
| `slug` уникальный | `rustok-module.toml` | Реестром маркетплейса |
| `version` semver | `rustok-module.toml` | Валидатором |
| `locales/en.json` | `locales/` | Валидатором |
| Компилируется с `rustok_min` | CI | Validation pipeline |

### 3.4. Расширенный контракт (nice to have)

| Компонент | Назначение |
|---|---|
| `admin/` | Leptos-компоненты для FSD слоёв админки |
| `storefront/` | Виджеты для slot-системы витрины |
| `assets/` | Иконка, баннер, скриншоты для каталога |
| `settings` секция | JSON Schema для генерации UI настроек |
| `events_emitted` | Документация генерируемых событий |
| Тесты | integration, migration тесты |

---

## 4. Архитектура маркетплейса

### 4.1. Компоненты системы

```
┌─────────────────────────────────────────────────────────────────────┐
│                      modules.rustok.dev                              │
│                                                                      │
│  ┌────────────────┐  ┌────────────────┐  ┌────────────────────────┐ │
│  │  Module Index   │  │ Crate Storage  │  │  Validation Pipeline   │ │
│  │                 │  │                │  │                        │ │
│  │  - metadata     │  │  - .crate      │  │  1. manifest check     │ │
│  │  - versions     │  │    archives    │  │  2. compile min/max    │ │
│  │  - stats        │  │  - checksums   │  │  3. trait contract     │ │
│  │  - reviews      │  │  - signatures  │  │  4. cargo-audit        │ │
│  │  - screenshots  │  │                │  │  5. migration test     │ │
│  └───────┬────────┘  └───────┬────────┘  │  6. metadata quality   │ │
│          │                    │           └───────────┬────────────┘ │
│          ▼                    ▼                       ▼               │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    GraphQL API                                │   │
│  │                                                               │   │
│  │  Queries:                                                     │   │
│  │    marketplace(search, category, compatible) → [Module]       │   │
│  │    marketplaceModule(slug) → Module                           │   │
│  │    checkCompatibility(slug, version) → Report                 │   │
│  │                                                               │   │
│  │  Mutations:                                                   │   │
│  │    publishModule(crate, manifest) → PublishResult              │   │
│  │    yankVersion(slug, version) → Result                         │   │
│  │                                                               │   │
│  └──────────────────────────────────────────────────────────────┘   │
└───────┬─────────────────────────────────────────────┬───────────────┘
        │ query                                       │ publish
        ▼                                             ▼
┌───────────────────┐                        ┌────────────────────┐
│  Admin UI         │                        │  Module Author     │
│  (Leptos / Next)  │                        │  (rustok CLI)      │
│                   │                        │                    │
│  Browse catalog   │                        │  rustok mod init   │
│  Check compat     │                        │  rustok mod test   │
│  Install module   │                        │  rustok mod publish│
│  View progress    │                        │                    │
└───────────────────┘                        └────────────────────┘
```

### 4.2. Module Index (Индекс метаданных)

Хранит метаданные о каждом модуле (не код, а информацию):

```
module_index/
├── blog/
│   ├── meta.json           # name, description, authors, license
│   ├── versions.json       # [{version, released_at, compatible_with, yanked}]
│   ├── icon.svg            # Cached icon
│   ├── screenshots/
│   └── stats.json          # downloads, ratings
├── seo-tools/
│   ├── meta.json
│   └── ...
```

**Хранение**: PostgreSQL + S3-совместимое хранилище для assets.

```sql
CREATE TABLE marketplace_modules (
    slug          VARCHAR(64) PRIMARY KEY,
    name          VARCHAR(128) NOT NULL,
    description   TEXT NOT NULL,
    long_description TEXT,
    authors       JSONB NOT NULL,
    license       VARCHAR(32) NOT NULL,
    repository    VARCHAR(512),
    homepage      VARCHAR(512),
    category      VARCHAR(32) NOT NULL,
    tags          JSONB NOT NULL DEFAULT '[]',
    icon_url      VARCHAR(512),
    banner_url    VARCHAR(512),
    screenshots   JSONB NOT NULL DEFAULT '[]',
    featured      BOOLEAN NOT NULL DEFAULT FALSE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Статистика
    total_downloads BIGINT NOT NULL DEFAULT 0,
    rating_sum    INTEGER NOT NULL DEFAULT 0,
    rating_count  INTEGER NOT NULL DEFAULT 0,

    -- Владелец
    owner_id      UUID NOT NULL REFERENCES marketplace_accounts(id)
);

CREATE TABLE marketplace_versions (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    module_slug   VARCHAR(64) NOT NULL REFERENCES marketplace_modules(slug),
    version       VARCHAR(32) NOT NULL,
    rustok_min    VARCHAR(32) NOT NULL,
    rustok_max    VARCHAR(32),
    changelog     TEXT,
    crate_url     VARCHAR(512) NOT NULL,     -- URL to .crate archive
    checksum      VARCHAR(64) NOT NULL,      -- SHA-256
    crate_size    BIGINT NOT NULL,
    manifest      JSONB NOT NULL,            -- Parsed rustok-module.toml
    yanked        BOOLEAN NOT NULL DEFAULT FALSE,
    published_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(module_slug, version)
);

CREATE TABLE marketplace_dependencies (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    version_id    UUID NOT NULL REFERENCES marketplace_versions(id),
    depends_on    VARCHAR(64) NOT NULL,      -- slug другого модуля
    version_req   VARCHAR(32) NOT NULL,      -- ">= 1.0.0"
    UNIQUE(version_id, depends_on)
);
```

### 4.3. Crate Storage (Хранилище артефактов)

Хранит скомпилированные `.crate` архивы (аналог crates.io):

```
crate-storage/
├── blog/
│   ├── rustok-blog-1.0.0.crate     # tar.gz с исходниками
│   ├── rustok-blog-1.1.0.crate
│   └── rustok-blog-1.2.0.crate
├── seo-tools/
│   └── rustok-seo-1.0.0.crate
```

**Формат `.crate`**: стандартный Cargo формат (tar.gz с Cargo.toml + src/).

**Хранение**: S3-совместимое (MinIO для self-hosted, AWS S3 для cloud).

### 4.4. Категории маркетплейса

Фиксированный набор категорий (расширяется только core-командой):

| Категория | Slug | Описание |
|---|---|---|
| Контент | `content` | CMS, блоги, страницы, медиа |
| E-commerce | `commerce` | Магазины, оплата, корзина, каталог |
| Аналитика | `analytics` | Метрики, дашборды, отчёты |
| Социальные | `social` | Комментарии, форумы, уведомления |
| SEO | `seo` | Поисковая оптимизация, sitemap |
| Интеграции | `integrations` | Внешние сервисы, API, webhooks |
| Инструменты | `dev-tools` | Скрипты, миграции, утилиты |
| Безопасность | `security` | Аудит, 2FA, защита |
| Локализация | `localization` | Переводы, мультиязычность |
| Тема/UI | `themes` | Темы, компоненты оформления |

---

## 5. Верификация модулей (Validation Pipeline)

### 5.1. Зачем верификация

В WordPress плагины — частый вектор атак. В RusTok модуль компилируется
в бинарник и получает полный доступ к runtime. Верификация **критична**.

### 5.2. Уровни проверки

```
┌─────────────────────────────────────────────────────────────────┐
│                    Validation Pipeline                           │
│                                                                  │
│  Stage 1: STATIC CHECKS (секунды)                               │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  ✓ rustok-module.toml валиден                              │ │
│  │  ✓ slug уникальный в реестре                               │ │
│  │  ✓ version — новый semver (не переписывает существующий)   │ │
│  │  ✓ license — из разрешённого списка                        │ │
│  │  ✓ Cargo.toml name == [crate].name в манифесте             │ │
│  │  ✓ locales/en.json существует                              │ │
│  │  ✓ icon.svg существует (для marketplace)                   │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  Stage 2: SECURITY AUDIT (минуты)                               │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  ✓ cargo-audit — нет known vulnerabilities                 │ │
│  │  ✓ Нет unsafe блоков без #[doc(reason)]                    │ │
│  │  ✓ Нет прямого fs::read / fs::write (только через API)    │ │
│  │  ✓ Нет std::net (только через platform HTTP client)        │ │
│  │  ✓ Нет std::process::Command                               │ │
│  │  ✓ Нет #[no_mangle] / extern "C" без обоснования          │ │
│  │  ✓ dependencies — из allow-list crate'ов                    │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  Stage 3: COMPILATION (минуты)                                  │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  ✓ Компилируется с rustok_min версией                      │ │
│  │  ✓ Компилируется с rustok_max версией (если указана)       │ │
│  │  ✓ Компилируется с latest rustok                           │ │
│  │  ✓ impl RusToKModule — трейт реализован корректно          │ │
│  │  ✓ impl MigrationSource — миграции доступны                │ │
│  │  ✓ Нет compile warnings                                    │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  Stage 4: RUNTIME TESTS (минуты)                                │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  ✓ cargo test — все тесты проходят                         │ │
│  │  ✓ Миграции: up + down идемпотентны                        │ │
│  │  ✓ on_enable() → OK                                        │ │
│  │  ✓ on_disable() → OK                                       │ │
│  │  ✓ health() → Healthy                                      │ │
│  │  ✓ GraphQL schema валидна (если есть provides.graphql)     │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  Stage 5: METADATA QUALITY (секунды)                            │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  ✓ description >= 20 символов                              │ │
│  │  ✓ long_description >= 100 символов                        │ │
│  │  ✓ icon.svg — валидный SVG, < 50KB                         │ │
│  │  ✓ screenshots — валидные PNG/JPG, < 2MB каждый            │ │
│  │  ✓ changelog для версии заполнен                           │ │
│  │  ✓ Все label_key из admin_nav есть в locales/en.json       │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  ═══════════════════════════════════════════════════════════    │
│  ALL PASSED → опубликовано в реестре                             │
│  ANY FAILED → отклонено с детальным отчётом                     │
└─────────────────────────────────────────────────────────────────┘
```

### 5.3. Restricted API — что модулю запрещено

Модуль получает доступ к платформе **только через определённые API**:

| Разрешено | Через что | Пример |
|---|---|---|
| БД (своя schema) | `ModuleContext.db` | SeaORM queries |
| Конфиг тенанта | `ModuleContext.config` | serde_json::Value |
| Permissions | `fn permissions()` | Объявление, не проверка |
| Events | `fn event_listeners()` | Подписка через trait |
| GraphQL | `provides.graphql` | Расширение схемы |
| i18n | `locales/*.json` | Через platform i18n |

| **Запрещено** | Почему |
|---|---|
| Прямой доступ к FS | Безопасность — модуль не должен читать /etc/passwd |
| Прямой network (TCP/HTTP) | Модуль не должен звонить домой |
| `std::process::Command` | Не должен запускать процессы |
| `unsafe` без обоснования | Потенциальный UB |
| Доступ к чужим таблицам | Изоляция данных между модулями |

### 5.4. Trust Levels (уровни доверия)

```
┌─────────────────────────────────────────────────┐
│  🟢 Official — Модули от команды RusTok        │
│     - Полный доступ к platform API              │
│     - Автоматически рекомендуются               │
│     - Бейдж "Official" в каталоге               │
├─────────────────────────────────────────────────┤
│  🔵 Verified — Прошли полную верификацию        │
│     - Все 5 стадий pipeline пройдены            │
│     - Бейдж "Verified" в каталоге               │
│     - Стандартный уровень доступа               │
├─────────────────────────────────────────────────┤
│  🟡 Community — Базовая проверка               │
│     - Stage 1-3 пройдены                        │
│     - Stage 4-5 опционально                     │
│     - Бейдж "Community" + предупреждение        │
├─────────────────────────────────────────────────┤
│  ⚪ Private — Приватные модули                  │
│     - Не в маркетплейсе                         │
│     - Ответственность на операторе              │
│     - Устанавливаются через git/path            │
└─────────────────────────────────────────────────┘
```

---

## 6. GraphQL API маркетплейса

### 6.1. Queries — для Admin UI

```graphql
# ──────────────────────────────────────────────────
# Каталог
# ──────────────────────────────────────────────────

type Query {
  # Поиск и фильтрация модулей в каталоге
  marketplace(
    search: String             # Полнотекстовый поиск
    category: String           # Фильтр по категории
    tag: String                # Фильтр по тегу
    trustLevel: TrustLevel     # official / verified / community
    onlyCompatible: Boolean    # Только совместимые с текущей версией
    sort: MarketplaceSort      # downloads / rating / newest / name
    limit: Int = 20
    offset: Int = 0
  ): MarketplaceConnection!

  # Детали одного модуля
  marketplaceModule(slug: String!): MarketplaceModule

  # Категории с количеством модулей
  marketplaceCategories: [MarketplaceCategory!]!

  # Проверка совместимости перед установкой
  checkCompatibility(
    slug: String!
    version: String!
  ): CompatibilityReport!

  # Какие обновления доступны для установленных модулей
  availableUpdates: [ModuleUpdate!]!
}

# ──────────────────────────────────────────────────
# Типы
# ──────────────────────────────────────────────────

type MarketplaceModule {
  slug: String!
  name: String!
  latestVersion: String!
  description: String!
  longDescription: String

  # Авторство
  authors: [String!]!
  license: String!
  repository: String
  homepage: String
  trustLevel: TrustLevel!

  # Визуал
  iconUrl: String
  bannerUrl: String
  screenshots: [String!]!

  # Каталог
  category: String!
  tags: [String!]!
  featured: Boolean!

  # Совместимость
  rustokMinVersion: String!
  rustokMaxVersion: String

  # Зависимости
  dependencies: [ModuleDependency!]!
  conflicts: [String!]!

  # Что предоставляет
  providesAdminNav: Boolean!
  providesStorefrontSlots: Boolean!
  providesGraphql: Boolean!
  permissionsCount: Int!

  # Настройки
  hasSettings: Boolean!

  # Статистика
  totalDownloads: Int!
  rating: Float                  # 0.0 - 5.0
  reviewCount: Int!

  # Версии
  versions: [ModuleVersionInfo!]!

  # Статус на текущей платформе
  installed: Boolean!
  installedVersion: String
  updateAvailable: Boolean!
  compatible: Boolean!
}

type ModuleDependency {
  slug: String!
  versionConstraint: String!    # ">= 1.0.0"
  satisfied: Boolean!           # Есть ли на платформе
  installedVersion: String      # Какая версия стоит (null если нет)
}

type CompatibilityReport {
  compatible: Boolean!
  issues: [CompatibilityIssue!]!
  missingDependencies: [ModuleDependency!]!
  willAutoInstall: [String!]!   # Зависимости, которые поставятся автоматически
  conflictsWithInstalled: [String!]!
}

type CompatibilityIssue {
  severity: IssueSeverity!      # error / warning / info
  message: String!
  detail: String
}

type ModuleUpdate {
  slug: String!
  name: String!
  currentVersion: String!
  latestVersion: String!
  changelog: String
  breaking: Boolean!            # semver major bump
}

enum TrustLevel {
  OFFICIAL
  VERIFIED
  COMMUNITY
}

enum MarketplaceSort {
  DOWNLOADS
  RATING
  NEWEST
  NAME
}
```

### 6.2. Mutations — для install/uninstall

```graphql
type Mutation {
  # Установить модуль из маркетплейса → запускает build pipeline
  installModule(
    slug: String!
    version: String!
    autoInstallDeps: Boolean = true    # Автоматически ставить зависимости
  ): BuildJob!

  # Удалить модуль → запускает build pipeline
  uninstallModule(slug: String!): BuildJob!

  # Обновить до новой версии
  upgradeModule(slug: String!, version: String!): BuildJob!

  # Обновить все модули с доступными обновлениями
  upgradeAllModules: BuildJob!

  # Откат к предыдущей сборке
  rollbackBuild(buildId: ID!): BuildJob!
}

type Subscription {
  # Реальтайм прогресс сборки
  buildProgress(buildId: ID!): BuildJob!
}
```

### 6.3. Mutations — для авторов модулей (publish)

```graphql
type Mutation {
  # Опубликовать версию модуля
  publishModuleVersion(input: PublishInput!): PublishResult!

  # Отозвать версию (yank)
  yankVersion(slug: String!, version: String!): Boolean!

  # Обновить метаданные (описание, скриншоты)
  updateModuleMetadata(slug: String!, input: UpdateMetadataInput!): MarketplaceModule!
}

input PublishInput {
  # Crate архив (upload)
  crateArchive: Upload!
  # Или URL на .crate файл
  # crateUrl: String

  # Changelog для этой версии
  changelog: String
}

type PublishResult {
  success: Boolean!
  module: MarketplaceModule
  validationReport: ValidationReport
}

type ValidationReport {
  passed: Boolean!
  stages: [ValidationStage!]!
  errors: [String!]!
  warnings: [String!]!
  duration: Int!               # Время проверки в секундах
}

type ValidationStage {
  name: String!                # "static_checks", "security_audit", etc.
  passed: Boolean!
  duration: Int!
  details: [String!]!
}
```

---

## 7. CLI для разработчиков модулей

### 7.1. Команды

```bash
# ──────────────────────────────────────────────────
# Создание модуля
# ──────────────────────────────────────────────────

# Scaffold нового модуля с шаблоном
rustok module init my-module
# Создаёт:
#   my-module/
#   ├── rustok-module.toml (шаблон)
#   ├── Cargo.toml
#   ├── src/lib.rs (impl RusToKModule)
#   ├── locales/en.json
#   └── assets/icon.svg (placeholder)

# ──────────────────────────────────────────────────
# Разработка
# ──────────────────────────────────────────────────

# Проверить манифест локально
rustok module check
# → ✓ rustok-module.toml valid
# → ✓ slug "my-module" is valid
# → ✓ version "0.1.0" is valid semver
# → ✓ locales/en.json found
# → ✗ icon.svg not found (required for marketplace)

# Запустить тесты совместимости
rustok module test
# → Compiling with rustok 0.5.0 (min)... OK
# → Compiling with rustok latest... OK
# → Running cargo test... 12 tests passed
# → Checking migrations up/down... OK
# → Checking on_enable/on_disable hooks... OK

# Сборка с RusTok для локального тестирования
rustok module dev
# → Adds module to local modules.toml
# → Rebuilds server with module included
# → Starts dev server

# ──────────────────────────────────────────────────
# Публикация
# ──────────────────────────────────────────────────

# Логин в маркетплейс
rustok auth login
# → Opens browser for OAuth / API token

# Dry-run публикации
rustok module publish --dry-run
# → Running validation pipeline...
# → Stage 1: Static checks ✓
# → Stage 2: Security audit ✓
# → Stage 3: Compilation ✓
# → Stage 4: Runtime tests ✓
# → Stage 5: Metadata quality ✓
# → Ready to publish rustok-blog@1.2.0

# Публикация
rustok module publish
# → Publishing rustok-blog@1.2.0...
# → Uploading crate (245 KB)...
# → Running validation pipeline...
# → Published! https://modules.rustok.dev/modules/blog

# ──────────────────────────────────────────────────
# Управление
# ──────────────────────────────────────────────────

# Отозвать версию
rustok module yank 1.1.0
# → Yanked rustok-blog@1.1.0

# Список своих модулей
rustok module list --mine
# → blog       1.2.0  official  ↓12,345
# → seo-tools  1.0.0  verified  ↓3,456

# Поиск в маркетплейсе
rustok module search "blog"
# → blog        1.2.0  official  Blogging engine with categories
# → microblog   0.3.0  community Microblogging module
```

### 7.2. Аутентификация авторов

> **См. также**: Полная архитектура auth-потоков с маркетплейсом —
> `docs/concepts/plan-oauth2-app-connections.md`, Приложение A.

```
┌────────────────────────────────────────────────────────┐
│  Как стать автором модуля                               │
│                                                         │
│  1. Зарегистрироваться на modules.rustok.dev            │
│  2. Подтвердить email                                   │
│  3. (Опционально) Верифицировать организацию            │
│  4. Получить API token: rustok auth login               │
│     → OAuth PKCE flow (client_id: "rustok-cli")         │
│  5. Опубликовать первый модуль: rustok module publish   │
│                                                         │
│  Авторы могут быть:                                     │
│  - Индивидуальные разработчики                          │
│  - Организации (несколько участников)                   │
│  - Core-команда RusTok (official)                       │
│                                                         │
│  marketplace_accounts — аккаунты авторов на стороне     │
│  маркетплейса, не в основной БД RusTok.                 │
└────────────────────────────────────────────────────────┘
```

---

## 8. UI маркетплейса в админке

### 8.1. Навигация

Страница `/modules` расширяется вкладками:

```
┌─────────────────────────────────────────────────────────────────┐
│  Модули                                                         │
│  Управление модулями платформы                                  │
│                                                                  │
│  ┌───────────┐ ┌───────────┐ ┌───────────┐ ┌──────────────┐   │
│  │ Installed  │ │ Catalog   │ │ Updates   │ │ Build History│   │
│  │ (active)   │ │           │ │    (3)    │ │              │   │
│  └───────────┘ └───────────┘ └───────────┘ └──────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

### 8.2. Вкладка "Installed" (текущая функциональность)

То что уже есть — core и optional модули с toggle switch.

### 8.3. Вкладка "Catalog" (маркетплейс)

```
┌─────────────────────────────────────────────────────────────────┐
│  Каталог модулей                                                │
│                                                                  │
│  ┌─────────────────────────────────────────┐ ┌────────────────┐│
│  │ 🔍 Поиск модулей...                     │ │ Категория ▾   ││
│  └─────────────────────────────────────────┘ └────────────────┘│
│                                                                  │
│  ★ Рекомендованные                                              │
│  ┌────────────────────────┐ ┌────────────────────────┐         │
│  │ 🟢 SEO Tools    v1.0  │ │ 🟢 Analytics    v2.1  │         │
│  │ Official               │ │ Official               │         │
│  │ Meta-теги, sitemap,   │ │ Метрики, дашборды,    │         │
│  │ structured data        │ │ воронки конверсии      │         │
│  │ ↓ 12,345              │ │ ↓ 8,901               │         │
│  │ ★★★★★ (45)            │ │ ★★★★☆ (23)            │         │
│  │                        │ │                        │         │
│  │ [Установить]           │ │ [Установить]           │         │
│  └────────────────────────┘ └────────────────────────┘         │
│                                                                  │
│  Контент                                                        │
│  ┌────────────────────────┐ ┌────────────────────────┐         │
│  │ 🔵 Gallery      v0.8  │ │ 🟡 Microblog    v0.3  │         │
│  │ Verified                │ │ Community              │         │
│  │ Галерея изображений    │ │ Микроблог в стиле      │         │
│  │ с lightbox             │ │ Twitter                 │         │
│  │ Зависит от: content   │ │                        │         │
│  │                        │ │ ⚠ Не верифицирован    │         │
│  │ [Установить]           │ │ [Установить]           │         │
│  └────────────────────────┘ └────────────────────────┘         │
└─────────────────────────────────────────────────────────────────┘
```

### 8.4. Детальная страница модуля

```
┌─────────────────────────────────────────────────────────────────┐
│  ← Назад в каталог                                              │
│                                                                  │
│  ┌────┐  SEO Tools                           🟢 Official       │
│  │icon│  v1.0.0 · MIT · RusTok Team                            │
│  └────┘  Meta-теги, sitemap, Open Graph, structured data        │
│                                                                  │
│  ┌───────────┐ ┌───────────┐                                   │
│  │ Установить │ │ GitHub ↗  │                                   │
│  └───────────┘ └───────────┘                                   │
│                                                                  │
│  ─────────────────────────────────────────────────              │
│                                                                  │
│  📋 Описание                                                    │
│  Full-featured SEO module for RusTok platform...                │
│                                                                  │
│  📸 Скриншоты                                                   │
│  [screenshot-1] [screenshot-2] [screenshot-3]                   │
│                                                                  │
│  📦 Зависимости                                                 │
│  content >= 1.0.0  ✓ установлен (v1.2.0)                       │
│                                                                  │
│  🔒 Разрешения                                                  │
│  seo:read, seo:write, seo:configure                             │
│                                                                  │
│  📊 Статистика                                                  │
│  ↓ 12,345 установок · ★★★★★ 4.8 (45 отзывов)                 │
│                                                                  │
│  📝 Changelog v1.0.0                                            │
│  - Initial release                                               │
│  - Meta tags per page                                            │
│  - Auto-generated sitemap.xml                                    │
│                                                                  │
│  🔄 Версии                                                      │
│  1.0.0  2026-02-15  Текущая                                    │
│  0.9.0  2026-01-20  Beta                                        │
└─────────────────────────────────────────────────────────────────┘
```

### 8.5. Вкладка "Updates"

```
┌─────────────────────────────────────────────────────────────────┐
│  Доступные обновления (3)               [Обновить все]          │
│                                                                  │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  Blog         1.1.0 → 1.2.0              [Обновить]      │  │
│  │  - Added scheduled publishing                             │  │
│  │  - Fixed category pagination                              │  │
│  └───────────────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  SEO Tools    1.0.0 → 1.1.0              [Обновить]      │  │
│  │  - JSON-LD structured data support                        │  │
│  └───────────────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  ⚠ Commerce   2.0.0 → 3.0.0  BREAKING    [Обновить]     │  │
│  │  - New payment API (breaking change)                      │  │
│  │  - Migration required                                     │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### 8.6. Прогресс сборки (при install/uninstall/upgrade)

```
┌─────────────────────────────────────────────────────────────────┐
│  Установка модуля SEO Tools v1.0.0                              │
│                                                                  │
│  ✓ Проверка совместимости              2s                       │
│  ✓ Обновление манифеста                1s                       │
│  ● Компиляция                          ...                      │
│    ████████████████░░░░░░░░  62%                                │
│    Compiling rustok-seo v1.0.0                                  │
│  ○ Тестирование                                                 │
│  ○ Сборка образа                                                │
│  ○ Деплой                                                       │
│                                                                  │
│  Время: 1:42 / ~3:00                                            │
│                                                                  │
│  [Отменить]                                                     │
└─────────────────────────────────────────────────────────────────┘
```

---

## 9. Deployment Topologies

Маркетплейс и rebuild pipeline работают **одинаково** для пользователя,
независимо от того, как развёрнута платформа:

### 9.1. Монолит (self-hosted)

```
modules.rustok.dev
       │ download .crate
       ▼
┌────────────────────┐
│  Build Worker      │  ← На том же сервере
│  cargo build       │
│  systemctl restart │
└────────────────────┘
```

- Build worker встроен в сервер.
- `cargo build` на production-машине.
- Zero-downtime через graceful restart.
- Простейший вариант для MVP.

### 9.2. Docker / Docker Compose

```
modules.rustok.dev
       │ download .crate
       ▼
┌────────────────────┐      ┌────────────────┐
│  Build Worker      │─────>│  Registry      │
│  cargo build       │      │  (Docker Hub / │
│  docker build      │      │   private)     │
└────────────────────┘      └───────┬────────┘
                                    │ pull
                                    ▼
                            ┌────────────────┐
                            │  docker compose│
                            │  rolling update│
                            └────────────────┘
```

### 9.3. Kubernetes

```
modules.rustok.dev
       │ download .crate
       ▼
┌────────────────────┐      ┌────────────────┐
│  K8s Job (kaniko)  │─────>│  Registry      │
│  cargo build       │      │  (ECR/GCR)     │
│  kaniko build      │      └───────┬────────┘
└────────────────────┘              │
                                    ▼
                            ┌────────────────┐
                            │  K8s Deployment│
                            │  rolling update│
                            │  canary / blue │
                            │  green deploy  │
                            └────────────────┘
```

### 9.4. Headless (разнесённые фронтенды)

```
modules.rustok.dev
       │
       ├──────────────────────────────┐
       ▼                              ▼
┌────────────────┐           ┌────────────────┐
│ Server rebuild │           │ Frontend rebuild│
│ (API + GraphQL)│           │ (Leptos SSR    │
│ region: eu     │           │  or Next.js)   │
└────────────────┘           │ region: us     │
                             └────────────────┘
```

- Build pipeline собирает несколько артефактов.
- Server и frontend деплоятся независимо.
- Для пользователя — та же кнопка "Установить".

---

## 10. Формирование маркетплейса — дорожная карта

### Phase 0: Внутренний реестр (текущее состояние)

- ✅ Встроенные модули в монорепе.
- ✅ `modules.toml` как source of truth.
- ✅ Tenant-level toggle в UI.
- ✅ Provider abstraction для внутреннего каталога с `local-manifest` как первым concrete source.
- ✅ Internal `registry` provider уже умеет делать базовый HTTP fetch по `{registry_url}/catalog`, кэшировать ответ в памяти и мягко деградировать в `local-manifest`, если registry недоступен.
- ✅ Internal registry payload уже versioned (`schema_version`) и может нести compatibility bounds (`rustok_min_version` / `rustok_max_version`); платформа уже рассчитывает `compatible` на catalog projection layer.
- ✅ Internal catalog contract уже несет provenance/history baseline: `publisher`, checksum/signature presence и `versions[]` доступны на projection layer даже до появления полноценного external registry UI.
- ✅ Internal GraphQL contract уже умеет detail/query baseline: `marketplace(...)` поддерживает richer optional filters, а `marketplaceModule(slug)` уже сидит на том же projection layer и готов под будущую detail page.
- ✅ Обе текущие админки уже отображают provenance/detail baseline на `/modules`: compatibility, publisher, checksum/signature presence и short version history видны прямо в catalog/update cards.
- ❌ Нет внешнего каталога.
- ⏳ Следующий шаг этой фазы: договориться о verified badge policy, richer dedicated detail page и затем включить `git/private` providers в ту же federated chain.

### Phase 1: Стандарт и валидатор

**Цель**: Все встроенные модули следуют единому стандарту.

- Определить формат `rustok-module.toml` (см. секцию 3).
- Добавить `rustok-module.toml` ко всем встроенным first-party path-модулям.
- Написать `rustok module check` валидатор (server + `cargo xtask validate-manifest` baseline уже есть; publish-grade CLI остается отдельным шагом).
- Интегрировать валидацию в CI.

**Критерий готовности**: Все 10 встроенных path-модулей имеют валидный `rustok-module.toml`.

### Phase 2: Build Pipeline

**Зависимость**: OAuth2 AS должен быть реализован (см. `plan-oauth2-app-connections.md`),
т.к. Build Worker аутентифицируется через `client_credentials` (`rustok-internal-worker`).

**Цель**: Install/uninstall через админку с автоматической пересборкой.

- `ManifestManager` — CRUD для `modules.toml`.
- Build Service — запуск `cargo build` по запросу.
- GraphQL mutations: `installModule`, `uninstallModule`.
- GraphQL subscription: `buildProgress` (server schema/event hub contract is in place; websocket/client transport still pending, and current admin baseline uses live polling on top of `activeBuild`/`buildHistory`).
- UI: прогресс-бар, история сборок.
- Permissions: `marketplace:install`, `builds:view`, `builds:manage`.

**Критерий готовности**: Модуль можно установить из git URL через админку.

### Phase 3: Публичный реестр

**Цель**: modules.rustok.dev — каталог модулей.

- Database schema для Module Index.
- S3 storage для .crate архивов.
- GraphQL API для marketplace queries.
- Validation Pipeline (5 стадий).
- Аутентификация авторов.
- `rustok module publish` CLI.

**Критерий готовности**: Сторонний разработчик может опубликовать модуль.

### Phase 4: Каталог в админке

**Цель**: Browse + Install из админки как в WordPress.

- UI: вкладки Installed / Catalog / Updates / History.
- Поиск и фильтрация.
- Детальная страница модуля.
- Проверка совместимости перед установкой.
- Обновления с changelog.

**Критерий готовности**: Оператор может найти, оценить и установить модуль из UI.

### Phase 5: Экосистема

**Цель**: Живой маркетплейс с сообществом.

- Рейтинги и отзывы.
- Статистика (downloads, активные установки).
- Уведомления об обновлениях.
- Организации (team accounts).
- Рекомендации ("modules like this").
- (Будущее) Монетизация — платные модули.

**Критерий готовности**: > 20 модулей от > 5 авторов.

---

## 11. Безопасность маркетплейса

| Угроза | Митигация |
|---|---|
| Вредоносный модуль | Validation pipeline + security audit stage |
| Supply chain attack (зависимости) | cargo-audit + allow-list crate'ов |
| Подмена .crate (MITM) | SHA-256 checksum + TLS |
| Компрометация аккаунта автора | 2FA для авторов + yank вместо удаления |
| Модуль звонит домой | Запрет std::net, проверка на Stage 2 |
| Модуль читает чужие данные | Изоляция через ModuleContext (только свой tenant) |
| Typosquatting (slug) | Проверка на сходство с official модулями |
| Сломанная миграция | Проверка up/down идемпотентности на Stage 4 |
| Breaking update | semver enforcement + breaking warning в UI |

---

## 12. Итого

Маркетплейс модулей RusTok — это **три компонента**:

1. **Стандарт** (`rustok-module.toml`) — единый контракт для любого модуля.
2. **Реестр** (modules.rustok.dev) — каталог + хранилище + верификация.
3. **Инструменты** (CLI + Admin UI) — публикация для авторов, установка для операторов.

Для пользователя это выглядит как WordPress:
кнопка "Установить" → прогресс-бар → готово.
