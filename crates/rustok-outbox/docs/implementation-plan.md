# План реализации `rustok-outbox`

Статус: core outbox baseline зафиксирован; модуль приведён к единому
manifest/doc contract.

## Область работ

- удерживать `rustok-outbox` как bounded-context модуль transactional publishing;
- синхронизировать relay/runtime contract, local docs и manifest metadata;
- развивать operational guarantees без размазывания event runtime contract по host-слою.

## Текущее состояние

- write-side transactional publishing contract уже реализован;
- relay/retry/DLQ semantics уже входят в базовый runtime surface;
- модуль публикует admin visibility через `rustok-outbox-admin`;
- root README, local docs и manifest contract входят в scoped audit path.

## Этапы

### 1. Contract stability

- [x] выровнять root README, local docs и manifest metadata под единый standard path;
- [x] зафиксировать transactional publishing как основной bounded-context contract;
- [ ] удерживать sync между public crate API и server event-runtime tests.

### 2. Runtime hardening

- [ ] расширить automated tests вокруг relay/backlog/DLQ boundary behavior;
- [ ] документировать новые runtime guarantees вместе с изменениями event transport contract;
- [ ] держать observability и operability частью delivery readiness, а не постфактум.

### 3. Productionization

- [ ] уточнить rollout и migration strategy для incremental adoption;
- [ ] завершить security/tenancy/rbac checks, которые реально относятся к модулю;
- [ ] удерживать incident runbook в sync с operational semantics.

## Проверка

- `cargo xtask module validate outbox`
- `cargo xtask module test outbox`
- targeted event-runtime tests для transactional publish, relay, retry и DLQ semantics

## Правила обновления

1. При изменении transactional publishing или relay contract сначала обновлять этот файл.
2. При изменении public/runtime contract синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata и UI wiring синхронизировать `rustok-module.toml`.
