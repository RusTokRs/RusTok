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

1. **Remove implicit OAuth grant expansion for manifest-created applications.**
   `oauth_apps::Model::supports_grant_type` currently treats an auto-created
   application that declares only `authorization_code` as also supporting
   `refresh_token`. This is a compatibility execution path that expands the
   persisted grant policy instead of enforcing it literally.
   **Depends on:** updating any manifest producer that genuinely requires
   refresh rotation to declare `refresh_token` explicitly.
   **Done when:** grant checks use exact persisted membership, all required
   producers declare their complete grant set, and regression tests reject an
   undeclared refresh grant for both manual and auto-created applications.

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
   tenant read. Base application identity and localized display copy must be
   persisted in one transaction against the current translation-table schema.
   The server does not register a task for this operation.

6. **Keep session maintenance in the auth-owned CLI adapter.**
   `rustok-cli auth sessions-cleanup` removes expired auth sessions without the
   server task bridge.

7. **Keep bootstrap identity provisioning in the auth owner.**
   `AuthUserBootstrapDbWriter` provides idempotent tenant-scoped user creation
   from an explicit database handle for installer and future standalone seed
   composition. RBAC role assignment remains a separate owner boundary.

8. **Provide a lightweight owner-owned OAuth transaction regression target.**
   The current server-lib test target compiles embedded UI, storage, cloud, and
   unrelated domain composition before it can exercise OAuth code/refresh CAS.
   Move the persistence algorithm behind an auth-owned or narrowly shared adapter
   contract whose SQLite concurrency tests do not require the full server graph.
   **Done when:** authorization-code and refresh-token exactly-once tests run
   without compiling `apps/admin`, storefront hosts, cloud SDKs, or unrelated
   domain modules, while the server provider consumes the same tested boundary.

## Verification

- `npm run verify:auth:admin-boundary`
  (`scripts/verify/verify-auth-admin-boundary.mjs`)
- `npm run verify:ai:fba-baseline`
- `cargo xtask module validate auth`
- `cargo xtask module test auth`
- `cargo test -p rustok-auth-cli development_app_uses_current_translation_schema`
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
- Status: `blocked`
- Last verified at (UTC): `2026-07-29`
- Scope inspected: `auth ownership; JWT and credential lifecycle; OAuth app, authorization-code and refresh-token paths; tenant-qualified admin mutations; translation-table migration and runtime localization; auth-owned CLI bootstrap adapter; implicit grant expansion`
- Findings: `P0=0, P1=6, P2=0, P3=2`
- Fixed in this pass: `preserved the four previously repaired P1 defects; fixed the post-migration rustok-auth-cli OAuth bootstrap failure by deleting raw writes to removed oauth_apps.name/description columns and persisting the base row plus en translation in one transaction; added a SQLite regression against the current translation-table schema`
- Remaining risks or blockers: `P1 implicit refresh_token authority remains for auto-created applications whose persisted grant_types omit it; targeted Rust execution is unavailable in the current connector-only environment because local git clone failed DNS resolution and no workflow run exists for the branch yet; PostgreSQL forward/down migration smoke and browser/runtime mutation parity evidence remain required; lightweight owner-owned OAuth code/refresh regression target remains P3 verification debt`
- Evidence: `source inspection confirms tenant-qualified OAuth client, consent and admin lookups; authorization-code and refresh-token replacements use transactional compare-and-set; oauth_app_translations uses VARCHAR(32), tenant-composite FK and unique tenant/app/locale identity; branch agent/verify-core-auth-cycle-001 commits e4a7fb44902af0dd64e60516444958d440d21535 and 435e5f3a3b02311ca62eaeda5e1d1daef9f97b59 contain the CLI fix and regression; local clone failure was Could not resolve host: github.com and is classified as an environment limitation`
- Next action: `run the targeted rustok-auth-cli regression and canonical auth module checks; remove implicit refresh grant expansion with explicit producer updates; rerun PostgreSQL migration smoke; then revisit this blocked item during closing gates`
- Resume command: `cargo test -p rustok-auth-cli development_app_uses_current_translation_schema && cargo xtask module validate auth && cargo xtask module test auth`
