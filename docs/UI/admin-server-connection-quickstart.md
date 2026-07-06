---
id: doc://docs/UI/admin-server-connection-quickstart.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Admin ↔ Server: quickstart

This document captures the minimum runtime contract between UI host applications and `apps/server`. It does not replace full deployment runbooks or duplicate instructions for specific environments.

## Basic Scheme

Recommended base path for UI hosts:

- browser opens the host application;
- UI communicates with `apps/server`;
- backend publishes `/api/graphql`, `/api/fn/*`, `/api/auth/*` and related runtime surfaces;
- reverse proxy or host runtime hides unnecessary cross-origin complexity where possible.

## Leptos Admin Profiles

`apps/admin` separates transport by runtime profile. The production target for Leptos admin is SSR-first monolith/hydrate, while standalone CSR is needed for local debug and compatibility checking of module-owned UI packages:

- `csr`: standalone Trunk/WASM host. Critical paths go directly to `apps/server` via `/api/graphql`,
  `/api/auth/*` and REST. `/api/fn/*` is not required for basic shell/debug and must not be the sole transport.
- `hydrate`: browser half for SSR/monolith. UI may call `#[server]` because the backend origin
  must serve `/api/fn/*`.
- `ssr`: server half or monolith. `#[server]` is available as a native transport and may be the preferred path
  for server-side surfaces.

Rule: `#[server]` does not replace GraphQL/REST. If a surface is needed in standalone `csr`, it must have a
working GraphQL/REST path or an explicitly documented fallback.

## Preferred Local/Dev Path

For local debugging, a same-origin or proxy-aware mode is preferred, where the UI and backend appear as a single origin to the browser. This reduces CORS errors, simplifies auth/session flows, and makes the transport contract predictable.

Minimum that must be available:

- UI host;
- `apps/server`;
- a working auth path;
- a working GraphQL path;
- if the host is Leptos in `ssr`/`hydrate` profile — a working `#[server]` path;
- if the host is Leptos in standalone `csr` debug profile — a working GraphQL/REST fallback without mandatory `/api/fn/*`.

## Minimum Runtime Contract

The UI host must be able to reach:

- `/api/graphql`
- `/api/auth/*`
- `/api/fn/*` for Leptos `ssr`/`hydrate` hosts
- health/runtime surfaces for operator-level diagnostics

If the UI and backend are on different origins, the backend must explicitly support the required CORS and auth contract. If this is not needed, a same-origin scheme remains preferred.

## What to Verify After Connection

Minimum smoke:

1. The login surface of the host application opens.
2. Login and loading of the current user/session work.
3. Requests to `/api/auth/*` succeed.
4. A request to `/api/graphql` succeeds.
5. For Leptos `ssr`/`hydrate` hosts, if a native path is involved, calls to `/api/fn/*` succeed.
6. For Leptos standalone `csr`, if a module-owned UI package is involved, the same screen works via GraphQL/REST fallback.

If these steps pass, the host ↔ server contract is correctly set up.

## Route-Selection Contract for Admin Hosts

For module-owned admin surfaces, the runtime contract includes not only transport but also routing:

1. selection state is stored in the URL;
2. module-owned admin UI reads it via the host route context;
3. a valid user-driven select/open writes a canonical typed `snake_case` key back to the query;
4. reset/delete/archive/close clears the corresponding key;
5. an invalid or deleted entity id produces an empty state and does not leave stale detail/form state.

For the Leptos host, this contract goes through `UiRouteContext` + host-provided policy for
`leptos-ui-routing`. For `apps/next-admin`, the same schema-level contract applies via local
Next helpers. Legacy keys like `id`, `pageId`, `topicId` are not supported.

## Diagnostics

### `401 Unauthorized`

Check:

- auth token or session transport;
- tenant/channel headers, if required for the specific scenario;
- whether the backend-side auth/runtime contract is broken.

### CORS Errors

This usually means the UI and backend are working cross-origin without the required backend configuration. The preferred fix is a same-origin/proxy path, not growing ad hoc exceptions.

### `404` on `/api/graphql` or `/api/fn/*`

Check:

- that the reverse proxy actually forwards `/api/*`;
- that `apps/server` is running on the expected port;
- that the selected UI host uses the correct transport contract for the current runtime mode.

## Local Debug Stack Without Docker

For local debugging without `docker compose`, the minimum stack is brought up as separate processes:

