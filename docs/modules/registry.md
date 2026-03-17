# Module & Application Registry

This document provides a comprehensive map of all components within the RusToK ecosystem, including their relationships and responsibilities.

## High-Level Architecture

```mermaid
graph TD
    subgraph Applications
        SERVER[apps/server - Loco Server]
        ADMIN[apps/admin - Leptos Admin]
        SF[apps/storefront - Leptos Storefront]
        NEXT_ADMIN[apps/next-admin - Next.js Admin]
        NEXT_SF[apps/next-frontend - Next.js Storefront]
    end

    subgraph Domain Modules
        COMMERCE[crates/rustok-commerce]
        CONTENT[crates/rustok-content]
        BLOG[crates/rustok-blog]
        FORUM[crates/rustok-forum]
        PAGES[crates/rustok-pages]
        INDEX[crates/rustok-index]
        RBAC[crates/rustok-rbac]
        TENANT[crates/rustok-tenant]
        ALLOY[crates/alloy-scripting]
        WORKFLOW[crates/rustok-workflow]
        MEDIA[crates/rustok-media]
    end

    subgraph Module UI Packages
        BLOG_UI_ADMIN[crates/rustok-blog/ui/admin]
        BLOG_UI_FRONT[crates/rustok-blog/ui/frontend]
        OPTIONAL_UI[crates/rustok-<module>/ui/* (target)]
    end

    subgraph Platform Core Crates
        CORE[crates/rustok-core]
        EVENTS[crates/rustok-events]
        STORAGE[crates/rustok-storage]
        OUTBOX[crates/rustok-outbox - Core Infra]
        CACHE[crates/rustok-cache]
        IGGY[crates/rustok-iggy]
        IGGY_CONN[crates/rustok-iggy-connector]
        MCP[crates/rustok-mcp]
        TELEMETRY[crates/rustok-telemetry]
        TEST_UTILS[crates/rustok-test-utils]
        FLEX[crates/flex]
    end

    subgraph Frontend Libraries - internal custom
        L_AUTH[crates/leptos-auth]
        L_UI[crates/leptos-ui]
        L_GRAPHQL[crates/leptos-graphql]
        L_FORMS[crates/leptos-forms]
        L_HOOKFORM[crates/leptos-hook-form]
        L_TABLE[crates/leptos-table]
        L_ZOD[crates/leptos-zod]
        L_ZUSTAND[crates/leptos-zustand]
        L_PAGINATION[crates/leptos-shadcn-pagination]
    end

    SERVER --> COMMERCE
    SERVER --> CONTENT
    SERVER --> RBAC
    SERVER --> TENANT
    SERVER --> INDEX
    SERVER --> BLOG
    SERVER --> FORUM
    SERVER --> PAGES
    SERVER --> ALLOY
    SERVER --> WORKFLOW

    ADMIN --> L_AUTH
    ADMIN --> L_UI
    ADMIN --> L_GRAPHQL
    ADMIN --> L_HOOKFORM
    ADMIN --> L_TABLE
    ADMIN --> L_PAGINATION
    SF --> L_UI
    SF --> L_GRAPHQL


    COMMERCE --> CORE
    COMMERCE --> EVENTS
    COMMERCE --> OUTBOX
    COMMERCE --> MEDIA
    CONTENT --> CORE
    CONTENT --> OUTBOX
    CONTENT --> MEDIA
    BLOG --> CONTENT
    FORUM --> CONTENT
    PAGES --> CORE
    INDEX --> CORE
    OUTBOX --> IGGY
    IGGY --> IGGY_CONN
    ALLOY --> CORE
    WORKFLOW --> CORE
    WORKFLOW --> ALLOY
    CACHE --> CORE
    STORAGE --> CORE
    MEDIA --> CORE
    MEDIA --> STORAGE
    MEDIA --> OUTBOX
    FLEX --> CORE

    BLOG --> BLOG_UI_ADMIN
    BLOG --> BLOG_UI_FRONT
    BLOG_UI_ADMIN --> NEXT_ADMIN
    BLOG_UI_FRONT --> NEXT_SF
    OPTIONAL_UI -. optional contracts .-> ADMIN
    OPTIONAL_UI -. optional contracts .-> NEXT_ADMIN
    OPTIONAL_UI -. optional contracts .-> SF
    OPTIONAL_UI -. optional contracts .-> NEXT_SF

    Domain Modules -.-> TELEMETRY
```

## Component Directory

