# План реализации `rustok-core`

Статус: foundation crate служит shared contract layer; выполнен sweep boundary hardening — вычищен domain-specific auth logic, синхронизированы docs и public surface.

## Область работ

- удерживать `rustok-core` как минимально необходимый shared foundation layer;
- синхронизировать typed primitives, validation/security contracts и local docs;
- не допускать превращения `rustok-core` в свалку host- или domain-owned логики.

## Текущее состояние

- crate используется как базовая зависимость для platform и domain modules;
- shared typed contracts и foundation helpers являются частью live surface;
- другие модули строят свои integration contracts поверх `rustok-core`, не размазывая базовые типы по workspace;
- **boundary hardening**: auth module (user entity, repository, service, migrations) удалён из `rustok-core` — canonical auth lifecycle живёт в `rustok-auth`;
- **contract sync**: `CRATE_API.md`, `README.md`, `docs/README.md` синхронизированы с актуальным public surface;
- **deps cleanup**: удалены `jsonwebtoken` и `argon2` из `Cargo.toml` (больше не нужны после удаления auth);
- **targeted tests**: добавлен `tests/foundation_primitives.rs` с coverage для `UserRole`/`UserStatus` (display, parse, serde), `generate_id`/`parse_id`, locale normalization и field-schema guardrails;
- **security/validation tests**: добавлен `tests/security_validation.rs` с coverage для `SecurityHeaders`, `RateLimiter`, `InputValidator`, `SsrfProtection` и utils (`is_valid_email`, `is_valid_uuid`, `html_escape`, `slugify`);
- **contract tests**: расширен `tests/contract_surface.rs` проверками на отсутствие auth re-exports и лишних auth-зависимостей в `Cargo.toml`;
- local docs и root `README.md` удерживаются как часть scoped audit path.

## Этапы

### 1. Contract stability

- [x] закрепить `rustok-core` как shared foundation layer;
- [x] удерживать typed primitives и shared helpers вне host/domain buckets;
- [x] удерживать sync между public surface, compatibility exports и module metadata.

### 2. Boundary hardening

- [x] продолжать вычищать domain-specific logic из foundation layer;
- [x] переносить shared primitives сюда только при реальной cross-module необходимости;
- [x] покрывать новые foundation contracts targeted tests и compatibility checks.

### 3. Operability

- [x] документировать изменения foundation contracts одновременно с изменением runtime surface;
- [x] удерживать local docs и `README.md` синхронизированными;
- [x] обновлять consumer-module docs, если меняются базовые typed contracts.

## Проверка

- контрактные тесты покрывают все публичные use-case
- `cargo xtask module validate core`
- `cargo xtask module test core`
- targeted tests для primitives, validation, security и compatibility exports

## Правила обновления

1. При изменении foundation contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
4. При изменении shared contracts обновлять связанные consumer docs там, где это влияет на live behavior.
