# План реализации `rustok-email`

Статус: core delivery baseline зафиксирован; модуль возвращён в обязательный
manifest/doc contract path.

## Execution checkpoint

- Current phase: plan_sync
- Last checkpoint: Initial bootstrap by registry workflow.
- Next step: Синхронизировать план с текущим кодом и выбрать первый незавершённый пункт.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок.
- Last updated at (UTC): 2026-05-20T00:00:00Z

## Область работ

- удерживать `rustok-email` как capability-only core module без собственного UI;
- синхронизировать SMTP/rendering contract, local docs и manifest metadata;
- не размывать границу между email delivery и host-level authorization logic.

## Текущее состояние

- `EmailModule` зарегистрирован как обязательный core-модуль;
- SMTP transport, template rendering и typed email helpers живут внутри модуля;
- root `README.md`, local docs и `rustok-module.toml` входят в scoped audit path;
- RBAC остаётся в вызывающем модуле или host runtime, а не в `rustok-email`.

## Этапы

### 1. Contract stability

- [x] вернуть `rustok-module.toml` и local docs в module standard path;
- [x] зафиксировать capability-only статус и отсутствие собственного UI;
- [ ] удерживать sync между delivery contract и host integration tests.

### 2. Integration hardening

- [ ] расширять typed email payloads только вместе с local docs и host tests;
- [ ] не переносить SMTP/rendering logic обратно в `apps/server`;
- [ ] документировать новые delivery flows до их публикации в host runtime.

## Проверка

- `cargo xtask module validate email`
- `cargo xtask module test email`
- targeted host tests для auth/email delivery flows при изменении runtime wiring

## Правила обновления

1. При изменении SMTP/rendering contract сначала обновлять этот файл.
2. При изменении public/runtime contract синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