### Applications (`apps/`)

| Path | Name | Description |
|------|------|-------------|
| `apps/server` | **Server** | Main API server built on Loco.rs. Orchestrates all domain modules. ([CRATE_API](../../apps/server/CRATE_API.md)) |
| `apps/admin` | **Leptos Admin** | Back-office management interface built with Leptos (CSR/WASM). ([CRATE_API](../../apps/admin/CRATE_API.md)) |
| `apps/storefront` | **Leptos Storefront** | Customer-facing web interface built with Leptos (SSR). ([CRATE_API](../../apps/storefront/CRATE_API.md)) |
| `apps/next-admin` | **Next.js Admin** | Modern React-based admin interface (Next.js). Primary admin dashboard. ([CRATE_API](../../apps/next-admin/CRATE_API.md)) |
| `apps/next-frontend` | **Next.js Storefront** | Modern React-based storefront (Next.js). ([CRATE_API](../../apps/next-frontend/CRATE_API.md)) |

### Core Platform Crates (`crates/`)

#### Модули-библиотеки уровня 0 (leaf — без зависимости от core)

Предоставляют чистые типы и трейты. Core зависит от них и ре-экспортирует.

| Path | Name | Description |
|------|------|-------------|
| `crates/rustok-events` | **Events Contracts** | Модуль-библиотека (leaf). Stable import point for `DomainEvent`/`EventEnvelope`. Core re-exports. ([CRATE_API](../../crates/rustok-events/CRATE_API.md)) |
| `crates/rustok-telemetry` | **Telemetry** | Модуль-библиотека (leaf). Observability setup (OTLP, Tracing, Prometheus metrics). Core re-exports. ([CRATE_API](../../crates/rustok-telemetry/CRATE_API.md)) |
| `crates/rustok-storage` | **Storage** | Модуль-библиотека (leaf). `StorageBackend` async trait + `LocalStorage` backend, `StorageService` wrapper, `StorageConfig`. ([docs](../../crates/rustok-storage/docs/README.md)) |

#### Модуль-агрегатор (уровень 1)

| Path | Name | Description |
|------|------|-------------|
| `crates/rustok-core` | **Core (critical)** | Модуль-библиотека (агрегатор). Re-exports leaf contracts, provides `CacheBackend`, `ModuleRegistry`, `RusToKModule`, RBAC primitives, i18n, `SecurityContext`, circuit breaker. Содержит **Flex** — набор типов, валидаторов и migration-хелперов для кастомных полей (`field_schema.rs`, `HasCustomFields`). ([CRATE_API](../../crates/rustok-core/CRATE_API.md)) |

> **Flex** — часть модуля `rustok-core`. Сейчас — набор типов и хелперов (модуль-библиотека). Данные живут внутри модуля-потребителя.
> Режим **Attached** (кастомные поля для сущностей) — в core. Режим **Standalone** (`flex`) — запланирован.
> План реализации: [`docs/architecture/flex.md`](../architecture/flex.md).

#### Инфраструктурные модули-библиотеки

| Path | Name | Description |
|------|------|-------------|
| `crates/rustok-cache` | **Cache** | Redis connection lifecycle, `CacheModule` + `CacheService`. `CacheBackend` (Moka + Redis + Fallback): circuit breaker, anti-stampede coalescing, negative cache, Redis pub/sub invalidation, metrics. ([docs](../../crates/rustok-cache/docs/README.md)) |
| `crates/rustok-outbox` | **Outbox (Core, critical)** | Модуль-библиотека. Core event delivery (`TransactionalEventBus`). Initialized via `build_event_runtime()`. ([CRATE_API](../../crates/rustok-outbox/CRATE_API.md)) |
| `crates/rustok-iggy` | **Iggy Transport** | L2 streaming `EventTransport` implementation with serialization, topology, DLQ, replay. ([CRATE_API](../../crates/rustok-iggy/CRATE_API.md)) |
| `crates/rustok-iggy-connector` | **Iggy Connector** | Embedded/Remote mode switching, connection lifecycle, message I/O. ([CRATE_API](../../crates/rustok-iggy-connector/CRATE_API.md)) |
| `crates/rustok-mcp` | **MCP** | MCP adapter crate with embedded `rustok-mcp-server` binary. Exposes RusToK tools/resources via the MCP protocol using the `rmcp` SDK. ([CRATE_API](../../crates/rustok-mcp/CRATE_API.md)) |
| `crates/rustok-test-utils` | **Test Utils** | Shared testing helpers and mocks. `[dev-dependencies]` only — never in production binary. ([CRATE_API](../../crates/rustok-test-utils/CRATE_API.md)) |

