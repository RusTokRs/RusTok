---
id: doc://docs/UI/module-package-architecture.md
kind: project_overview
language: markdown
status: active
---

# Module UI Package Architecture

Read this document when **designing a new UI package** or **changing the structure** of an
existing one. It explains *why* the architecture is the way it is.

For the concrete file layout and code rules, see
[Module UI Package Implementation](./module-package-implementation.md).
For verification commands, see [Module UI Package Verification](./module-package-verification.md).

---

## What Is FFA

**FFA — Fluid Frontend Architecture** is the RusTok-specific model that allows the same UI
code to run as an embedded monolith (SSR/hydrate inside `apps/server`) and as a standalone
headless client (Trunk/CSR, Next.js, mobile) without rewriting the UI layer.

Full definition: [`docs/research/fluid-frontend-architecture.md`](../research/fluid-frontend-architecture.md)

The core claim: **deployment topology changes the transport and packaging, but never the
components, routes, or state logic.** If switching from monolith to headless requires
rewriting UI, the package is not FFA-compliant.

---

## The Three-Layer Split

Every module-owned Leptos UI package is decomposed into three layers:

```
core              — framework-agnostic domain logic
transport/        — transport adapters (native server functions + GraphQL)
ui/leptos         — thin Leptos render/bind adapter
```

### Why three layers?

**`core` is framework-agnostic** so that when the platform moves from Leptos to Dioxus,
only `ui/leptos.rs` needs to be replaced with `ui/dioxus.rs`. View-models, state transitions,
validation, and display policy are reused unchanged across frameworks and across transport
profiles.

**`transport/` hides adapter selection** so that `ui/leptos.rs` calls
`transport::fetch_something()` without knowing whether the current runtime is SSR/hydrate
(uses native `#[server]`) or CSR/headless (uses GraphQL). In small packages this may be a
single `transport.rs`; in larger packages it should be a `transport/` directory. The UI
layer is transport-blind either way.

**`ui/leptos.rs` contains only binding code** — `#[component]`, `view!`, signals, resources,
effects. It has no business logic, no request construction, no CSS class policy. This keeps
the Leptos-specific surface as small as possible, which is what makes framework migration
incremental.

---

## Dual-Path Transport Model

The RusTok transport contract is always dual-path. Full spec:
[`docs/UI/graphql-architecture.md`](./graphql-architecture.md)

```
ui/leptos.rs
  └─► transport::fetch_x()          (facade, transport-blind)
        ├─ SSR/hydrate profile:      native_server_adapter  →  ServiceLayer  →  DB
        └─ CSR/headless profile:     graphql_adapter        →  /api/graphql
```

Both paths must work. Neither cancels the other.

| Situation | Rule |
|---|---|
| Adding a `#[server]` path | GraphQL path stays; both coexist |
| SSR monolith deployment | `native_server_adapter` is preferred |
| CSR/Trunk debug | `graphql_adapter` is used; `/api/fn/*` must not be required |
| Next.js or mobile host | GraphQL/REST only; `#[server]` is never involved |

The reason for both paths: in monolith deployment `apps/admin` and `apps/storefront` run
same-origin with `apps/server`, so native server functions provide a short internal Rust
path. But headless clients (Next.js, mobile, external integrations) always use GraphQL/REST
and must not depend on Leptos runtime.

---

## Dioxus-Readiness

The three-layer structure is designed so that a Dioxus migration never touches `core` or
`transport/` or the `transport.rs` facade. Only `ui/leptos.rs` is replaced.

Rules that keep a package Dioxus-ready:

1. `core.rs` / `core/` must contain **zero `leptos::*` imports**. CI enforces this.
2. The transport facade public API uses transport-neutral domain types, never Leptos types.
   These types usually live in `model.rs`, but small/bootstrap packages may keep them in
   `core.rs` or `transport.rs` when no shared DTO module is needed.
3. `ui/leptos.rs` contains only Leptos binding — no business logic, no request building.

When Dioxus is introduced, a new `ui/dioxus.rs` is added alongside `ui/leptos.rs`. The
Leptos adapter is not deleted — both coexist until the host migration is complete.

### FFA GraphQL Client Boundary

