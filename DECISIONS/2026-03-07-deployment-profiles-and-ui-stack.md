# Deployment Profiles и выбор UI-стека

- Date: 2026-03-07
- Status: Partially superseded by `2026-03-29-leptos-server-functions-as-internal-data-layer.md`
  (компонуемые профили деплоя остаются в силе; транспорт между Leptos UI и сервером пересмотрен)

> Уточнение после внедрения `#[server]`: для Leptos UI native server functions и GraphQL живут параллельно. `#[server]` стал preferred internal path, но `/api/graphql` не удаляется.

## Context

RusTok поддерживает два UI-стека:

- **Leptos** (Rust) — admin (`apps/admin`) + storefront (`apps/storefront`)
- **Next.js** (TypeScript) — admin (`apps/next-admin`) + storefront (`apps/next-frontend`)

Первая итерация ADR предлагала 3 жёстких профиля (`monolith | headless-leptos |
headless-next`), но реальные сценарии гибче:

> «Мы были на монолите, но захотели вынести storefront на Next.js для двух
> сайтов в разных регионах, а бэкенд с админкой оставить вместе»

Это невозможно выразить через 3 preset'а — нужна **компонуемая модель**.

## Decision

### 1. Компонуемые слои вместо жёстких профилей

Каждый слой (server, admin, storefront) конфигурируется **независимо**:

```toml
# modules.toml

[build]
target = "x86_64-unknown-linux-gnu"
profile = "release"

# ───────────────────────────────────────────────
# Server: всегда Axum (Rust). Вопрос — что встроить.
# ───────────────────────────────────────────────
[build.server]
embed_admin = true          # Встроить Leptos admin в бинарник?
embed_storefront = false    # Встроить Leptos storefront в бинарник?

# ───────────────────────────────────────────────
# Admin: если embed_admin = false, нужен отдельный процесс
# ───────────────────────────────────────────────
[build.admin]
stack = "leptos"            # "leptos" | "next"
# deploy = "embedded" выводится из embed_admin = true

# ───────────────────────────────────────────────
# Storefronts: один или несколько (мультисайт)
# ───────────────────────────────────────────────
[[build.storefront]]
id = "default"
stack = "next"              # "leptos" | "next"
# deploy = "standalone" выводится из embed_storefront = false
```

### 2. Типичные конфигурации

#### WordPress-монолит (всё в одном)

```toml
[build.server]
embed_admin = true
embed_storefront = true

[build.admin]
stack = "leptos"

[[build.storefront]]
id = "default"
stack = "leptos"
```

**Результат**: 1 Rust-бинарник. Admin на `/admin`, storefront на `/`.

#### Headless Next.js (Strapi-стиль)

```toml
[build.server]
embed_admin = false
embed_storefront = false

[build.admin]
stack = "next"

[[build.storefront]]
id = "default"
stack = "next"
```

**Результат**: 1 Rust API + 1 Node.js admin + 1 Node.js storefront.

#### Гибрид: монолит-админка + Next.js мультисайт

Сценарий: бэкенд + админка вместе (один бинарник), а 2 storefront'а на Next.js
в разных регионах.

```toml
[build.server]
embed_admin = true           # Админка встроена в сервер
embed_storefront = false     # Storefront — отдельно

[build.admin]
stack = "leptos"             # Leptos встроен в Axum

[[build.storefront]]
id = "site-eu"
stack = "next"

[[build.storefront]]
id = "site-us"
stack = "next"
```

**Результат**: 1 Rust-бинарник (API + admin) + 2 Node.js storefront'а.

```
                   ┌──────────────────────────────┐
                   │  rustok-server (Rust binary)  │
                   │  ┌────────────────────────┐  │
                   │  │ Axum API (GraphQL)      │  │
                   │  ├────────────────────────┤  │
                   │  │ Leptos Admin (WASM)     │  │  ← /admin
                   │  └────────────────────────┘  │
                   └──────────────┬───────────────┘
                                  │ GraphQL
                      ┌───────────┴───────────┐
                      │                       │
               ┌──────┴──────┐         ┌──────┴──────┐
               │ Next.js     │         │ Next.js     │
               │ site-eu     │         │ site-us     │
               │ EU region   │         │ US region   │
               └─────────────┘         └─────────────┘
```

#### Полный headless Leptos (для max performance)

```toml
[build.server]
embed_admin = false
embed_storefront = false

[build.admin]
stack = "leptos"

[[build.storefront]]
id = "default"
stack = "leptos"
```

**Результат**: 3 Rust-бинарника, независимо деплоятся.

#### Leptos admin + Leptos storefront EU + Next.js storefront US

```toml
[build.server]
embed_admin = true
embed_storefront = false

[build.admin]
stack = "leptos"

[[build.storefront]]
id = "main-site"
stack = "leptos"

[[build.storefront]]
id = "us-site"
stack = "next"
```

**Результат**: 1 Rust-бинарник (API + admin) + 1 Rust SSR + 1 Node.js.
Можно даже миксовать стеки storefront'ов.

### 3. Реализация через Cargo features