### Полноценные модули (`crates/`)

Реализуют `RusToKModule`, регистрируются через `ModuleRegistry` в `apps/server`.
Имеют таблицы, entities, бизнес-логику.

#### Core-модули (уровень 2 — `ModuleKind::Core`, нельзя отключить)

| Path | Name | Kind | Depends on |
|------|------|------|-----------|
| `crates/rustok-tenant` | **Tenant** | `Core` (mandatory, critical) | `rustok-core` ([CRATE_API](../../crates/rustok-tenant/CRATE_API.md)) |
| `crates/rustok-rbac` | **RBAC** | `Core` (mandatory, critical) | `rustok-core` ([CRATE_API](../../crates/rustok-rbac/CRATE_API.md)) |
| `crates/rustok-index` | **Index** | `Core` (mandatory, critical) | `rustok-core` ([CRATE_API](../../crates/rustok-index/CRATE_API.md)) |
| `crates/rustok-media` | **Media** | `Core` (mandatory, feature `mod-media`) | `rustok-core`, `rustok-storage`. `MediaService`: upload/get/list/delete + translations. SeaORM entities `media` + `media_translations`. ([docs](../../crates/rustok-media/docs/README.md)) |

#### Optional-модули (уровень 3 — `ModuleKind::Optional`, toggle per-tenant)

| Path | Name | Kind | Depends on |
|------|------|------|-----------|
| `crates/rustok-content` | **Content** | `Optional` | `rustok-core`, `rustok-media` ([CRATE_API](../../crates/rustok-content/CRATE_API.md)) |
| `crates/rustok-commerce` | **Commerce** | `Optional` | `rustok-core`, `rustok-media` ([CRATE_API](../../crates/rustok-commerce/CRATE_API.md)) |
| `crates/rustok-blog` | **Blog** | `Optional` | `rustok-content` ([CRATE_API](../../crates/rustok-blog/CRATE_API.md)) |
| `crates/rustok-forum` | **Forum** | `Optional` | `rustok-content` ([CRATE_API](../../crates/rustok-forum/CRATE_API.md)) |
| `crates/rustok-pages` | **Pages** | `Optional` | `rustok-core` ([CRATE_API](../../crates/rustok-pages/CRATE_API.md)) |
| `crates/alloy-scripting` | **Alloy Scripting** | `Optional` | `rustok-core` (registered via `AlloyModule` in `apps/server/src/modules/alloy.rs`) |
| `crates/rustok-workflow` | **Workflow** | `Optional` | `rustok-core`, `alloy-scripting`. Визуальная автоматизация на платформенной очереди. Горизонтальный модуль. ([docs](../../crates/rustok-workflow/docs/README.md) · [CRATE_API](../../crates/rustok-workflow/CRATE_API.md) · [arch](../architecture/workflow.md)) |

> **4-уровневая архитектура платформы:**
> - Уровень 0 (модули-библиотеки, leaf): `rustok-events`, `rustok-telemetry`, `rustok-storage`
> - Уровень 1 (модуль-агрегатор): `rustok-core` (зависит от leaf, ре-экспортирует их)
> - Уровень 2 (полноценные Core-модули, всегда активны): `rustok-tenant`, `rustok-rbac`, `rustok-index`, `rustok-media`
> - Уровень 3 (полноценные Optional-модули, toggle per-tenant): `content`, `commerce`, `blog`, `forum`, `pages`, `alloy-scripting`, `workflow`
>
> Обязательный базис платформы: `rustok-core`, `rustok-outbox`, `rustok-telemetry`, `rustok-tenant`, `rustok-rbac`, `rustok-index` + инфраструктурные модули (`rustok-cache`, `rustok-events`).
>
> **Граница подвижна:** модуль-библиотека может получить таблицы и стать полноценным модулем.

### Module UI Packages Layer (`crates/*/ui/*`)

This layer contains UI packages shipped by domain modules.
For `ModuleKind::Optional`, UI composition must come from module-owned UI packages (screens, nav items, guards, editors) instead of hardcoded app-level features.

Core exceptions: `index`, `tenant`, `rbac`, and platform core crates (`rustok-core`, `rustok-outbox`, `rustok-telemetry`) are not required to follow this UI packaging pattern.

Recommended package shape:

