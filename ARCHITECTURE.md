# RusTok Architecture Quick Reference

This document provides a 1-page high-level architecture map for developers and AI agents operating in the RusTok repository.

---

## 1. High-Level System Layers

```
                     ┌────────────────────────────────────────────────────────┐
                     │              Host Applications                         │
                     │  apps/server (Composition Root)                        │
                     │  apps/admin | apps/storefront (Leptos SSR)             │
                     │  apps/next-admin | apps/next-frontend (Next.js)        │
                     └──────────────────────────┬─────────────────────────────┘
                                                │
                     ┌──────────────────────────▼─────────────────────────────┐
                     │          Platform Core & Modules                       │
                     │  modules.toml (Manifest Composition)                  │
                     │  Core Modules: auth, tenant, rbac, index, search,      │
                     │                outbox, events, cache, email, channel   │
                     │  Optional Domain Modules: commerce, product, cart...   │
                     └──────────────────────────┬─────────────────────────────┘
                                                │
                     ┌──────────────────────────▼─────────────────────────────┐
                     │          Foundation & Capability Crates                │
                     │  rustok-core | rustok-api | rustok-events | rustok-fba │
                     │  rustok-mcp  | rustok-ai  | alloy | rustok-sandbox     │
                     └────────────────────────────────────────────────────────┘
```

---

## 2. Key Entry Points & Files

| What | Canonical Path | Description |
|---|---|---|
| **Module Manifest** | [`modules.toml`](modules.toml) | Declarative build composition & module taxonomy |
| **Composition Root** | [`apps/server/src/main.rs`](apps/server/src/main.rs) | Main Axum HTTP server, GraphQL, & runtime registry |
| **API & Port Contracts** | [`crates/rustok-api/src/ports.rs`](crates/rustok-api/src/ports.rs) | `PortContext`, `PortActor`, and transport-agnostic errors |
| **Read Index Engine** | [`crates/rustok-index/docs/README.md`](crates/rustok-index/docs/README.md) | PostgreSQL `JSONB` CQRS read model & keyset cursors |
| **Transactional Outbox** | [`crates/rustok-outbox/docs/README.md`](crates/rustok-outbox/docs/README.md) | Atomic `sys_events` delivery & Iggy stream replay |
| **Sandboxed Scripting** | [`crates/alloy/README.md`](crates/alloy/README.md) | Dynamic Rhai/WASM hooks & ETL data sanitization |
| **AI & MCP Agent Server** | [`crates/rustok-mcp/README.md`](crates/rustok-mcp/README.md) | Model Context Protocol server for AI agent operations |
| **Documentation Map** | [`docs/index.md`](docs/index.md) | Canonical index of all architecture & module docs |
| **Module & Owner Map** | [`docs/modules/registry.md`](docs/modules/registry.md) | Complete FFA/FBA readiness board and evidence links |

---

## 3. Core Design Invariants

1. **Safety by Design**: Every domain invocation receives a `PortContext` with mandatory `tenant_id`, `actor`, OpenTelemetry trace identifiers, and `deadline_ms` timeout propagation.
2. **AI Permission Intersection**: AI agent runs operate under `AgentPrincipal` where effective permissions are calculated as $\text{User Permissions} \cap \text{Agent Descriptor Permissions}$.
3. **Fluid Topology (FBA)**: Business logic is written once in pure Rust traits. Modules can run in-process (embedded monolith) or as remote **gRPC** microservices by changing runtime provider flags (`RUSTOK_*_PROVIDER=grpc`).
4. **Fluid Frontend (FFA)**: UI view-models and state machines live in framework-agnostic Rust crates (`rustok-ui-core`). Leptos views (`#[server]` functions) and Next.js / Flutter clients consume identical domain logic without code rewrite.
5. **No Code Duplication**: Shared UI primitives live in `crates/leptos-ui/`, UI routing in `crates/leptos-ui-routing/`, and transport helpers in `crates/rustok-ui-transport/`.

---

## 4. Where to Read More

- [Platform Architecture Overview](docs/architecture/overview.md)
- [Architecture Principles](docs/architecture/principles.md)
- [API Architecture](docs/architecture/api.md)
- [Routing Architecture](docs/architecture/routing.md)
- [Glossary of Platform Terms](docs/glossary.md)
