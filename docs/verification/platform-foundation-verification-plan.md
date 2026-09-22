---
id: doc://docs/verification/platform-foundation-verification-plan.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Platform Verification Plan: Foundation

- **Status:** current detailed checklist
- **Scope:** workspace baseline, foundation crates, module composition, auth/RBAC/tenant foundation
- **Companion plan:** [Main Platform Verification Plan](./PLATFORM_VERIFICATION_PLAN.md)

---

## Current Scoped Contract

The foundation verification plan confirms that the platform baseline remains
consistent at three levels:

- workspace and host/runtime foundation
- module composition contract
- minimum docs/manifests/verification for scoped modules

For path-modules, the current-state minimum:

- root `README.md`
- `docs/README.md`
- `docs/implementation-plan.md`
- `rustok-module.toml`

Canonical local commands:

- `cargo xtask module validate <slug>`
- `cargo xtask module test <slug>`
- `cargo xtask validate-manifest`

## Windows-hybrid path

On Windows, the mandatory local verification path does not depend on Bash as a hard
prerequisite.

Minimum baseline:

- Cargo/xtask for module/runtime contract
- Node/npm for UI/i18n/routes gates
- Python for architecture guard
- Git Bash only for legacy perimeter checks, if needed separately

## Phase 1. Workspace Baseline

### 1.1 Build and basic consistency

- [ ] `cargo check --workspace --all-targets --all-features`
- [ ] `cargo fmt --all -- --check`
- [ ] targeted `cargo test`, if foundation/runtime contract changed

### 1.2 Tooling and prerequisites

- [ ] Local environment supports the minimum Windows-hybrid verification path.
- [ ] Environment blockers are recorded separately and do not replace the contract itself.

## Phase 2. Module Composition Contract

### 2.1 `modules.toml` and runtime registry

- [ ] `modules.toml` reflects the actual platform scope.
- [ ] `ModuleRegistry` and manifest/runtime wiring match the composition contract.
- [ ] `Core` and `Optional` semantics are not blurred.
- [ ] Support/capability crates are not passed off as platform modules.

### 2.2 Scoped module contract

- [ ] Path-modules have `rustok-module.toml`.
- [ ] Root `README.md`, `docs/README.md`, `docs/implementation-plan.md` are present and match the current docs-standard.
- [ ] Module dependencies and wiring are consistent between code, manifest and local docs.

## Phase 3. Foundation Crates

### 3.1 Shared contracts

- [ ] `rustok-core`, `rustok-api`, `rustok-events`, `rustok-storage`, `rustok-test-utils` form a consistent foundation layer.
- [ ] Shared contracts are not duplicated locally in host/module code.
- [ ] Central docs match the current foundation boundaries.

### 3.2 Core platform modules

- [ ] `auth`, `cache`, `channel`, `email`, `index`, `search`, `outbox`, `tenant`, `rbac` remain consistent with the runtime baseline.
- [ ] `rustok-outbox` remains a `Core` module, not an optional/support add-on.

## Phase 4. Auth / Tenant / RBAC Foundation

### 4.1 Auth

- [ ] Auth/session/token contract remains centralized.
- [ ] Host-local workarounds do not replace the foundation auth flow.

### 4.2 Tenant

- [ ] Tenant resolution and tenant lifecycle match the current runtime contract.
- [ ] `tenant_modules` is used only for `Optional` flows and does not replace platform composition.

### 4.3 RBAC

- [ ] Typed permission/runtime contract remains unified.
- [ ] No reversion to ad-hoc role checks in host/module code.

## Phase 5. Targeted Local Checks

### 5.1 Minimum

- [ ] `cargo xtask validate-manifest`
- [ ] targeted `cargo xtask module validate <slug>` for affected modules
- [ ] targeted `cargo xtask module test <slug>` for affected modules
- [ ] `powershell -ExecutionPolicy Bypass -File scripts/verify/verify-architecture.ps1`, if architecture/runtime contract changed

## Open Blockers

- [ ] Do not turn this document into a historical incident log.
- [ ] Record runtime/environment blockers briefly and separately.

## Related Documents

- [`rustok-module.toml` Contract](../modules/manifest.md)
- [Modular Platform Overview](../modules/overview.md)
- [Module Architecture](../architecture/modules.md)
- [Main Verification README](./README.md)
