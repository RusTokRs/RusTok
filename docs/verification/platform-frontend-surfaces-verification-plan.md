---
id: doc://docs/verification/platform-frontend-surfaces-verification-plan.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Platform Verification Plan: Frontend Surfaces

- **Status:** current detailed checklist
- **Scope:** Leptos hosts, Next.js hosts, module-owned UI packages, shared UI libraries
- **Companion plan:** [Leptos Libraries Verification Plan](./leptos-libraries-verification-plan.md)

---

## Current Scoped Contract

The frontend surfaces verification plan relies on the current-state UI model:

- UI remains module-owned
- hosts only mount surfaces
- frontend hosts have the status of `FFA-compatible composition host`, not module FFA status
- internal Leptos data layer uses `#[server]`
- GraphQL remains a parallel transport contract
- effective locale comes from the host/runtime layer

## Phase 1. Leptos Hosts

### 1.1 `apps/admin`

**Files:**
- `apps/admin/src/`
- `apps/admin/docs/README.md`

- [ ] `apps/admin` remains a host application, not an owner of module UI.
- [ ] `apps/admin` is documented as an `FFA-compatible composition host`.
- [ ] Module routing and registry reflect the current manifest-driven contract.
- [ ] `#[server]` path and GraphQL path coexist without contract drift.
- [ ] Effective locale is passed through host/runtime context.

### 1.2 `apps/storefront`

**Files:**
- `apps/storefront/src/`
- `apps/storefront/docs/README.md`

- [ ] `apps/storefront` remains a host application for module-owned storefront surfaces.
- [ ] `apps/storefront` is documented as an `FFA-compatible composition host`.
- [ ] Routing, locale path and host wiring match `docs/UI/storefront.md`.
- [ ] No app-local business logic that replaces module package ownership.

## Phase 2. Next.js Hosts

### 2.1 `apps/next-admin`

- [ ] Next admin host mounts module-owned or capability-owned surfaces without ownership drift.
- [ ] `apps/next-admin` is documented as an `FFA-compatible composition host`.
- [ ] Locale/runtime contract matches the common i18n policy.
- [ ] Frontend build/type/lint path remains reproducible.

### 2.2 `apps/next-frontend`

- [ ] Next storefront host uses host/runtime locale contract.
- [ ] `apps/next-frontend` is documented as an `FFA-compatible composition host`.
- [ ] Storefront routing is consistent with the common route contract.
- [ ] Host-only code does not duplicate module-owned domain logic.

## Phase 3. Module-Owned UI Packages

### 3.1 Leptos UI packages

- [ ] `admin/` and `storefront/` sub-crates are consistent with `rustok-module.toml`.
- [ ] UI package docs are consistent with the owning module's local docs.
- [ ] Package does not introduce its own locale/auth contract.
- [ ] Package does not take ownership of domain logic.

### 3.2 Capability-owned UI

- [ ] Capability-owned UI packages are not passed off as UI surfaces of platform modules.
- [ ] Their runtime/docs contract remains consistent with the host layer.

## Phase 4. Shared UI Libraries

### 4.1 Reusable UI/tooling layer

- [ ] Shared Leptos/UI libraries are used as reusable building blocks, not as a hidden host/business layer.
- [ ] Library contracts do not conflict with host locale/runtime policy.

## Phase 5. i18n and Route Checks

### 5.1 Mandatory targeted gates

- [ ] `npm run verify:i18n:ui`
- [ ] `npm run verify:i18n:contract`
- [ ] `npm.cmd run verify:storefront:routes`
- [ ] `npm run verify:frontend:host-ffa-contract`

If host wiring or UI contract changed, these checks are considered mandatory.

## Phase 6. Targeted Local Checks

### 6.1 Minimum

- [ ] targeted `cargo check` / `cargo test` for affected Leptos packages
- [ ] targeted `npm run lint` / `npm run typecheck` for affected Next host
- [ ] targeted build/smoke, if runtime wiring changed

## Open Blockers

- [ ] Record runtime-only blockers separately and briefly, do not turn this document into an endless backlog.
- [ ] When drift occurs between host docs and module docs, first fix the local docs of the owning component.

## Related Documents

- [UI README](../UI/README.md)
- [GraphQL and Leptos server functions](../UI/graphql-architecture.md)
- [Storefront](../UI/storefront.md)
- [i18n Architecture](../architecture/i18n.md)
- [Main Verification README](./README.md)
