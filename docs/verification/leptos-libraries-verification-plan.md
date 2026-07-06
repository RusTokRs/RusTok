---
id: doc://docs/verification/leptos-libraries-verification-plan.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Platform Verification Plan: Leptos Libraries

- **Status:** current detailed checklist
- **Scope:** shared Leptos libraries, host integration, module-owned UI packages, reusable UI/tooling layer
- **Companion plan:** [Frontend Surface Verification Plan](./platform-frontend-surfaces-verification-plan.md)

---

## Current Leptos Library Contract

This plan confirms that the library UI loop remains consistent
between reusable Leptos crates, host applications and module-owned UI surfaces.

Verification relies on the current-state contract:

- Leptos hosts mount UI surfaces but do not override module ownership
- reusable libraries remain common building blocks, not a hidden application layer
- internal Leptos data path uses `#[server]` as the default internal layer
- GraphQL is preserved as a parallel transport contract
- effective locale comes from the host/runtime layer, not from a package-local fallback chain

## Verification Scope

### Shared Leptos crates

- [ ] `crates/leptos-auth`
- [ ] `crates/leptos-forms`
- [ ] `crates/leptos-zustand`
- [ ] `crates/leptos-graphql`
- [ ] `crates/leptos-shadcn-pagination`
- [ ] `crates/leptos-ui`
- [ ] `crates/leptos-zod`
- [ ] `crates/leptos-table`
- [ ] `crates/leptos-hook-form`

### Host consumers

- [ ] `apps/admin`
- [ ] `apps/storefront`

## Phase 1. Public Library Contract

### 1.1 Root README and local docs

- [ ] Each library maintains an up-to-date `README.md` with `Purpose`, `Responsibilities`, `Entry points`, `Interactions`.
- [ ] Local docs inside `crates/leptos-*` do not diverge from the actual public contract.
- [ ] The library explicitly documents where the reusable layer ends and host/module-owned logic begins.

### 1.2 Ownership boundary

- [ ] Reusable Leptos crates are not disguised as module-owned UI packages.
- [ ] Libraries do not introduce their own auth/locale/runtime contract on top of host policy.
- [ ] App-specific scenarios are not embedded in shared crates as hidden dependencies on a specific host.

## Phase 2. Host Integration

### 2.1 `apps/admin`

- [ ] `apps/admin` uses Leptos libraries as building blocks, not as a container for bypassing module-owned UI.
- [ ] `UiRouteContext`, effective locale and module route base remain host-provided.
- [ ] `#[server]` and GraphQL integration do not diverge from the current UI/runtime contract.

### 2.2 `apps/storefront`

- [ ] `apps/storefront` uses shared Leptos libraries without duplicating storefront-specific business logic in shared crates.
- [ ] Storefront route/locale contract matches `docs/UI/storefront.md` and `docs/architecture/i18n.md`.
- [ ] Library-level abstractions do not replace module-owned storefront packages.

## Phase 3. Data Layer and Transport Contract

### 3.1 `#[server]` and GraphQL

- [ ] Leptos libraries do not break the rule: `#[server]` as default internal layer, GraphQL as parallel contract.
- [ ] Shared crates do not hardcode host-specific transport assumptions.
- [ ] Library APIs do not create a second source of truth for fetching/mutations on top of the server contract.

### 3.2 i18n and runtime context

- [ ] Shared packages do not introduce a package-local locale negotiation chain.
- [ ] Locale, tenant and auth context come from the host/runtime layer.
- [ ] UI libraries do not diverge from manifest/module wiring and host route context.

## Phase 4. Bypass and Drift Checks

### 4.1 Bypass patterns

- [ ] In `apps/admin` and `apps/storefront` there are no systematic bypass implementations on top of shared Leptos contracts.
- [ ] If a bypass temporarily exists, it is documented locally and does not replace the library contract.
- [ ] New reusable functionality is added to a shared crate, not scattered across host apps.

### 4.2 Documentation drift

- [ ] `docs/UI/README.md`, `docs/UI/graphql-architecture.md`, `docs/UI/storefront.md` are consistent with the current library layer.
- [ ] Local docs of applications and libraries describe the same integration contract.

## Targeted Local Checks

- [ ] targeted `cargo check` / `cargo test` for affected `crates/leptos-*`
- [ ] targeted `cargo check` / `cargo test` for `apps/admin` and `apps/storefront`, if the host integration path changed
- [ ] `npm run verify:i18n:ui`, if shared locale/UI contracts changed
- [ ] `npm run verify:i18n:contract`, if locale/runtime contract changed
- [ ] targeted UI smoke, if route wiring or shared rendering contract changed

## Open Blockers

- [ ] Do not turn this document into a weekly status table and backlog of workarounds.
- [ ] Runtime-only blockers should be recorded briefly and separately from the library contract.
- [ ] When drift occurs between shared crate and host app, first fix the owning docs and public contract, rather than accumulating exceptions.

## Related Documents

- [Frontend Surface Verification](./platform-frontend-surfaces-verification-plan.md)
- [UI README](../UI/README.md)
- [GraphQL and Leptos server functions](../UI/graphql-architecture.md)
- [Storefront](../UI/storefront.md)
- [i18n Architecture](../architecture/i18n.md)
- [Leptos Admin Documentation](../../apps/admin/docs/README.md)
- [Leptos Storefront Documentation](../../apps/storefront/docs/README.md)
