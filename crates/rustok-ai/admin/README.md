# rustok-ai-admin

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../docs/UI/module-package-verification.md)

Large Leptos operator/admin UI package for the `rustok-ai` capability crate.

## Responsibilities

- Exposes the large AI operator/admin surface used by `apps/admin`.
- Stays capability-owned: AI business UI does not live in `apps/admin`.
- Ships package-owned `admin/locales/en.json` and `admin/locales/ru.json` bundles for visible UI
  chrome, diagnostics, operator chat, approval flows, and session history surfaces.
- Owns the provider profile, tool policy, chat session, trace, and approval flows for the AI
  control plane.
- Renders the owner-provided agent catalog, workflows, principals, and model assignments as a
  read-only control-plane view. Agent role assignment remains a platform RBAC concern.
- Shows live incremental assistant output for active sessions through the shared GraphQL WebSocket
  subscription `aiSessionEvents`.
- Shows bounded recent stream-event history in diagnostics and session detail through the
  parallel `aiRecentRunStreamEvents` query surface.
- Shows bounded recent persisted run history in diagnostics through the parallel `aiRecentRuns`
  query surface.
- Uses build-profile-selected native Leptos `#[server]` functions through `HostRuntimeContext` while keeping GraphQL in parallel.
- Reads the effective admin locale from `UiRouteContext.locale` and does not invent a separate
  package-local fallback chain.
- Does not use `rustok-module.toml`: unlike module-owned packages, `rustok-ai-admin` is a
  capability-owned operator/admin surface that still follows the same host locale contract.

## Entry Points

- `AiAdmin` — root admin page component for the AI control plane.
- Host routes:
  - `/ai` — overview/control-plane surface
  - `/ai/diagnostics` — focused diagnostics surface for router/run observability

## Interactions

- Consumed by `apps/admin` as a host/composition-root dependency.
- Talks to `apps/server` through `rustok-ai` server functions that receive DB/event/registry/storage/runtime handles from the host runtime context, plus the parallel GraphQL contract.
- Uses the shared `/api/graphql/ws` transport for live session streaming while keeping server
  functions as the monolith/hydrate selected path.
- Depends on `rustok-ai` for typed runtime/service contracts and on `rustok-mcp` indirectly through
  the server-side orchestration path.

## Documentation

- See [platform docs](../../../docs/index.md).
