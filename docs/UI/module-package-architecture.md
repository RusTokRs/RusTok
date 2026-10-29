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

**FFA — Fluid Frontend Architecture** is the RusTok-specific model for
framework-separated **Rust UI**: pure core/model, transport facade and a
replaceable Leptos adapter (SSR/hydrate or CSR/headless). Dioxus is a planned
second Rust render adapter that will reuse core/model and transport.
Classic React/Next.js modules are independent UI implementations owned
inside their Next hosts; they do not execute or implement the FFA Rust layer.

Full definition: [`docs/research/fluid-frontend-architecture.md`](../research/fluid-frontend-architecture.md)

The core claim for Rust FFA is that switching Leptos SSR/hydrate to
CSR/headless changes only the selected transport, not domain presentation
logic or the Leptos components. A later Dioxus render adapter shares the
same pure state and backend facades but provides its own components.
A classic Next.js host independently composes React components and routes.

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
only a new `ui/dioxus.rs` render adapter needs to be added alongside `ui/leptos.rs`. View-models, state transitions,
validation, and display policy are reused unchanged across frameworks and across transport
profiles.
Framework-agnostic UI route/query contracts (`UiRouteContext`, `UiRouteQueryUpdate`,
`UiRouteQueryIntent`), input normalization, and busy-key helpers live in `rustok-ui-core`,
not in `rustok-api`.

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
| Classic Next.js host or mobile client | Uses public GraphQL/REST; not a Rust FFA adapter and never calls `#[server]` |

The reason for both paths: in monolith deployment `apps/admin` and `apps/storefront` run
same-origin with `apps/server`, so native server functions provide a short internal Rust
path. But independent headless clients (Next.js, mobile, external integrations) always use GraphQL/REST
and must not depend on Leptos runtime.

Shared transport path/error evidence and build-profile selected transport orchestration for module facades belongs
in `rustok-ui-transport`. Module packages may keep domain-specific transport policy locally,
but should not duplicate generic transport path, result, native/GraphQL error evidence types
or `execute_selected_transport` selected-path wiring.

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
Leptos-specific resources, signals, and effects remain in `ui/leptos.rs`; they call the
module transport facade and must not move into transport/core code. The platform does not
retain a generic reactive GraphQL hooks crate without a concrete shared consumer.

When Dioxus is introduced, its resource bindings call the same module transport facade. The
facade remains unchanged because GraphQL request execution is already framework-neutral.

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

**Current state:** Host and module catalogs use `rustok-ui-i18n`. Leptos hosts own only
the reactive propagation of a host-provided effective locale; they do not own message
parsing, fallback, formatting, or catalog generation.
Module-owned UI packages use `rustok-ui-i18n` (`UiMessages`), which resolves
messages against host-provided `UiRouteContext.locale` without any framework coupling.
`UiRouteContext` itself is a framework-agnostic UI contract from `rustok-ui-core`.
Because `UiMessages` operates entirely on plain Rust types, it is universally reusable
across Leptos, Dioxus, or any other UI framework without adapter crates.

If module business UI ends up inside `apps/admin/src/` (outside of
`src/widgets/app_shell/` or `src/shared/`), that is an ownership violation.

### Module UI packages own their domain surface

A module-owned UI package lives in `crates/modules/rustok-<module>/admin/` or
`crates/modules/rustok-<module>/storefront/`. The host mounts it through manifest-driven wiring
(`rustok-module.toml`) and passes context — it never pulls internal logic out of the package.

A UI package must not place another module's logic inside itself. If it needs data from
another module, it consumes that module's public transport contract only.

Host-level FFA slices (navigation policy, header route/link policy) are enforced by
`npm run verify:frontend:host-ffa-contract`.

---

## UI scaffolding: request is not readiness

The native generator's `--ui admin`, `--ui storefront`, and `--ui dual`
options request module-owned surfaces. They do not establish functional routes,
transport coverage, parity or FFA status. No UI route or `[provides.*_ui]`
entry should be published for placeholder CRUD or missing owner operations.
When the backend contract is not implemented, the owner README and plan must
describe UI as pending instead of rendering demo records or log-only actions.

When real operations exist, a generated Leptos UI adapter uses a thin `lib.rs`,
transport-neutral `model`, pure tested `core`, working `transport`, render-only
`ui`, and interface-copy-only `i18n`. The adapter cannot substitute
an empty transport facade for the two real native/GraphQL paths, subject to the
documented operator/bootstrap exception. `--no-ui` means no UI packages.

A functional **Leptos FFA publication** requires actual native and GraphQL
transport implementations and the relevant Leptos host registration. It does
not structurally require a classic Next.js package. Where the same business
feature is independently exposed by a Next host, the feature's API and
capability compatibility must be reviewed in that host's own scope; such
verification is not a prerequisite for the FFA source-layout lint and cannot
be inferred from Leptos compilation. The current scaffold preview does not
yet meet the functional-publication contract.

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

## Existing scaffold: draft UI preview versus live surface

The `cargo xtask scaffold` / `cargo xtask module new` generator is a long-lived
workspace entrypoint, not a new competing scaffolding system. Its native backend
now starts without invented domain state. A requested `--ui admin|storefront|dual`
creates unmounted Cargo workspace Leptos packages for later module-owned work.
Use `--dry-run` to inspect without writing. The manifest uses `future_ui`
and does not declare any live `[provides.*_ui]` until the module owner
implements real operations and host wiring.
The generated draft contains only Leptos FFA files. Classic Next.js packages are
independent React packages inside Next hosts, not FFA adapters and not part
of the Rust scaffolding output.

The generated Leptos draft keeps `core/mod.rs`, `model.rs`,
`transport/mod.rs`, `transport/native_server_adapter.rs`,
`transport/graphql_adapter.rs`, `ui/mod.rs`, and `ui/leptos.rs`.
It uses the canonical `rustok-ui-transport` path selection, but both
owner-specific adapters explicitly fail with `MissingOwnerContract` until
real operations are implemented. This is **structural FFA scaffolding**, not
a working business API or a verified runtime transport. The generated shell
shows an unavailable state without simulated CRUD.
Before activation, implement actual owner operations and context-aware
native/GraphQL adapters. Future `ui/dioxus.rs` must share the same
framework-independent core/model and transport facade, keeping Leptos intact.
Any Next.js product features must be built in their Next host packages.

Blog remains the production reference for these boundaries; its existing
Leptos adapters are not modified by scaffold changes.
