# `rustok-auth` — Implementation Plan

Status: core baseline locked; UI modularized via FFA in `crates/rustok-auth/admin`.

## Execution checkpoint

- Current phase: server artifact cleanup and Loco-free auth admin runtime
- Last checkpoint: Production runtime extensions register `ServerAuthLifecycleProvider` behind `AuthLifecycleRuntime` and one `ServerAuthAdminMutationProvider` behind `OAuthAdminRuntime` and `UserAdminMutationRuntime`. Auth lifecycle and OAuth GraphQL query/mutation/types live in `rustok-auth`; auth, OAuth and users REST request/response DTOs and OpenAPI schema derives now live in `rustok-auth::rest`; `apps/server` only implements the persisted lifecycle/OAuth/email adapters, schema composition and HTTP route extraction/response mapping. OAuth and user native `#[server]` adapters consume the same typed ports. `rustok-auth-admin` now reads DB and `ModuleRuntimeExtensions` through `HostRuntimeContext`, removing its Loco dependency. User custom-field validation, tenant scoping, RBAC, role replacement, atomic create metadata/localized-value lifecycle, host-resolved locale propagation and case-insensitive role/status normalization execute inside the shared provider instead of transport resolvers.
- Next step: Record browser/runtime parity evidence for the auth admin user and OAuth mutation flows before promoting to `parity_verified`.
- Open blockers: None.
- Hand-off notes for next agent: Update this block after each increment.
- Last updated at (UTC): 2026-07-10T00:00:00Z

## Scope of work

- maintain `rustok-auth-admin` as an isolated UI package encapsulating all auth and user pages;
- synchronize runtime permission surface, local docs and manifest metadata;
- do not move auth business logic back into `apps/server`.

## Current state

- `AuthModule` is registered as a mandatory core module;
- JWT, claims, AuthConfig assembly/validation and credential helpers live inside the module;
- auth, OAuth and users REST DTOs for login/register/refresh/logout/invite/reset/verification/profile/password/session/user-list/user-detail/token/authorize/consent/browser-session/revoke flows live in `crates/rustok-auth/src/rest.rs`; server controllers re-export or import them only for existing Swagger and route paths;
- root `README.md`, local docs and `rustok-module.toml` are part of the mandatory acceptance contract;
- permission surface `users:*` is published through `RusToKModule::permissions()`.

## Stages

### 1. Contract stability

- [x] return `rustok-module.toml` and local module docs to the scoped audit path;
- [x] align root README with mandatory sections and link to local docs;
- [x] maintain sync between runtime permission surface and server integration tests (`AUTH_USER_PERMISSIONS` + server registry/GraphQL contract checks).

### 2. Integration hardening

- [x] do not move auth lifecycle logic to the host layer without updating the module contract;
- [x] expand token/config surface only together with local docs and runtime tests;
- [x] explicitly document new auth-owned flows before publishing them in the host runtime.
- [x] extract auth UI admin surfaces into a separate crate `crates/rustok-auth/admin`.

## FFA/FBA status

- FFA status: `phase_b_ready`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- FBA registry/evidence: `crates/rustok-auth/contracts/auth-fba-registry.json`, `crates/rustok-auth/contracts/evidence/auth-capability-static-matrix.json`, `crates/rustok-auth/contracts/evidence/auth-runtime-fallback-smoke.json`.
- Evidence: auth admin UI pages are fully relocated to `crates/rustok-auth/admin` with Leptos-free core, module-owned transport facade and explicit `admin/src/ui/leptos.rs`. Focused admin unit tests and `scripts/verify/verify-auth-admin-boundary.mjs` lock request normalization, pagination/error policy, user/OAuth presentation mapping, host-locale landing-page copy, profile preference host-locale defaulting, absence of package-local locale storage fallback, shared provider registration, build-profile-selected native mutation routing, host-resolved locale propagation into native user mutations, atomic user create custom-field persistence, shared provider role/status enum normalization, owner-owned auth/OAuth/users REST DTO/OpenAPI schema placement and the absence of direct GraphQL lifecycle bypasses. Native adapters now receive DB and `Arc<ModuleRuntimeExtensions>` via `HostRuntimeContext`; `apps/server` supplies that typed host handle during server-function composition, without exposing its Loco shared store. Direct `leptos-auth` hook use remains only in UI adapters where it updates auth context signals/storage after sign-in, sign-up and sign-out; core stays framework-free. `rustok-auth/src/lifecycle.rs` defines `AuthLifecyclePort`; `rustok-auth/src/admin_mutations.rs` defines `UserAdminMutationPort` and the complete read/write/consent `OAuthAdminPort`; `rustok-auth/src/rest.rs` defines REST DTOs consumed/re-exported by the server HTTP adapter. Production bootstrap registers `ServerAuthLifecycleProvider` and `ServerAuthAdminMutationProvider`; owner-owned auth lifecycle/OAuth GraphQL and native adapters consume typed runtimes with canonical error mapping, tenant scope and RBAC. FBA metadata/evidence remains locked by `npm run verify:ai:fba-baseline`; executable boundary evidence is `cargo test -p rustok-auth --lib`, `cargo test -p rustok-auth-admin --lib` and `npm run verify:auth:admin-boundary`.

## Verification

- `cargo xtask module validate auth`
- `cargo xtask module test auth`
- targeted auth/RBAC server tests when changing runtime wiring
- `cargo check -p rustok-auth-admin`
- `cargo check -p rustok-admin`
- `npm run verify:i18n:ui`
- `npm run verify:auth:admin-boundary`

## Update rules

1. When changing token lifecycle or permission surface, first update this file.
2. When changing public/runtime contract, synchronize `README.md` and `docs/README.md`.
3. When changing module metadata, synchronize `rustok-module.toml`.


## Quality backlog

- [x] Update test coverage for key module scenarios.
- [x] Verify completeness and relevance of `README.md` and local docs for permission surface sync.
- [x] Lock/update verification gates for the current module state.
- [x] Fully split and extract the auth UI layer into `rustok-auth-admin`.
