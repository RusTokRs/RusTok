# Next Admin Documentation

> **MANDATORY FOR AI AGENTS — Read these guides BEFORE any code changes:**
>
> **FSD Architecture:** This app follows **Feature-Sliced Design** (FSD) layers: `app`, `shared`, `entities`, `features`, `widgets`.
>
> **Module Ownership:** Module business UI must NOT be placed in `apps/next-admin/src/`. It belongs in `apps/next-admin/packages/*` or `@rustok/*-admin` packages.
>
> **IMPORTANT RULES:**
> - **DO NOT write custom components** — check existing components in `src/shared/ui` and `packages/*` first
> - **DO NOT invent custom i18n** — use host-provided `x-rustok-effective-locale` and `next-intl`
> - **DO NOT duplicate transport/auth** — use shared contracts in `src/shared/api` and `src/shared/lib`
> - **DO NOT create starter-only routes** — `billing`, `exclusive`, `workspaces` must return `notFound()`
>
> **Related Guides for Rust Module UI Packages:**
> - [FFA Architecture Guide](../../../docs/UI/module-package-architecture.md) — explains **FFA** (Fluid Frontend Architecture) for Rust module packages
> - [Implementation Guide](../../../docs/UI/module-package-implementation.md) — internal libraries, i18n, file structure for Rust packages

Local documentation for `apps/next-admin`.

## Purpose

`apps/next-admin` is the Next.js admin host for RusToK. It provides the React/Next path for the admin panel, works in parallel with `apps/admin`, and mounts module-owned/admin-owned packages instead of moving module UI inside the host.

FFA classification: `apps/next-admin` is an `FFA-compatible composition host`, not a module-owned UI package. Its FFA responsibility is to maintain Next shell/routing/context composition and parity with Leptos admin without moving module-specific workflows into the host.

## Boundaries of Responsibility

- own the Next.js admin host, routing and shared integration layer;
- mount package-owned admin surfaces from `packages/*`;
- use canonical frontend contracts for auth, GraphQL, forms and shared UI;
- maintain parity with `apps/admin` at the platform contract level;
- keep admin shell navigation in parity with Leptos Admin: `Overview`, `Management`,
  `Module Plugins`, `Account`, with module-owned items remaining registry-driven
  and filtered by enabled module slug;
- do not pull module-owned business UI into host code.

## Runtime contract

- canonical FSD layers for host: `app`, `shared`, `entities`, `features`, `widgets`;
- backend integration goes through `apps/server` and shared transport packages;
- effective locale is selected by the host/runtime layer via `x-rustok-effective-locale`
  and `next-intl`; module-owned packages read the host-provided locale, not a cookie/query fallback chain;
- user language selection in Next Admin is stored in the host-owned cookie `rustok-admin-locale`;
  middleware normalizes the effective locale in order `?locale` → cookie → `x-rustok-effective-locale`
  → `Accept-Language` → `en`, and the UI uses dropdown in header and auth screens;
- global admin search uses `rustok-search` as a host-level capability;
- shared SEO operator/headless contract must also go through the backend surface:
  registry-backed target descriptors are read from GraphQL `seoTargets`, not from host-local slug mapping;
- legacy import paths are allowed only as a temporary compatibility layer;
- new code must go through canonical FSD paths and shared package boundaries.

## Ownership contract for module UI

- If a module supplies admin UI, it remains a module-owned package alongside the module or in `packages/*`.
- Host `apps/next-admin` acts only as the composition root.
- Core navigation `apps/next-admin` must not contain module-owned business routes. Each module or capability connects its Next UX through `apps/next-admin/packages/*` / `@rustok/*-admin` entrypoint, and the shell filters items by enabled module slug.
- If a tenant has only `blog` enabled, ecommerce/catalog/product UX must not appear in navigation and must not live as a host-owned starter page.
- Starter-only routes `billing`, `exclusive`, `workspaces` and `workspaces/team` are not public RusTok admin surfaces and must return `notFound()`, not a placeholder UI.
- The same rule applies for core-modules, optional-modules and capability packages.
- Capability-owned surface `rustok-ai` is mounted as a package-owned UI, not as an ad-hoc host feature.
- Route-selection contract must be in parity with `apps/admin`: selection state URL-owned,
  only typed `snake_case` query keys are used, invalid keys do not fallback to the first item,
  and local Next helpers do not invent a separate schema/policy on top of the `rustok-api` contract.

## Packages and Integrations

- shared UI and frontend contracts go through `UI/next` and internal transport/auth packages;
- backend — `apps/server`;
- module-owned Next admin packages live in `apps/next-admin/packages/*`;
- shared API helper `src/shared/api/seo.ts` provides typed access to SEO control-plane: `seoTargets`, diagnostics, sitemap status/jobs, bulk jobs and job detail with REST primary (rollout-gated) + GraphQL secondary path strategy;
- semantic SEO error taxonomy (`BAD_USER_INPUT`, `PERMISSION_DENIED`, `NOT_FOUND`, transport failures) is considered canonical for Next hosts and is reused not only in `next-admin`, but also in Next storefront SEO runtime adapters;
- package naming contract for module-owned admin UI remains `@rustok/*-admin`.

## SEO operator readiness evidence

- Next Admin participates in SEO D8/D9 as the operator host for diagnostics, sitemap/bulk read surfaces and index repair/replay controls.
- Compile-free documentation evidence is tracked from the Next storefront SEO fixture docs sync matrix; live sign-off still requires a running backend sample for semantic error mapping and repair/replay telemetry.
- The required owner sign-off evidence is: REST/GraphQL error-code parity, bounded replay input validation, before/after index delivery counters and no duplicate idempotency transition.

## Interactions

- `apps/server` provides the API/runtime contract;
- `apps/admin` remains the primary Leptos admin stack and the reference for parity;
- module-owned UI packages are connected as external surfaces, not as host-owned business code.

## Verification

- typecheck/lint runs on `apps/next-admin`
- targeted checks on package-owned admin surfaces
- cross-reference shared contract with `docs/UI/*` and `docs/modules/manifest.md`

## Related Documents

- [Implementation Plan](./implementation-plan.md)
- [Navigation RBAC](./nav-rbac.md)
- [Deprecated Clerk starter reference](./clerk_setup.md) — not an active RusTok auth contract.
- [Themes](./themes.md)
- [Documentation Map](../../../docs/index.md)