`transport/graphql_adapter.rs` uses `rustok-graphql`, the framework-agnostic GraphQL HTTP client.
Leptos-specific reactive hooks live in `rustok-graphql-leptos` and must not be imported
from transport/core code.

When Dioxus is introduced, add a sibling Dioxus hooks adapter if reactive GraphQL hooks are
needed. The module transport facade remains unchanged because GraphQL request execution is
already framework-neutral.

**Current target state:**
```
ui/leptos.rs -> transport/mod.rs -> graphql_adapter (rustok-graphql) -> /api/graphql
ui/dioxus.rs -> transport/mod.rs -> graphql_adapter (rustok-graphql) -> /api/graphql
```

Migration plan: [`docs/research/dioxus-ffa-ui-migration-plan.md`](../research/dioxus-ffa-ui-migration-plan.md)

---

## Host vs Module Ownership

### Host apps (`apps/admin`, `apps/storefront`) are composition roots

They are responsible for:
- shell, routing, navigation, RBAC guards
- mounting module-owned packages via generated registry (`build.rs`)
- providing `UiRouteContext`, locale context, auth/session, tenant scope

They are **not** responsible for:
- module-specific CRUD or business workflows
- domain logic that belongs to a module
- knowing about module-internal transport or core

**Current state:** Host apps still use `leptos_i18n` for their shell/navigation i18n.
Module-owned Leptos UI packages use `rustok-ui-i18n-leptos`, which adapts the
framework-agnostic `rustok-ui-i18n` catalog core to host-provided `UiRouteContext.locale`.
When Dioxus enters the workspace, add a sibling `rustok-ui-i18n-dioxus` adapter instead
of adding Dioxus dependencies to the core crate.

If module business UI ends up inside `apps/admin/src/` (outside of
`src/widgets/app_shell/` or `src/shared/`), that is an ownership violation.

### Module UI packages own their domain surface

A module-owned UI package lives in `crates/rustok-<module>/admin/` or
`crates/rustok-<module>/storefront/`. The host mounts it through manifest-driven wiring
(`rustok-module.toml`) and passes context — it never pulls internal logic out of the package.

A UI package must not place another module's logic inside itself. If it needs data from
another module, it consumes that module's public transport contract only.

Host-level FFA slices (navigation policy, header route/link policy) are enforced by
`npm run verify:frontend:host-ffa-contract`.

---

## What Counts as a Correctly Structured UI Package

A module UI package is considered correctly structured when:

- `core.rs` / `core/` has no `leptos::*` imports
- `transport/mod.rs` or `transport.rs` is the only facade consumed by `ui/leptos.rs`
- `ui/leptos.rs` calls only `transport::*` functions, never raw adapter internals
- Both `native_server_adapter` and `graphql_adapter` exist for target dual-path packages.
  A single-adapter package must be documented as a current exception in the module
  implementation plan: GraphQL-only means native parity is pending; native-only is allowed
  only for internal operator/bootstrap surfaces with no GraphQL/REST contract yet and must
  carry an explicit parity or exemption note.
- Effective locale comes from `UiRouteContext.locale` passed by the host — no package-local
  fallback chain
- Selection state uses typed `snake_case` URL query keys via `leptos-ui-routing`
- `rustok-module.toml` declares the UI surface with correct `leptos_crate` and
  `route_segment`
- The README links to this document and to the implementation guide

The structural shape taxonomy used in
[`docs/modules/registry.md`](../modules/registry.md):
`none` → `docs_boundary` → `core_only` → `core_transport` → `core_transport_ui`

---

## Related Documents

| Document | When to read |
|---|---|
| [Implementation Guide](./module-package-implementation.md) | Writing or changing code |
| [Verification Guide](./module-package-verification.md) | Checking nothing is broken |
| [FFA Definition](../research/fluid-frontend-architecture.md) | Understanding the full model |
| [Dioxus Migration Plan](../research/dioxus-ffa-ui-migration-plan.md) | Planning Dioxus work |
| [Transport Contract](./graphql-architecture.md) | Transport dual-path rules |
| [ADR: SSR-first + headless parity](../../DECISIONS/2026-04-24-ssr-first-leptos-hosts-with-headless-parity.md) | Governing decision |
| [Module UI Quickstart](../modules/UI_PACKAGES_QUICKSTART.md) | Creating a new module UI |