- `crates/rustok-<module>/ui/admin-next`
- `crates/rustok-<module>/ui/admin-leptos`
- `crates/rustok-<module>/ui/frontend-next`
- `crates/rustok-<module>/ui/frontend-leptos`

Recommended entry-point exports:

- `adminNavItems` (or equivalent admin contract; implemented per runtime: Next/Leptos)
- `frontendNavItems` (or equivalent storefront contract; implemented per runtime: Next/Leptos)

Admin and storefront runtimes (`apps/admin`, `apps/next-admin`, `apps/storefront`, `apps/next-frontend`) should consume these packages through one modular contract/registry layer (e.g., `registerAdminModule` / `registerStorefrontModule` and Leptos registry equivalents).

| Path | Module | UI Scope | Status |
|------|--------|----------|--------|
| `crates/rustok-blog/ui/admin` | Blog (+forum composition currently colocated) | Admin (Next) | Existing (reference sample for Next) |
| `crates/rustok-blog/ui/frontend` | Blog | Frontend (Next) | Existing (reference sample for Next) |
| `crates/rustok-<module>/ui/*` | Content/Commerce/Forum/Pages/Alloy scripting | Admin + Frontend | Planned / partial |

Current reference sample in repository covers Next runtime: `crates/rustok-blog/ui/admin` and `crates/rustok-blog/ui/frontend`; Leptos-specific package pair remains a TODO for full dual-stack parity.

### Internal Frontend Libraries (`crates/`)

All `leptos-*` and `tailwind-*` crates are **internal custom libraries** written and maintained by the RusToK team.
They are not published to crates.io. Treat them as first-party code — changes here affect all consuming apps.

| Path | Name | Used by | Description |
|------|------|---------|-------------|
| `crates/leptos-ui` | **Leptos UI** | `apps/admin`, `apps/storefront` | Shared UI component library. |
| `crates/leptos-auth` | **Leptos Auth** | `apps/admin` | Authentication hooks and components. |
| `crates/leptos-forms` | **Leptos Forms** | `apps/admin` | Low-level form abstractions. |
| `crates/leptos-graphql` | **Leptos GraphQL** | `apps/admin`, `apps/storefront` | Thin GraphQL client wrapper (request shape, headers, error mapping). |
| `crates/leptos-hook-form` | **Leptos Hook Form** | `apps/admin` | Hook-form style bindings (React-hook-form inspired). |
| `crates/leptos-shadcn-pagination` | **Leptos Pagination** | `apps/admin` | Pagination components compatible with shadcn design. |
| `crates/leptos-table` | **Leptos Table** | `apps/admin` | Data table primitives. |
| `crates/leptos-zod` | **Leptos Zod** | `apps/admin` | Zod-like validation helpers. |
| `crates/leptos-zustand` | **Leptos Zustand** | `apps/admin` | Lightweight state management utilities. |
| `crates/utoipa-swagger-ui-vendored` | **Swagger UI** | `apps/server` | Vendored Swagger UI static assets. |
| `crates/rustok-cache` | **Cache** | `apps/server` | Redis/Moka cache backends, `CacheModule`, `CacheService`. Выделен из `rustok-core`. |
| `crates/rustok-storage` | **Storage** | `rustok-media`, `apps/server` | Leaf crate: `StorageBackend` async trait + `LocalStorage` impl, `StorageService` wrapper. ([docs](../../crates/rustok-storage/docs/README.md)) |
| `crates/rustok-media` | **Media** | `apps/server`, `rustok-content`, `rustok-commerce` | Core module: `MediaService`, SeaORM entities `media` + `media_translations`. Feature: `mod-media`. ([docs](../../crates/rustok-media/docs/README.md)) |
| `crates/flex` | **Flex Contracts** *(Phase 4.5, in progress)* | `apps/server` (Attached mode) | Optional crate для выноса Flex attached-mode контрактов (`FieldDefinitionService`, `FieldDefRegistry`, DTOs). Standalone режим планируется следующим этапом. |

## Maintenance Rule

> [!IMPORTANT]
> This registry must be kept up to date. AI Agents are required to update the **Mermaid diagram** and **Component Directory** whenever a new crate or application is added, renamed, or significantly restructured.
> Also update [`docs/index.md`](../index.md) when this registry changes.
>
> Перед изменением любого `crates/rustok-*` необходимо проверить и обновить соответствующий `CRATE_API.md`, если изменился публичный контракт (модули, сигнатуры, события, зависимости).
