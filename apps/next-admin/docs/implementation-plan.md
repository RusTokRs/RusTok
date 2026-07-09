# Next Admin Implementation Plan

## Current Contract

`apps/next-admin` is an FFA-compatible Next.js composition host. It owns the
Next App Router shell, shared runtime context, host navigation and package
registration. Module and capability UI lives in package-owned surfaces under
`apps/next-admin/packages/*` or external `@rustok/*-admin` packages.

The live host structure is:

- `src/app/` for Next routes and layouts;
- `src/shared/` for shared API, auth, i18n, UI and utility contracts;
- `src/entities/` for host-local read models;
- `src/widgets/` for composite shell UI;
- `src/features/` only for host-owned composition and platform screens;
- `packages/*` for module-owned Next admin surfaces.

Routes and navigation must import package entrypoints for module UI. Package
entrypoints must not re-export `src/features/*` implementations.

## Active Work

- Keep module navigation registry-driven and filtered by enabled module slug.
- Keep locale selection host-owned through `x-rustok-effective-locale` and
  `next-intl`.
- Keep GraphQL/REST access centralized in `src/shared/api` and package-owned API
  modules instead of page-local clients.
- Keep typed `snake_case` URL query keys aligned with the Leptos admin host.
- Keep starter-only routes (`billing`, `exclusive`, `workspaces`,
  `workspaces/team`) returning `notFound()`.

## Open Improvement Areas

- Add focused package boundary checks for `packages/*` entrypoints and route
  imports.
- Expand contract tests for API mapping and typed client validation.
- Align loading, empty, error and permission-gated states with `apps/admin`.
- Add client telemetry events and correlation-id propagation for critical admin
  flows.
- Strengthen route/action RBAC guard coverage.

## Verification

For Next admin host/package changes, run:

```powershell
npm.cmd run typecheck
npm.cmd run lint
npm.cmd run verify:i18n:ui
npm.cmd run verify:i18n:contract
git diff --check
```

When touching package-owned module surfaces, also verify the matching backend or
Leptos parity contract where applicable.
