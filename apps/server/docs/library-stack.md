# Server library stack (core dependencies)

This document establishes the **core backend stack libraries** in `apps/server` and their role in RusTok.

> Goal: so that developers and AI agents don't guess from random articles, but quickly see the "official" server stack in this repository.

## Core libraries (platform heart)

| Library | Role in server | Where to look in repository |
|---|---|---|
| `axum` | Backend framework, bootstrap, HTTP routing, handlers and middleware integration | `apps/server/src/host.rs`, `apps/server/src/controllers/**`, `apps/server/src/middleware/**` |
| `sea-orm` | ORM, entities, queries, migrations | `apps/server/src/models/**`, `crates/rustok-migrations/**` |
| `async-graphql` | GraphQL schema/query/mutation/resolvers | `apps/server/src/graphql/**` |
| `tokio` | Async runtime for I/O and background tasks | server entry point and async services in `apps/server/src/**` |
| `serde` / `serde_json` | (De)serialization for API, configs and payload | DTO/response/request structures in `apps/server/src/**` |
| `tracing` | Structured logging/telemetry hooks | `apps/server/src/**` and telemetry/crates integrations |
| `utoipa` | OpenAPI/Swagger description of REST API | `apps/server/src/controllers/swagger.rs` |

## How to verify stack currency

1. Check declared dependencies:

```bash
sed -n '1,220p' apps/server/Cargo.toml
```

2. If the core server stack changes (added/removed a key library), update this file in the same PR.

3. Historical Loco material is archived and must not guide implementation. The
   active runtime boundary is the [Axum runtime and operations CLI ADR](../../../DECISIONS/2026-07-02-axum-runtime-and-ops-cli-boundary.md).

## Document boundary

- This is the **root reference for core libraries**, not a complete tutorial.
- Extract narrow specialized details (e.g., transport/events/observability) into separate markdown files inside `apps/server/docs/`.
