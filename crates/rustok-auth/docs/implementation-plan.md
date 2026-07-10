# Implementation plan for `rustok-auth`

## Current state

`rustok-auth` is the mandatory core capability for JWT, claims, credential and
token configuration, lifecycle, OAuth, users, permissions (including
`AUTH_USER_PERMISSIONS`), and their public REST DTOs. `apps/server` supplies
persisted adapters, schema composition, and HTTP extraction only; it must not
regain auth business logic.

The admin surface is owned by `rustok-auth/admin` and follows the
core/transport/UI split. `AuthLifecyclePort`, `UserAdminMutationPort`, and
`OAuthAdminPort` are module-owned. Native adapters receive DB and runtime
extensions through `HostRuntimeContext`, while GraphQL and REST use the same
typed runtime contracts. The package consumes the host effective locale and
does not create a package-local locale fallback.

## FFA/FBA boundary

- FFA status: `phase_b_ready`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- FBA registry and static/runtime evidence:
  `crates/rustok-auth/contracts/auth-fba-registry.json`,
  `crates/rustok-auth/contracts/evidence/auth-capability-static-matrix.json`,
  and `crates/rustok-auth/contracts/evidence/auth-runtime-fallback-smoke.json`.

## Open results

1. **Capture runtime parity evidence for user and OAuth mutations.** Exercise
   the browser/admin path and the owner-owned GraphQL/native paths for the same
   successful and rejected operations.
   **Depends on:** an environment with persisted lifecycle/OAuth adapters and
   test identities.
   **Done when:** reproducible evidence covers tenant scope, RBAC, canonical
   error mapping, and host-resolved locale propagation; only then consider a
   `parity_verified` promotion.

2. **Preserve boundary parity as auth flows evolve.** Add or change token,
   credential, OAuth, or user-management behavior only through the typed module
   ports and published REST/GraphQL contracts.
   **Depends on:** the change-owning public contract.
   **Done when:** the module README, metadata, and FFA/FBA evidence describe the
   same runtime surface without a server-local bypass.

## Verification

- `npm run verify:auth:admin-boundary`
  (`scripts/verify/verify-auth-admin-boundary.mjs`)
- `npm run verify:ai:fba-baseline`
- `cargo xtask module validate auth`
- `cargo xtask module test auth`
- `cargo check -p rustok-auth-admin`
- Targeted auth/RBAC server tests when runtime wiring changes.

## Change rules

1. Keep auth lifecycle, OAuth, user mutation, and permission policy in the
   owning module.
2. Update the root README, local docs, and `rustok-module.toml` with a public
   or metadata change.
3. Update this status block and `docs/modules/registry.md` in the same change
   when the UI or transport boundary changes.
