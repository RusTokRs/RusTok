# План реализации `rustok-auth`

Статус: core baseline зафиксирован; UI модулирован по FFA в `crates/rustok-auth/admin`.

## Execution checkpoint

- Current phase: phase_b_ready
- Last checkpoint: Production runtime extensions register `ServerAuthLifecycleProvider` behind `AuthLifecycleRuntime` and one `ServerAuthAdminMutationProvider` behind `OAuthAdminRuntime` and `UserAdminMutationRuntime`. Auth lifecycle and OAuth GraphQL query/mutation/types live in `rustok-auth`; `apps/server` only implements the persisted lifecycle/OAuth/email adapters and schema composition. OAuth and user native `#[server]` adapters consume the same typed ports. User custom-field validation, tenant scoping, RBAC, role replacement, atomic create metadata/localized-value lifecycle, host-resolved locale propagation and case-insensitive role/status normalization execute inside the shared provider instead of transport resolvers.
- Next step: Record browser/runtime parity evidence for the auth admin user and OAuth mutation flows before promoting to `parity_verified`.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок.
- Last updated at (UTC): 2026-06-30T14:19:18Z

## Область работ

- удерживать `rustok-auth-admin` как изолированный UI-пакет, инкапсулирующий все страницы авторизации и пользователей;
- синхронизировать runtime permission surface, local docs и manifest metadata;
- не возвращать auth business logic обратно в `apps/server`.

## Текущее состояние

- `AuthModule` зарегистрирован как обязательный core-модуль;
- JWT, claims, AuthConfig assembly/validation и credential helpers живут внутри модуля;
- root `README.md`, local docs и `rustok-module.toml` входят в обязательный acceptance contract;
- permission surface `users:*` публикуется через `RusToKModule::permissions()`.

## Этапы

### 1. Contract stability

- [x] вернуть `rustok-module.toml` и локальные module docs в scoped audit path;
- [x] выровнять root README с обязательными разделами и ссылкой на local docs;
- [x] удерживать sync между runtime permission surface и server integration tests (`AUTH_USER_PERMISSIONS` + server registry/GraphQL contract checks).

### 2. Integration hardening

- [x] не выносить auth lifecycle logic в host-слой без обновления module contract;
- [x] расширять token/config surface только вместе с local docs и runtime tests;
- [x] явно документировать новые auth-owned flows до их публикации в host runtime.
- [x] выделить админ-поверхности UI авторизации в отдельный crate `crates/rustok-auth/admin`.

## FFA/FBA status

- FFA status: `phase_b_ready`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- FBA registry/evidence: `crates/rustok-auth/contracts/auth-fba-registry.json`, `crates/rustok-auth/contracts/evidence/auth-capability-static-matrix.json`, `crates/rustok-auth/contracts/evidence/auth-runtime-fallback-smoke.json`.
- Evidence: auth admin UI pages are fully relocated to `crates/rustok-auth/admin` with Leptos-free core, module-owned transport facade and explicit `admin/src/ui/leptos.rs`. Focused admin unit tests and `scripts/verify/verify-auth-admin-boundary.mjs` lock request normalization, pagination/error policy, user/OAuth presentation mapping, host-locale landing-page copy, profile preference host-locale defaulting, absence of package-local locale storage fallback, shared provider registration, native-first mutation routing, host-resolved locale propagation into native user mutations, atomic user create custom-field persistence, shared provider role/status enum normalization and the absence of direct GraphQL lifecycle bypasses. Direct `leptos-auth` hook use remains only in UI adapters where it updates auth context signals/storage after sign-in, sign-up and sign-out; core stays framework-free. `rustok-auth/src/lifecycle.rs` defines `AuthLifecyclePort`; `rustok-auth/src/admin_mutations.rs` defines `UserAdminMutationPort` and the complete read/write/consent `OAuthAdminPort`. Production bootstrap registers `ServerAuthLifecycleProvider` and `ServerAuthAdminMutationProvider`; owner-owned auth lifecycle/OAuth GraphQL and native adapters consume typed runtimes with canonical error mapping, tenant scope and RBAC. FBA metadata/evidence remains locked by `npm run verify:ai:fba-baseline`; executable boundary evidence is `cargo test -p rustok-auth --lib`, `cargo test -p rustok-auth-admin --lib` and `npm run verify:auth:admin-boundary`.

## Проверка

- `cargo xtask module validate auth`
- `cargo xtask module test auth`
- targeted auth/RBAC server tests при изменении runtime wiring
- `cargo check -p rustok-auth-admin`
- `cargo check -p rustok-admin`
- `npm run verify:i18n:ui`
- `npm run verify:auth:admin-boundary`

## Правила обновления

1. При изменении token lifecycle или permission surface сначала обновлять этот файл.
2. При изменении public/runtime contract синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.


## Quality backlog

- [x] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [x] Проверить полноту и актуальность `README.md` и локальных docs для permission surface sync.
- [x] Зафиксировать/обновить verification gates для текущего состояния модуля.
- [x] Полностью разбить и вынести UI-слой авторизации в `rustok-auth-admin`.