```powershell
# 1. apps/server
$env:RUSTOK_MODULES_MANIFEST = (Resolve-Path .\modules.local.toml)
target\debug\rustok-server.exe start --no-banner --binding localhost --port 5150
Invoke-WebRequest http://localhost:5150/health/live -UseBasicParsing

# 2. apps/next-admin
cd apps\next-admin
npm.cmd run dev -- --hostname localhost --port 3000 --webpack

# 3. apps/admin
cd ..\admin
trunk serve --address ::1 --port 3001
```

For local debug without Docker, the server must read `modules.local.toml`, where embedded admin/storefront are disabled.
The root `modules.toml` describes the monolith/release composition and requires `embed-admin`/`embed-storefront`.
In the current Windows debug environment, building `apps/admin` as an SSR embedded artifact runs out of memory (`rustc-LLVM ERROR: out of memory`),
so the external stack `apps/server -> apps/next-admin -> apps/admin` is launched via `modules.local.toml`.
In `apps/server/config/development.yaml`, for this debug profile only maintenance workers are disabled:
`runtime.background_workers.workflow_cron_enabled=false` and `runtime.background_workers.seo_bulk_enabled=false`.
This preserves the full HTTP/GraphQL/module surface for admin panels but prevents cron/bulk loops from consuming the DB pool during
interactive debugging. The production/default runtime keeps workers enabled.

Tenant contract for standalone admin hosts is slug-based: the UI sends `X-Tenant-Slug`, and the backend in header-mode must accept this
header as a public admin contract. `X-Tenant-ID` remains a valid internal/legacy header but must not be required from the UI host.

Binding resolution: the canonical URL remains `http://localhost:5150`. On this Windows machine, `127.0.0.1`
causes HTTP response hangs even for a simple Node server, while `localhost` resolves to `::1` and works stably.
Therefore the local debug stack must use `localhost`/`::1`, not `127.0.0.1`.

Next admin resolution: with Next.js 16, local `next dev` on Turbopack hung during compilation of
`/auth/sign-in`, so the debug command uses `--webpack`. This is a startup/debug choice, not a change to the
public API.

Leptos admin resolution: standalone `csr` profile is needed for debug and headless parity, but the production target
remains SSR-first/hydrate. `#[server]` remains the preferred internal path in SSR/monolith, GraphQL/REST remains
a mandatory fallback for headless/CSR. `trunk serve` must build a binary artifact `rustok-admin`, because a
library artifact `rustok_admin` does not run `main()` and does not mount the shell.

Visual contract: Leptos admin and Next admin should not diverge as independent products. Host-level auth shell,
navigation, route-selection UX, and module-owned surface containers must follow a common admin UI contract/tokens.
Next admin may remain a React/Next host for Next packages, but Leptos admin is the canonical operator surface for
the monolithic/SSR path; divergences are recorded as parity debt, not as an acceptable design fork.

For standalone `trunk serve`, CSS is part of the startup contract. `apps/admin/input.css` uses Tailwind v4
`@import "tailwindcss"` + `@source`, and `tailwind.config.js` must scan not only `apps/admin/src`, but also
module-owned admin UI packages in `crates/**/admin/src/**/*.rs`, as well as shared Leptos UI crates. Otherwise, the host shell may
load, but module-owned pages will lack spacing/layout utilities and visually diverge from Next admin.
The Trunk post-build hook `scripts\tailwind-build.cmd` places `output.css` in staging/dist; missing `dist/output.css`
is considered a startup blocker for Leptos admin debug.

## Scope of This Quickstart

This document intentionally does not contain:

- lengthy instructions for Docker Compose, VPS, Kubernetes, or PaaS;
- install scripts and bootstrap runbooks;
- detailed production rollout steps.

Such instructions should live in separate deployment/runbook documents, while this document keeps only the live UI ↔ server contract.

## Related Documents

- [UI index](./README.md)
- [GraphQL and Leptos server functions](./graphql-architecture.md)
- [`apps/admin` Documentation](../../apps/admin/docs/README.md)
- [`apps/server` Documentation](../../apps/server/docs/README.md)
- [ADR: SSR-first Leptos hosts with headless parity](../../DECISIONS/2026-04-24-ssr-first-leptos-hosts-with-headless-parity.md)
- [Health and Runtime Guardrails](../../apps/server/docs/health.md)
- [Documentation Map](../index.md)
