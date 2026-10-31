# apps/next-admin / CRATE_API

## Purpose

`rustok-next-admin` (`apps/next-admin`) is the Next.js administration dashboard application host and composition root for RusToK. It provides the React/Next.js-based operator interface in parallel with `apps/admin` (Leptos), hosting module-owned Next.js admin packages (`packages/*`), dynamic sidebar navigation, RBAC permission filters, multi-tenant and multi-locale controls, and GraphQL/REST API integrations.

## Architecture & Responsibilities

- **Host & Composition Root:** Provides App Router layouts, global providers (Apollo Client, ThemeProvider, Fluent i18n, Toast/Dialog providers), and dynamic module registration.
- **Module Isolation & Navigation:** Modules register their admin navigation entries and pages through `packages/*` (`@rustok/*-admin`). The host shell filters visible routes and navigation groups based on the tenant's enabled module list.
- **Fluid Frontend Architecture (FFA) Alignment:** Implements Feature-Sliced Design (FSD) layers (`app`, `shared`, `entities`, `features`, `widgets`), ensuring zero framework leakage into shared contracts and complete parity with Leptos admin view-models and route queries.
- **Transport & Security:** Interacts with `apps/server` using Apollo Client over `/api/graphql` and native fetch over `/api/*`, enforcing Bearer token authentication, CSRF headers, and strict tenant slug headers.

## Public Surface

- `app`: Next.js App Router entrypoints (`/[locale]/dashboard/...`, `/login`, `/register`).
- `packages/*`: Module-owned admin packages (`@rustok/blog-admin`, `@rustok/commerce-admin`, `@rustok/events-admin`, `@rustok/forum-admin`, `@rustok/pages-admin`, `@rustok/product-admin`, etc.).
- `shared/api`: Backend API client adapters, module catalog resolvers, and typed GraphQL client queries.
- `shared/ui`: Reusable design system primitives following the canonical RusToK UI specification (Button, Input, Badge, Dialog, Table, Form).
- `messages`: Project Fluent localization bundles (`en.ftl`, `ru.ftl`).

## Invariants & Guarantees

1. **Dual-Implementation Parity:** Route structure (`/dashboard/[module]`), capability workflows (moderation, taxonomy, status filters, action buttons), and query parameters maintain parity with `apps/admin` (Leptos).
2. **Fail-Closed Module Filtering:** Disabled or unassigned tenant modules are hidden from navigation and reject direct route access with `notFound()`.
3. **Exact Decimal Monetary Math:** Currency calculations and discount values use exact string/decimal arithmetic without floating-point cent conversions or scientific notation.
4. **Idempotency & Journal Safety:** Write operations track client attempt tokens and journal state machines to prevent accidental duplicate mutations during network retries.

## Dependencies & Boundaries

- **Upstream Backend Transports:** Connects exclusively to `apps/server` via GraphQL (`/api/graphql`) and REST (`/api/*`).
- **Internal Packages:** Imports module UI from `packages/*` (`@rustok/*-admin`). Never reads another module's internal database or private types.
