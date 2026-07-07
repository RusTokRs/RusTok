---
id: doc://docs/ai/KNOWN_PITFALLS.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---

# KNOWN_PITFALLS for AI (RusToK)

Short list of typical mistakes before making code changes.

## Loco

- Do not add new dependencies on `loco_rs` outside the already classified inventory. Run `node scripts/verify/verify-loco-inventory.mjs` on Loco/Axum cutover.
- Do not design new server-owned services around `loco_rs::app::AppContext`; use `ServerRuntimeContext` or narrow typed contexts.
- Do not move maintenance/CLI flows into the production server binary. The target layer is a separate `rustok-ops` and module-local `cli/` adapters.
- While legacy controllers are not yet migrated, do not mix new Axum error contracts with Loco controller paths in the same slice; translate route/error surface atomically per plan.

## Iggy / Outbox

- For write + event, do not use fire-and-forget `publish(...)`; use `publish_in_tx(...)`.
- Do not port Kafka/NATS-specific APIs (offset commits, subject-only routing) that don't exist in the current abstraction.
- Do not invent Iggy configuration: first check the actual `IggyConfig`, `ConnectorConfig`, `ConnectorMode`.

## MCP

- Do not bypass typed tools/response envelope (`McpToolResponse`) with ad-hoc JSON responses.
- Do not move business logic into the MCP adapter: the layer must remain thin over service/registry.
- For limited access, use an allow-list of tools via `McpServerConfig::with_enabled_tools(...)`.

## Outbox

- For write + event that require consistency, use `publish_in_tx(...)`, not `publish(...)`.
- Do not run production with outbox without a relay worker.

## Telemetry

- Do not reinitialize telemetry runtime multiple times.
- Do not spread metrics across different registries unnecessarily.

## Database / SeaORM

- **Always** add `.filter(...::Column::TenantId.eq(tenant_id))` to every SELECT/UPDATE/DELETE. A query without `tenant_id` is a cross-tenant data leak.
- Do not use `find().all(&db)` without a filter — it will load the ENTIRE table.
- Do not create domain tables without a `tenant_id UUID NOT NULL` column.
- Do not use string concatenation for SQL — only parameterized queries via SeaORM.
- Do not return Entity directly from API — create separate DTOs (Input/Response).
- Do not hard DELETE business entities (products, orders, nodes) — use soft delete via status = Archived.
- Name migrations strictly: `mYYYYMMDD_<module>_<nnn>_<description>`.

## State Machines

- Do not use `String` for status fields — use enum with type-safe transitions.
- Do not add state transitions without updating property tests (`*_proptest.rs`).
- Do not allow "reverse" transitions without an explicit ADR (e.g., Published → Draft).
- Every new state machine must have a proptest for exhaustive transition checking.

## Frontend / Leptos

- Do not use raw `fetch()` in Rust FFA GraphQL adapters; use `rustok-graphql`.
- Do not store JWT manually in localStorage — use `leptos-auth`.
- Do not copy components between admin and storefront — use `iu-leptos` design system.
- Do not use SSR for admin panel (use CSR/WASM) and do not use CSR for storefront (use SSR for SEO).
- Do not thread props through 5+ levels — use `leptos-zustand` for global state.

## Frontend / Next.js

- Do not duplicate code between `apps/next-admin` and `apps/next-frontend` — extract to `packages/`.
- Do not add custom GraphQL clients in Next.js code; use the host Apollo wrapper.
- Do not use `any` types — strict TypeScript mode.
- Do not forget Clerk ↔ Server JWT integration in `apps/next-admin`.
- Do not use `@ts-ignore` / `@ts-expect-error` — fix the types.

## Docker / Deployment

- Do not run production with `transport = "memory"` — use `transport = "outbox"`.
- Do not forget the relay worker when deploying with outbox transport.
- Do not use default credentials from `.env.dev.example` in production.
- Do not expose `/swagger` and `/metrics` without auth in production.

## Migrations

- Do not modify already applied migrations — create new ones.
- Do not delete columns without prior ADR and migration plan.
- Do not create migrations outside `RusToKModule::migrations()` — use the standard mechanism.
- Do not forget to add a migration for every new entity.

## Mandatory Check Before Changes

If the task touches Loco/Iggy/MCP/Outbox/Telemetry/Database/Frontend:
1. First open the corresponding reference package:
   - `docs/architecture/loco-exit-plan.md`
   - `DECISIONS/2026-07-02-axum-runtime-and-ops-cli-boundary.md`
   - `docs/references/iggy/README.md`
   - `docs/references/mcp/README.md`
   - `docs/references/outbox/README.md`
   - `docs/references/telemetry/README.md`
2. Read [Forbidden Actions](../standards/forbidden-actions.md) — hard prohibitions.
3. Read [Patterns vs Antipatterns](../standards/patterns-vs-antipatterns.md) — summary table.
4. Only after that change code/documentation.