```toml
# apps/server/Cargo.toml
[features]
default = []

# Встраивает Leptos admin WASM assets в сервер
embed-admin = ["dep:admin-assets"]

# Встраивает Leptos storefront SSR в сервер
embed-storefront = ["dep:leptos-storefront"]
```

Build pipeline читает `[build.server]` и собирает features:

```bash
# embed_admin=true, embed_storefront=true → монолит
cargo build -p rustok-server --release \
  --features "embed-admin,embed-storefront"

# embed_admin=true, embed_storefront=false → админка встроена, storefront отдельно
cargo build -p rustok-server --release \
  --features "embed-admin"

# embed_admin=false, embed_storefront=false → чистый API
cargo build -p rustok-server --release
```

Для отдельных storefront'ов:

```bash
# Leptos storefront → отдельный Rust SSR бинарник
cargo build -p rustok-storefront --release

# Next.js storefront → npm build
cd apps/next-frontend && npm run build
```

### 4. Миграция между конфигурациями

Переход с монолита на гибрид — это **изменение `modules.toml` + пересборка**.
Данные (БД, tenant_modules, пользователи) не затрагиваются.

```bash
# Было: монолит
# Хотим: бэкенд+админка вместе, storefront на Next.js

# 1. Обновить modules.toml
[build.server]
embed_admin = true
embed_storefront = false    # ← было true

[[build.storefront]]
id = "default"
stack = "next"              # ← было leptos

# 2. Пересборка
rustok rebuild
# → Собирает server (с админкой, без storefront)
# → Собирает Next.js storefront
# → Деплоит оба

# 3. Данные — без изменений
# GraphQL API тот же, tenant_modules те же,
# storefront просто берёт данные из другого стека
```

### 5. DeploymentProfile в БД

Enum в builds table остаётся для обратной совместимости, но расширяется:

```rust
pub enum DeploymentProfile {
    /// Всё в одном: server + admin + storefront
    Monolith,
    /// Server + embedded admin, storefronts отдельно
    ServerWithAdmin,
    /// Server + embedded storefront, admin отдельно
    ServerWithStorefront,
    /// Чистый API, всё остальное отдельно
    HeadlessApi,
}
```

Вычисляется автоматически из `[build.server]`:

| `embed_admin` | `embed_storefront` | Profile |
|---|---|---|
| `true` | `true` | `Monolith` |
| `true` | `false` | `ServerWithAdmin` |
| `false` | `true` | `ServerWithStorefront` |
| `false` | `false` | `HeadlessApi` |

### 6. Мультисайт — storefront per tenant

`[[build.storefront]]` поддерживает массив, что позволяет:

- **Разные регионы**: site-eu, site-us — один и тот же код, разные инстансы.
- **Разные стеки**: site-main на Leptos (performance), site-promo на Next.js (команда знает React).
- **Разные tenant'ы**: каждый storefront может обслуживать подмножество tenant'ов.

```toml
[[build.storefront]]
id = "main"
stack = "leptos"
tenants = ["*"]              # Все тенанты по умолчанию

[[build.storefront]]
id = "promo-us"
stack = "next"
tenants = ["acme-us", "beta-us"]  # Только конкретные тенанты
```

Маршрутизация (какой storefront обслуживает какой tenant) — через:
- DNS (tenant.rustok.dev → конкретный storefront)
- Reverse proxy (nginx/traefik routing rules)
- Или через конфиг в БД (`tenant.storefront_url`).

### 7. Маркетплейс — profile-agnostic

Маркетплейс модулей **не зависит от конфигурации**. Модуль работает в любом
варианте, потому что:

- Backend-часть — **всегда одинаковая** (RusToKModule trait).
- UI-часть — **оба стека в одном crate**, build pipeline берёт нужный.

Если модуль не имеет UI для конкретного стека — это ОК.
Backend-функциональность (GraphQL, миграции, events) работает.
UI просто не показывается в этом storefront'е.

## Consequences

### Позитивные

- **Любая комбинация** — монолит, гибрид, полный headless, мультисайт.
- **Пример с монолита на Next.js storefront** — просто меняем два поля в TOML.
- **Мультисайт из коробки** — `[[build.storefront]]` массив.
- **Миксование стеков** — один storefront на Leptos, другой на Next.js.
- **Данные не меняются** — переключение стека = только пересборка.

### Негативные

- **Сложнее для новичков** — больше полей чем один `deployment_profile`.
  Митигация: preset'ы (шаблоны) через CLI: `rustok init --preset monolith`.
- **Build pipeline сложнее** — нужно собирать разные артефакты для разных storefront'ов.
- **Тестирование** — больше комбинаций в CI.

### Follow-up

1. Обновить `modules.toml` на новый формат `[build.server]` / `[[build.storefront]]`.
2. Обновить `DeploymentProfile` enum: `Monolith | ServerWithAdmin | ServerWithStorefront | HeadlessApi`.
3. Добавить Cargo features: `embed-admin`, `embed-storefront`.
4. CLI preset'ы: `rustok init --preset monolith`, `--preset headless-next`, `--preset hybrid`.
5. Build pipeline: генерация команд сборки на основе TOML конфигурации.
6. Документировать типичные конфигурации в README.
