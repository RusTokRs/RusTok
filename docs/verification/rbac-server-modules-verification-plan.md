---
id: doc://docs/verification/rbac-server-modules-verification-plan.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Platform Verification Plan: RBAC, Server and Runtime Modules

- **Status:** current detailed checklist
- **Scope:** server authorization path, typed permissions, runtime module contract, capability boundaries
- **Companion plan:** [Main Platform Verification Plan](./PLATFORM_VERIFICATION_PLAN.md)

---

## Current RBAC and Server Access Contract

This plan confirms that the live authorization contract remains consistent
between `apps/server`, `rustok-rbac`, foundation crates, runtime modules and
capability surfaces.

Sources of truth for RBAC/server verification:

- `apps/server` code
- typed permission vocabulary from `rustok-core`
- runtime module contracts from `modules.toml`, `rustok-module.toml` and `RusToKModule`
- local docs of affected modules and capability crates

## Phase 1. Server Authorization Path

### 1.1 Entry points

- [ ] GraphQL, REST, `#[server]` and operational endpoints go through the current auth/RBAC path.
- [ ] `SecurityContext` is built from resolved permissions and tenant/user context, not from role shortcuts.
- [ ] Server extractors, guards and service entry points do not create parallel authorization rules.

### 1.2 Antipattern checks

- [ ] In the live server path, there are no ad-hoc checks like `UserRole::*` instead of typed permissions.
- [ ] `infer_user_role_from_permissions()` does not replace actual authorization.
- [ ] Host-level workarounds do not duplicate `RbacService` and permission-aware guards.

## Phase 2. Typed Permission Vocabulary

### 2.1 Foundation contract

- [ ] `Permission`, `Resource`, `Action` from `rustok-core` remain the single source of permission vocabulary.
- [ ] Server-side authorization does not fall back to stringly-typed permissions or local role aliases.
- [ ] Local docs and central docs do not diverge from the current permission model.

### 2.2 Module ownership

- [ ] Runtime modules with RBAC-managed functionality publish the current permission surface.
- [ ] Ownership permissions for `auth`, `tenant`, `rbac`, `content`, `commerce`, `blog`, `forum`, `pages`, `media`, `workflow` match between code, manifest and docs.
- [ ] Dependency edges like `blog -> content`, `forum -> content`, `pages -> content` do not hide undocumented authorization expectations.

## Phase 3. Runtime Modules and Capability Boundaries

### 3.1 Runtime module contract

- [ ] `modules.toml`, runtime registry and `RusToKModule::permissions()` are consistent.
- [ ] Runtime modules do not lose the `README.md` / `docs/README.md` / `docs/implementation-plan.md` contract.
- [ ] `outbox` remains a `Core` module and is not mixed with tenant-toggled capability semantics.

### 3.2 Capability surfaces

- [ ] `alloy`, `flex`, `rustok-mcp` and other capability crates are not disguised as runtime modules.
- [ ] Capability docs explicitly describe their authorization boundaries and dependencies on the server/runtime contract.
- [ ] Capability paths do not use `tenant_modules` as a substitute for an explicit permission model, unless this is part of a documented runtime contract.

## Phase 4. Documentation Sync

### 4.1 Central docs

- [ ] `docs/modules/registry.md`, `docs/modules/crates-registry.md`, `docs/architecture/api.md`, `docs/architecture/modules.md` reflect the current RBAC/server picture.
- [ ] Verification docs remain a checklist layer and do not turn into an archive of investigations.

### 4.2 Local docs

- [ ] Affected runtime modules and capability crates synchronize `README.md`, `docs/README.md`, `docs/implementation-plan.md`.
- [ ] The `## Interactions` section in the root `README.md` does not diverge from the server authorization path and runtime dependencies.

## Targeted Local Checks

- [ ] targeted `cargo xtask module validate <slug>` for modules affecting auth/RBAC/server boundaries
- [ ] targeted `cargo xtask module test <slug>` for affected modules
- [ ] targeted `cargo test -p rustok-server --lib`, if server authorization path changed
- [ ] targeted grep/rg on `apps/server/src` for role shortcuts and local authorization bypass patterns
- [ ] `powershell -ExecutionPolicy Bypass -File scripts/verify/verify-architecture.ps1`, if dependency boundaries or server/module ownership changed

## Stop-the-line Conditions

- [ ] Live server path authorizes by role shortcuts instead of explicit permissions.
- [ ] Runtime module with RBAC-managed behavior does not publish the current permission surface.
- [ ] Capability crate is introduced into the server/runtime path without a clear authorization contract.
- [ ] Docs claim one permission/dependency picture but the code implements another.

## Related Documents

- [Main Verification README](./README.md)
- [Core Integrity Verification](./platform-core-integrity-verification-plan.md)
- [API Surface Verification](./platform-api-surfaces-verification-plan.md)
- [API Architecture](../architecture/api.md)
- [Module Architecture](../architecture/modules.md)
- [Module and Application Registry](../modules/registry.md)
