# План реализации `rustok-auth`

Статус: core baseline зафиксирован; UI модулирован по FFA в `crates/rustok-auth/admin`.

## Execution checkpoint

- Current phase: auth_admin_core_policy_hardening
- Last checkpoint: Login, register and password-reset callbacks now consume typed core requests; tenant/email/name normalization and whitespace-only validation live in `admin/src/core.rs`, passwords remain byte-for-byte unchanged, and profile name/error classification policy is core-owned. Focused unit coverage reached 12 tests and the auth boundary guard forbids request normalization and profile error classification from returning to Leptos files.
- Next step: Audit direct `leptos-auth` hook calls and remaining UI-owned state/policy against the Phase B checklist; introduce a module-owned facade only where it reduces coupling without wrapping hooks mechanically, then decide whether auth can move to `phase_b_ready`.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок.
- Last updated at (UTC): 2026-06-29T00:00:00Z

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

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- Evidence: auth admin UI pages (Login, Register, ResetPassword, Profile, Security, Users, UserDetails, and OAuthAppsPage) are fully relocated to `crates/rustok-auth/admin`. `admin/src/ui/leptos.rs` is the explicit Leptos aggregation adapter, while password-reset dispatch goes through the module-owned `admin/src/transport/mod.rs` facade instead of a raw UI API call. The package implements its own client-side and server-side request/response models and translation lookup catalog matching `UiRouteContext.locale`. Users list/query policy, user and OAuth create/update request preparation, OAuth multiline/default/timestamp policy, typed login/register/reset request normalization, profile name normalization and profile transport-error classification now live in Leptos-free `admin/src/core.rs`; OAuth transport input DTOs live in framework-neutral `admin/src/model.rs`. Twelve focused unit tests cover these policies, while `scripts/verify/verify-auth-admin-boundary.mjs` prevents UI-owned DTO reconstruction/normalization and is wired into `npm run verify:ffa:ui:migration`. The package `ssr` feature explicitly enables `rustok-api/server`, so native adapters compile with canonical auth/tenant contexts. Host application `apps/admin` integrates the module pages dynamically via routing. FBA capability metadata is locked in `crates/rustok-auth/contracts/auth-fba-registry.json` for `AUTH_USER_PERMISSIONS` and auth-owned admin boundary contracts, mirrored by `crates/rustok-auth/contracts/evidence/auth-capability-static-matrix.json` and source-smoke `crates/rustok-auth/contracts/evidence/auth-runtime-fallback-smoke.json`, and checked by `scripts/verify/verify-ai-fba-baseline.mjs`.

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
