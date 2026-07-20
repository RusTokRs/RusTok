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

1. **Move OAuth application display content to the multilingual database
   contract.** `oauth_apps.name` and `oauth_apps.description` are currently
   inline user-facing strings. Introduce an auth-owned translation table with a
   canonical default locale, tenant-composite integrity, and parity across REST,
   GraphQL, native admin transport, and both admin hosts. Do not add a JSON or
   transport-only translation fallback.
   **Depends on:** the database multilingual contract and an atomic transport/UI
   cutover.
   **Done when:** storage, reads, writes, fallback behavior, migration evidence,
   and all auth-owned transports use the same translation contract.

2. **Capture runtime parity evidence for user and OAuth mutations.** Exercise
   the browser/admin path and the owner-owned GraphQL/native paths for the same
   successful and rejected operations.
   **Depends on:** an environment with persisted lifecycle/OAuth adapters and
   test identities.
   **Done when:** reproducible evidence covers tenant scope, RBAC, canonical
   error mapping, and host-resolved locale propagation; only then consider a
   `parity_verified` promotion.

3. **Preserve boundary parity as auth flows evolve.** Add or change token,
   credential, OAuth, or user-management behavior only through the typed module
   ports and published REST/GraphQL contracts.
   **Depends on:** the change-owning public contract.
   **Done when:** the module README, metadata, and FFA/FBA evidence describe the
   same runtime surface without a server-local bypass.

4. **Provide bounded identity reads for owner-owned operations.**
   `AuthUserBackfillReadPort` exposes only tenant-scoped user id, email and
   display-name data in creation order for profile provisioning. The
   host-independent `AuthUserBackfillDbReader` implements that port from an
   explicit database handle, while the server provider delegates to it.
   **Done when:** the selected CLI composition resolves the auth port without
   importing server models or expanding the profile domain with auth storage.

5. **Keep OAuth bootstrap in the auth-owned CLI adapter.**
   `rustok-cli oauth create-app` creates the development application through
   `rustok-auth/cli`, an explicit database handle, and the tenant-owned default
   tenant read. The server does not register a task for this operation.

6. **Keep session maintenance in the auth-owned CLI adapter.**
   `rustok-cli auth sessions-cleanup` removes expired auth sessions without the
   server task bridge.

7. **Keep bootstrap identity provisioning in the auth owner.**
   `AuthUserBootstrapDbWriter` provides idempotent tenant-scoped user creation
   from an explicit database handle for installer and future standalone seed
   composition. RBAC role assignment remains a separate owner boundary.

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

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `in_progress`
- Last verified at (UTC): `2026-07-20`
- Scope inspected: `auth ownership; HS256/RS256 JWT configuration and issuer/audience validation; credential-bound password reset; sessions; OAuth apps, code exchange, refresh rotation, consent, scopes and revocation; tenant-composite migrations and queries; RBAC durable-generation invalidation; multilingual database contract; REST/GraphQL/native/server composition`
- Findings: `P0=0, P1=4, P2=1, P3=1`
- Fixed in this pass: `added fail-closed tenant-composite OAuth and invite database integrity plus tenant-qualified consent queries; deleted the unbound replayable password-reset path; made RS256 configuration parse and prove its key pair at startup; replaced middleware-shadowed legacy OAuth token handlers with one direct transactional service and deleted superseded issuance methods; made app/consent token revocation atomic`
- Remaining risks or blockers: `P2 OAuth app name/description still require the planned translation-table cutover; PostgreSQL forward/down migration smoke remains part of the closing migration gate; browser/runtime mutation parity evidence remains an existing promotion requirement`
- Evidence: `auth boundary, AI FBA, and runtime-context guards pass; module validate auth passes; rustok-auth unit/migration/JWT suite 34/34 passes; targeted server refresh-rotation build/test is running`
- Next action: `finish the targeted server test, run the canonical auth module test, then hand off to core/cache`
- Resume command: `$env:CARGO_TARGET_DIR='D:\RusTok\target\codex-auth-cycle'; cargo test -p rustok-server refresh_rotation_consumes_a_token_exactly_once --lib`
