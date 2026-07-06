---
id: doc://docs/verification/platform-api-surfaces-verification-plan.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Platform Verification Plan: API Surfaces

- **Status:** current detailed checklist
- **Scope:** GraphQL, REST, `#[server]`, OpenAPI, host/runtime API contract
- **Companion plan:** [RBAC Rolling Plan for Server and Runtime Modules](./rbac-server-modules-verification-plan.md)

---

## Current Scoped Contract

The RusToK API layer must remain consistent with the current-state transport model:

- GraphQL — canonical UI-facing contract
- REST — integration/ops/module-owned HTTP contract
- `#[server]` — internal Leptos data layer
- OpenAPI — machine-readable REST contract

API surface verification must not break this separation.

## Phase 1. GraphQL

### 1.1 Schema composition

**No-compile evidence:** `node scripts/verify/verify-api-surface-contract.mjs` checks generated composition for optional GraphQL modules against `modules.toml` and package-local `rustok-module.toml` declarations.

**Files:**
- `apps/server/src/graphql/schema.rs`
- `apps/server/src/graphql/mod.rs`
- `apps/server/src/graphql/queries.rs`
- `apps/server/src/graphql/mutations.rs`

- [ ] `Query`, `Mutation` and `Subscription` are assembled through the current composition root.
- [ ] Core surfaces reflect the current-state contract of the host and modules.
- [x] Optional modules are connected through the current generated/manifest-driven path, without the old manual drift (`apps/server/build.rs` → `graphql_schema_codegen.rs`, `app_routes_codegen.rs`).
- [ ] GraphQL routing is consistent with `apps/server` and `docs/architecture/api.md`.

### 1.2 Module ownership

- [ ] GraphQL resolvers use the module/service layer, not host-local shortcuts.
- [ ] Module-owned GraphQL surfaces are consistent with the module's local docs.
- [ ] Capability-owned surfaces are not passed off as platform modules.

### 1.3 Locale / auth / RBAC

- [ ] The GraphQL path uses a unified tenant/auth/RBAC context.
- [ ] Locale contract matches `docs/architecture/i18n.md` and `docs/UI/*`.
- [ ] No module-local locale fallback chains within the API contract.

## Phase 2. REST and HTTP Surfaces

### 2.1 REST boundaries

**Files:**
- `apps/server/src/controllers/`
- module-owned `controllers/`

- [ ] REST is used for integration/ops/webhook flows and module-owned HTTP contract.
- [ ] REST does not duplicate UI-only GraphQL flows without reason.
- [ ] Commerce-compatible routes reflect the current host/runtime contract.

### 2.2 Module-owned routing

**No-compile evidence:** `node scripts/verify/verify-api-surface-contract.mjs` checks that modules with `[provides.http]` are read from package manifests and reflected in the central registry.

- [ ] Module HTTP routes are consistent with `rustok-module.toml`, if the module publishes a transport surface.
- [x] The host application only mounts routing, not becomes the owner of module transport logic via the generated `append_optional_module_routes` path.
- [ ] Presence of a controller-path without a manifest/doc contract is not considered completed wiring.

## Phase 3. `#[server]` Contract

### 3.1 Leptos internal data layer

- [ ] For Leptos hosts and module-owned Leptos UI, `#[server]` functions remain the preferred internal path.
- [ ] GraphQL is preserved in parallel and not removed.
- [ ] `#[server]` functions are not used as external integration API.

## Phase 4. OpenAPI and Operational Endpoints

### 4.1 Discovery and ops

**No-compile evidence:** `node scripts/verify/verify-api-surface-contract.mjs` — fast source guardrail; runtime/export evidence stays in 4.2.

- [ ] OpenAPI endpoints publish the current REST contract.
- [ ] health/metrics/ops endpoints match the current server/runtime contract.
- [ ] The API documentation layer does not diverge from actual routing.

### 4.2 Reference artifacts export (DOC-09 / B11)

- [ ] Export completed: `node scripts/verify/export-reference-artifacts.mjs artifacts/reference` (or equivalent CI/Unix wrapper `.sh`).
- [ ] Output contains `openapi/openapi.json`, `openapi/openapi.yaml`, `graphql/introspection.json`, `graphql/schema.graphql`, `manifest.json`.
- [ ] Export passed `node scripts/verify/verify-reference-artifacts.mjs artifacts/reference`.
- [ ] For API contract PRs, Verification Evidence with date, commands and statuses is attached.


## Phase 5. Targeted Checks

### 5.1 Local Minimum

- [ ] `cargo check --workspace --all-targets --all-features`
- [ ] targeted `cargo test` or `xtask module test <slug>` for affected API modules
- [ ] targeted GraphQL/REST smoke, if routing or schema contract changed

## Open Blockers

- [ ] Record runtime-only blockers separately if they cannot be reproduced in the current local environment.
- [ ] Do not turn this document into a backlog; describe blockers briefly and link to the owning component.

## Related Documents

- [API Architecture](../architecture/api.md)
- [Routing and Transport Boundaries](../architecture/routing.md)
- [Module Architecture](../architecture/modules.md)
- [Main Verification README](./README.md)
