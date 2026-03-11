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
        MCP_APP[apps/mcp - MCP stdio server]
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
    end

    subgraph Platform Core Crates
        CORE[crates/rustok-core]
        EVENTS[crates/rustok-events]
        OUTBOX[crates/rustok-outbox - Core Infra]
        IGGY[crates/rustok-iggy]
        IGGY_CONN[crates/rustok-iggy-connector]
        MCP[crates/rustok-mcp]
        TELEMETRY[crates/rustok-telemetry]
        TEST_UTILS[crates/rustok-test-utils]
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

    ADMIN --> L_AUTH
    ADMIN --> L_UI
    ADMIN --> L_GRAPHQL
    ADMIN --> L_HOOKFORM
    ADMIN --> L_TABLE
    ADMIN --> L_PAGINATION
    SF --> L_UI
    SF --> L_GRAPHQL

    MCP_APP --> MCP

    COMMERCE --> CORE
    COMMERCE --> EVENTS
    COMMERCE --> OUTBOX
    CONTENT --> CORE
    CONTENT --> OUTBOX
    BLOG --> CONTENT
    FORUM --> CONTENT
    PAGES --> CORE
    INDEX --> CORE
    OUTBOX --> IGGY
    IGGY --> IGGY_CONN
    ALLOY --> CORE

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
| `apps/mcp` | **MCP App** | MCP stdio application runtime for AI/agent integrations, built on top of `crates/rustok-mcp`. ([README](../../apps/mcp/README.md)) |

### Core Platform Crates (`crates/`)

| Path | Name | Description |
|------|------|-------------|
| `crates/rustok-core` | **Core (critical)** | Shared traits, base entities, events, cache abstractions, circuit breaker, RBAC primitives. Not a `RusToKModule`. ([CRATE_API](../../crates/rustok-core/CRATE_API.md)) |
| `crates/rustok-events` | **Events Contracts** | Stable import point for `DomainEvent`/`EventEnvelope` (re-exports from core). ([CRATE_API](../../crates/rustok-events/CRATE_API.md)) |
| `crates/rustok-outbox` | **Outbox (Core, critical)** | Core event delivery (`TransactionalEventBus`). Not a `RusToKModule` — initialized via `build_event_runtime()`. ([CRATE_API](../../crates/rustok-outbox/CRATE_API.md)) |
| `crates/rustok-iggy` | **Iggy Transport** | L2 streaming `EventTransport` implementation with serialization, topology, DLQ, replay. ([CRATE_API](../../crates/rustok-iggy/CRATE_API.md)) |
| `crates/rustok-iggy-connector` | **Iggy Connector** | Embedded/Remote mode switching, connection lifecycle, message I/O. ([CRATE_API](../../crates/rustok-iggy-connector/CRATE_API.md)) |
| `crates/rustok-mcp` | **MCP** | MCP adapter crate with embedded `rustok-mcp-server` binary. Exposes RusToK tools/resources via the MCP protocol using the `rmcp` SDK. ([CRATE_API](../../crates/rustok-mcp/CRATE_API.md)) |
| `crates/rustok-telemetry` | **Telemetry (Core, critical)** | Observability setup (OTLP, Tracing, Prometheus metrics). Mandatory core crate, not a `RusToKModule`. ([CRATE_API](../../crates/rustok-telemetry/CRATE_API.md)) |
| `crates/rustok-tenant` | **Tenant** | Multi-tenancy isolation and management logic. Registered as `ModuleKind::Core`. ([CRATE_API](../../crates/rustok-tenant/CRATE_API.md)) |
| `crates/rustok-rbac` | **RBAC** | Role-based access control engine. Registered as `ModuleKind::Core`. ([CRATE_API](../../crates/rustok-rbac/CRATE_API.md)) |
| `crates/rustok-test-utils` | **Test Utils** | Shared testing helpers and mocks. `[dev-dependencies]` only — never in production binary. ([CRATE_API](../../crates/rustok-test-utils/CRATE_API.md)) |

### Domain Modules (`crates/`)

These implement `RusToKModule` and are registered via `ModuleRegistry` in `apps/server`.
Core modules are mandatory for the platform runtime; optional modules are additive domain capabilities.
The core baseline includes `ModuleKind::Core` modules and additional mandatory core crates.

| Path | Name | Kind | Depends on |
|------|------|------|-----------|
| `crates/rustok-index` | **Index** | `Core` (mandatory, critical) | `rustok-core` ([CRATE_API](../../crates/rustok-index/CRATE_API.md)) |
| `crates/rustok-tenant` | **Tenant** | `Core` (mandatory, critical) | `rustok-core` ([CRATE_API](../../crates/rustok-tenant/CRATE_API.md)) |
| `crates/rustok-rbac` | **RBAC** | `Core` (mandatory, critical) | `rustok-core` ([CRATE_API](../../crates/rustok-rbac/CRATE_API.md)) |
| `crates/rustok-content` | **Content** | `Optional` | `rustok-core` ([CRATE_API](../../crates/rustok-content/CRATE_API.md)) |
| `crates/rustok-commerce` | **Commerce** | `Optional` | `rustok-core` ([CRATE_API](../../crates/rustok-commerce/CRATE_API.md)) |
| `crates/rustok-blog` | **Blog** | `Optional` | `rustok-content` ([CRATE_API](../../crates/rustok-blog/CRATE_API.md)) |
| `crates/rustok-forum` | **Forum** | `Optional` | `rustok-content` ([CRATE_API](../../crates/rustok-forum/CRATE_API.md)) |
| `crates/rustok-pages` | **Pages** | `Optional` | `rustok-core` ([CRATE_API](../../crates/rustok-pages/CRATE_API.md)) |
| `crates/alloy-scripting` | **Alloy Scripting** | `Optional` | `rustok-core` (registered via `AlloyModule` in `apps/server/src/modules/alloy.rs`) |


> Mandatory core modules (platform baseline, all critical): `rustok-index`, `rustok-tenant`, `rustok-rbac`, `rustok-core`, `rustok-outbox`, `rustok-telemetry`.

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
| `crates/tailwind-rs` | **Tailwind RS** | Build-time | Tailwind CSS utility generation core. |
| `crates/tailwind-css` | **Tailwind CSS** | Build-time | CSS value and property types for Tailwind. |
| `crates/tailwind-ast` | **Tailwind AST** | Build-time | AST and parser for Tailwind class expressions. |

## Maintenance Rule

> [!IMPORTANT]
> This registry must be kept up to date. AI Agents are required to update the **Mermaid diagram** and **Component Directory** whenever a new crate or application is added, renamed, or significantly restructured.
> Also update [`docs/index.md`](../index.md) when this registry changes.
>
> Перед изменением любого `crates/rustok-*` необходимо проверить и обновить соответствующий `CRATE_API.md`, если изменился публичный контракт (модули, сигнатуры, события, зависимости).
