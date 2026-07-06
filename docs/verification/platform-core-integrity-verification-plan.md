---
id: doc://docs/verification/platform-core-integrity-verification-plan.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Platform Verification Plan: Core Integrity

- **Status:** current detailed checklist
- **Scope:** core crates, foundation contracts, module registry, auth/RBAC/tenant core
- **Companion plan:** [Main Platform Verification Plan](./PLATFORM_VERIFICATION_PLAN.md)

---

## Current Scoped Contract

The core integrity verification plan checks that the server host and foundation crates
still form a consistent core for all platform modules.

This includes:

- `apps/server`
- `rustok-core`
- `rustok-api`
- `rustok-events`
- `rustok-outbox`
- `rustok-tenant`
- `rustok-rbac`
- `rustok-auth`
- `rustok-cache`
- `rustok-email`

## Phase 1. Foundation Contracts

### 1.1 Core crates

- [ ] Foundation crates compile and do not diverge in public contracts.
- [ ] Shared contracts for the module/runtime layer are not duplicated locally in host code.
- [ ] Event, auth, tenant and RBAC contracts match central docs and local docs of owning crates.

### 1.2 Module registry

- [ ] `ModuleRegistry` and manifest/runtime wiring reflect the current platform composition.
- [ ] `Core` and `Optional` semantics are not blurred.
- [ ] Support/capability crates are not passed off as platform modules.

## Phase 2. Auth / Tenant / RBAC Core

### 2.1 Auth baseline

- [ ] Auth/session contract is centralized and not scattered across host-local workarounds.
- [ ] Password/session/token flow matches the current auth contract.
- [ ] Email/auth integration does not diverge from the foundation/runtime layer.

### 2.2 Tenant baseline

- [ ] Tenant resolution remains a single host/runtime path.
- [ ] Tenant lifecycle does not break core module semantics.
- [ ] `tenant_modules` is used only for `Optional` flows and does not replace platform composition.

### 2.3 RBAC baseline

- [ ] RBAC enforcement path goes through the current typed/runtime contract.
- [ ] Host/module code does not revert to ad-hoc role checks.
- [ ] Permission ownership matches owning modules and local docs.

## Phase 3. Runtime Services

### 3.1 Cache / email / outbox

- [ ] Cache runtime remains a single shared path.
- [ ] Email runtime is not duplicated bypassing the platform contract.
- [ ] Outbox/runtime delivery remains part of the core baseline, not an optional add-on.

## Phase 4. Targeted Local Checks

### 4.1 Minimum

- [ ] `cargo check --workspace --all-targets --all-features`
- [ ] targeted `cargo test` for foundation/core crates, if the contract changed
- [ ] `cargo xtask validate-manifest`, if the central composition contract changed

## Open Blockers

- [ ] Record environment/runtime blockers separately, do not clutter the checklist itself with history.
- [ ] When drift occurs, first update local docs of the owning component, then central verification docs.

## Related Documents

- [Platform Architecture Overview](../architecture/overview.md)
- [Module Architecture](../architecture/modules.md)
- [`rustok-module.toml` Contract](../modules/manifest.md)
- [Main Verification README](./README.md)
