# План реализации `rustok-auth`

Статус: core baseline зафиксирован; модуль возвращён в обязательный
manifest/doc contract path.

## Область работ

- удерживать `rustok-auth` как чистый core capability module без собственного UI;
- синхронизировать runtime permission surface, local docs и manifest metadata;
- не возвращать auth business logic обратно в `apps/server`.

## Текущее состояние

- `AuthModule` зарегистрирован как обязательный core-модуль;
- JWT, claims и credential helpers живут внутри модуля;
- root `README.md`, local docs и `rustok-module.toml` входят в обязательный acceptance contract;
- permission surface `users:*` публикуется через `RusToKModule::permissions()`.

## Этапы

### 1. Contract stability

- [x] вернуть `rustok-module.toml` и локальные module docs в scoped audit path;
- [x] выровнять root README с обязательными разделами и ссылкой на local docs;
- [ ] удерживать sync между runtime permission surface и server integration tests.

### 2. Integration hardening

- [ ] не выносить auth lifecycle logic в host-слой без обновления module contract;
- [ ] расширять token/config surface только вместе с local docs и runtime tests;
- [ ] явно документировать новые auth-owned flows до их публикации в host runtime.

## Проверка

- `cargo xtask module validate auth`
- `cargo xtask module test auth`
- targeted auth/RBAC server tests при изменении runtime wiring

## Правила обновления

1. При изменении token lifecycle или permission surface сначала обновлять этот файл.
2. При изменении public/runtime contract синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
