# Admin App Implementation Plan

## Current Contract

`apps/admin` is an FFA-compatible Leptos composition host. It owns shell, routing,
host-level operator screens and cross-module composition, while module business UI
belongs in owner packages under `crates/rustok-*/admin`.

The live host structure is:

- `app/` for app wiring, generated module registry and providers;
- `widgets/` for host shell widgets with portable policy in `core.rs`;
- `features/` for host-owned operator features and cross-module composition;
- `entities/` for host-local read models;
- `shared/` for host shared transport, UI and context helpers.

Host-owned features use explicit model/transport boundaries:

- `features/workflow/model.rs` and `features/workflow/transport/`;
- `features/oauth_apps/model.rs` and `features/oauth_apps/transport/`;
- `features/installer/model.rs` and `features/installer/transport/`;
- `features/modules/transport/` for module control-plane transport.

Pages and components call public transport facades only. Raw GraphQL, REST and
native `#[server]` functions stay behind those transport boundaries.

## Active Work

- Keep host FFA guardrails current in `scripts/verify/verify-frontend-host-ffa-contract.mjs`.
- Keep `apps/admin/docs/README.md` synchronized with host-owned feature boundaries.
- Keep module-owned UI out of `apps/admin/src/features/`, except host composition and
  platform operator surfaces.
- Keep GraphQL and native `#[server]` paths in parallel where a surface is
  public/headless-capable.
- Keep locale propagation host-owned; module UI receives effective locale from
  host context and must not add local cookie/header/query fallback chains.

## Open Improvement Areas

- Add route-level and action-level permission checks where a host screen still
  relies only on backend rejection.
- Add UX flow metrics for critical admin actions, failures and latency.
- Propagate correlation ids through host transport helpers where backend surfaces
  expose them.
- Expand focused component and contract tests for host operator features.
- Keep Leptos admin and Next admin behavior aligned for loading, empty, error and
  permission-gated states.

## Verification

For host FFA changes, run:

```powershell
cargo fmt --manifest-path apps\admin\Cargo.toml --check
cargo check --manifest-path apps\admin\Cargo.toml --lib -j 1
node scripts\verify\verify-frontend-host-ffa-contract.mjs
node scripts\verify\verify-workflow-admin-boundary.mjs
git diff --check
```

When touching module-owned packages mounted by this host, also run the relevant
module verifier and `cargo xtask module validate <slug>`.
