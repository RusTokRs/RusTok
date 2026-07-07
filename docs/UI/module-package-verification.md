---
id: doc://docs/UI/module-package-verification.md
kind: project_overview
language: markdown
status: active
---

# Module UI Package Verification Guide

Run these checks after any change to a Leptos UI package, host app wiring, or i18n files.

For the code rules being verified, see
[Implementation Guide](./module-package-implementation.md).
For the architectural context, see
[Architecture Guide](./module-package-architecture.md).

---

## Minimum Checks for Any Module UI Change

Run in this order:

```powershell
# 1. Validate module manifest, sub-crate existence, and wiring
cargo xtask module validate <slug>

# 2. Run module-scoped tests
cargo xtask module test <slug>
```

Replace `<slug>` with the module slug from `rustok-module.toml`, e.g. `blog`, `pages`,
`product`, `forum`, `search`, `commerce`, `cart`, `pricing`, `region`, `workflow`.

---

## After i18n / Locale File Changes

```powershell
# Verify locale key parity between Leptos and Next.js stacks
npm run verify:i18n:ui

# Verify locale contract wiring (manifest declarations, bundle loading)
npm run verify:i18n:contract
```

Run both when:
- Adding or renaming keys in `admin/locales/*.json` or `storefront/locales/*.json`
- Adding a new locale file
- Changing `[provides.admin_ui.i18n]` in `rustok-module.toml`

---

## After Host App Wiring Changes (`apps/admin`, `apps/storefront`)

```powershell
# Verify FFA host contract (navigation policy in core.rs, render in leptos adapter)
npm run verify:frontend:host-ffa-contract

# Verify storefront module route contract
npm.cmd run verify:storefront:routes
```

Run when:
- Adding or changing a mounted module surface in host `build.rs`
- Modifying `src/widgets/app_shell/` or `src/widgets/header/`
- Adding host-level route segments

---

## Architecture Guard (FFA Structure)

Checks that `core.rs` / `core/` contain no `leptos::*` imports, that UI adapters do not
call raw transport internals, and other structural rules.

```powershell
# On Windows
powershell -ExecutionPolicy Bypass -File scripts/verify/verify-architecture.ps1

# FFA migration guardrails batch (if the module is part of the rollout)
npm run test:verify:ffa:ui:migration
```

Run when:
- Adding new files to `core.rs`, `core/`, or `ui/leptos.rs`
- Touching `transport/` boundaries
- Adding new dependencies to a module UI crate

---

## Auth Admin Boundary (for `rustok-auth-admin`)

```powershell
npm run verify:auth:admin-boundary
```

---

## Forum Storefront Boundary (for `rustok-forum-storefront`)

```powershell
npm run verify:forum:storefront-boundary
```

---

## Full Pre-Commit Sequence for Module UI Work

```powershell
cargo xtask module validate <slug>
cargo xtask module test <slug>
npm run verify:i18n:ui
npm run verify:i18n:contract
npm run verify:frontend:host-ffa-contract
powershell -ExecutionPolicy Bypass -File scripts/verify/verify-architecture.ps1
```

---

## What Each Check Catches

| Check | Catches |
|---|---|
| `module validate <slug>` | Missing `Cargo.toml`, manifest/sub-crate mismatch, version drift |
| `module test <slug>` | Broken transport adapters, failed unit/integration tests |
| `verify:i18n:ui` | Key drift between `admin/locales/` and `apps/next-admin/messages/` |
| `verify:i18n:contract` | Undeclared locale bundles, missing manifest wiring |
| `verify:frontend:host-ffa-contract` | Navigation/header policy leaking into Leptos render adapter |
| `verify:storefront:routes` | Broken or missing storefront module route segments |
| `verify-architecture.ps1` | `leptos::*` in `core`, UI calling raw transport internals |
| `test:verify:ffa:ui:migration` | FFA structural shape regressions across all rollout modules |

---

## Common Errors and Fixes

**`leptos` dependency found in core module`**
— Remove `use leptos::*` or `use leptos_router::*` from `core.rs`. Move the offending
logic to `ui/leptos.rs` or extract a transport-neutral type into `model.rs`.

**`i18n key not found in messages/<lang>.json`**
— Add the missing key to both `admin/locales/en.json` and `apps/next-admin/messages/en.json`
(and `ru.json`). Keys must be nested identically in both files.

**`ui adapter calls graphql_adapter directly`**
— Replace the direct call with a call to the facade in `transport/mod.rs`. The UI layer
must not know which adapter was selected.

**`module manifest declares leptos_crate but admin/Cargo.toml not found`**
— Either create `admin/Cargo.toml` with the matching crate name and version, or remove
`[provides.admin_ui]` from `rustok-module.toml` if the UI is not ready yet.

**CSR build fails with `server function not available`**
— A `#[server]` call is being made in the CSR/headless profile instead of the GraphQL selected path. Wrap
the native call in `#[cfg(feature = "ssr")]` and implement `graphql_adapter.rs` for the
same operation.

---

## Related Documents

| Document | Purpose |
|---|---|
| [Architecture Guide](./module-package-architecture.md) | Why the structure is this way |
| [Implementation Guide](./module-package-implementation.md) | Code rules and patterns |
| [FFA Migration Plan](../research/dioxus-ffa-ui-migration-plan.md) | Phase-gate criteria |
| [FFA Parity Checklist](../verification/ffa-ui-parity-checklist.md) | Phase-gate evidence tracker |
| [Module UI Quickstart](../modules/UI_PACKAGES_QUICKSTART.md) | Step-by-step for new packages |
