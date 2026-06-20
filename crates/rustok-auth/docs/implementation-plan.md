# План реализации `rustok-auth`

Статус: core baseline зафиксирован; модуль возвращён в обязательный
manifest/doc contract path.

## Execution checkpoint

- Current phase: integration_hardening
- Last checkpoint: Invite token issuance added to the auth-owned JWT surface with strict purpose validation, server bridge export, local docs, and unit-test coverage.
- Next step: Continue reducing host-only auth lifecycle logic by moving the next token/config primitive behind `rustok-auth` helpers with matching docs and tests.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок.
- Last updated at (UTC): 2026-06-20T00:00:00Z

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
- [x] удерживать sync между runtime permission surface и server integration tests (`AUTH_USER_PERMISSIONS` + server registry/GraphQL contract checks).

### 2. Integration hardening

- [ ] не выносить auth lifecycle logic в host-слой без обновления module contract;
- [x] расширять token/config surface только вместе с local docs и runtime tests;
- [x] явно документировать новые auth-owned flows до их публикации в host runtime.

## Проверка

- `cargo xtask module validate auth`
- `cargo xtask module test auth`
- targeted auth/RBAC server tests при изменении runtime wiring

## Правила обновления

1. При изменении token lifecycle или permission surface сначала обновлять этот файл.
2. При изменении public/runtime contract синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [x] Проверить полноту и актуальность `README.md` и локальных docs для permission surface sync.
- [x] Зафиксировать/обновить verification gates для текущего состояния модуля (без запуска компиляции в этом инкременте по запросу).
