# RusTok Architecture Quick Reference

This document is the short current-state architecture map for contributors and
AI agents. `AGENTS.md`, active ADRs, and owner-local documentation define the
detailed contracts.

---

## 1. Platform Shape

RusToK is a **modular monolith by default**. Domain ownership is separated by
contracts even when owners execute in one process.

```text
┌──────────────────────────────────────────────────────────────────────┐
│ Host applications                                                    │
│ apps/server — composition root / Axum / runtime context             │
│ apps/admin + apps/storefront — RusToK-owned Leptos hosts            │
│ apps/next-admin + apps/next-frontend — optional public-API clients  │
└───────────────────────────────┬──────────────────────────────────────┘
                                │ composes owner entrypoints
┌───────────────────────────────▼──────────────────────────────────────┐
│ crates/modules/*                                                     │
│ domain owners, core/optional module entries, capability extensions   │
│ modules.toml — selected build/runtime composition                    │
└───────────────────────────────┬──────────────────────────────────────┘
                                │ depends on stable foundation seams
┌───────────────────────────────▼──────────────────────────────────────┐
│ crates/libs/* | crates/ui/* | crates/utils/* | crates/workers/*      │
│ shared contracts/runtime | UI foundations | tooling | worker hosts   │
└──────────────────────────────────────────────────────────────────────┘
```

A module may have an external provider/transport profile **only when its owner
actually publishes that port, transport adapter, service host, security
boundary, and verification evidence**. Remote execution is not a universal
microservice switch and must never silently fall back to another authority.

---

## 2. Composition Axes

Do not collapse these independent concepts:

| Axis | Meaning |
| --- | --- |
| Architectural role | domain module, capability, shared library, host, worker |
| Tenant lifecycle | `Core` or `Optional` for tenant-managed module entries |
| Runtime composition | `runtime = "module"` or `runtime = "extension"` |
| Packaging | crate, package, binary, generated artifact |

`runtime = "extension"` is a deployment-scoped capability contribution, not a
third tenant `ModuleKind`.

The composition source of truth is [`modules.toml`](modules.toml), synchronized
with each owner manifest, runtime registration, and owner documentation.

---

## 3. Key Entry Points

| What | Canonical path | Contract |
| --- | --- | --- |
| Repository governance | [`AGENTS.md`](AGENTS.md) | source-of-truth, cutover, concurrency, verification rules |
| Documentation map | [`docs/index.md`](docs/index.md) | canonical documentation entry point |
| ADR registry | [`DECISIONS/README.md`](DECISIONS/README.md) | current architecture-decision and implementation status |
| Module composition | [`modules.toml`](modules.toml) | selected runtime/build graph |
| Module manifest contract | [`docs/modules/manifest.md`](docs/modules/manifest.md) | `modules.toml` / `rustok-module.toml` semantics |
| Composition root | [`apps/server`](apps/server) | host wiring, transport, auth/session integration |
| API/port contracts | [`crates/libs/rustok-api`](crates/libs/rustok-api) | stable cross-boundary request/error/context contracts |
| Transactional outbox | [`crates/modules/rustok-outbox`](crates/modules/rustok-outbox) | durable write + event publication path |
| Index engine | [`crates/modules/rustok-index`](crates/modules/rustok-index) | generic relational derived index/read substrate |
| Verification entry | [`docs/verification/README.md`](docs/verification/README.md) | change-driven canonical verification routes |

---

## 4. Core Invariants

1. **One canonical owner/source of truth.** Other representations are adapters,
   overlays, projections, caches, indexes, or derived state.
2. **Owner boundaries beat process boundaries.** Embedded execution does not
   permit consumers to query another owner's private tables or repositories.
3. **Write-side correctness is transactional.** Tenant integrity, database
   invariants, revisions, and required outbox facts are atomic where causally
   coupled.
4. **Read-side state is derived and rebuildable.** Index/search/projection
   models may denormalize but may not become competing write authorities or
   invent domain combinations that do not exist.
5. **Effective request context is typed.** Tenant, channel, locale, principal,
   policy, deadline, and trace dimensions are resolved by their canonical owner
   and propagated explicitly.
6. **UI remains owner-owned.** Leptos uses native `#[server]` paths for the
   internal SSR/hydrate surface where applicable; GraphQL/REST remain the public
   headless contracts required by the owner. Optional Next clients consume those
   public contracts rather than owning domain logic.
7. **Reuse follows semantics.** Similar code triggers an abstraction review;
   shared libraries are created only for a real common contract and dependency
   direction.
8. **Zero legacy before release.** Internal replacements are canonical,
   unversioned cutovers without dual read/write or compatibility families unless
   an explicit external consumer requires a bounded bridge.

---

## 5. Where to Read More

- [Architecture Principles](docs/architecture/principles.md)
- [Platform Architecture Overview](docs/architecture/overview.md)
- [Module Architecture](docs/architecture/modules.md)
- [Module Authoring](docs/modules/module-authoring.md)
- [API Architecture](docs/architecture/api.md)
- [Database Architecture](docs/architecture/database.md)
- [Routing Architecture](docs/architecture/routing.md)
- [Platform Glossary](docs/glossary.md)
