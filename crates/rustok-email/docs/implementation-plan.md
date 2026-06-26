# План реализации `rustok-email`

Статус: core delivery baseline зафиксирован; модуль возвращён в обязательный
manifest/doc contract path.

## Execution checkpoint

- Current phase: fba_contract_test_scaffolding
- Last checkpoint: Добавлены targeted Rust contract tests для shared write-policy mapping, disabled-provider noop receipt и typed request validation; static FBA evidence теперь указывает на test names, без запуска компиляции по ограничению итерации.
- Next step: Когда компиляции снова разрешены, запустить targeted `cargo test -p rustok-email ports::tests` и зафиксировать executable evidence/fallback smoke result.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок.
- Last updated at (UTC): 2026-06-26T00:00:00Z


## FFA/FBA status block

- FFA status: `not_started`
- FBA status: `in_progress`
- Structural shape: `no_ui_boundary`
- Evidence / notes:
  - capability-only module has no module-owned UI surface, so FFA remains `not_started`;
  - FBA provider slice: `crates/rustok-email/contracts/email-fba-registry.json` + `crates/rustok-email/src/ports.rs` declare `EmailDeliveryPort` / `email.delivery.v1` for transactional delivery consumers with shared `rustok_api::PortContext`/`PortError`, `PortCallPolicy::write()` deadline/idempotency semantics, disabled-provider noop preservation and static evidence packet `crates/rustok-email/contracts/evidence/email-contract-test-static-matrix.json` verified by `npm run verify:email:fba`; status remains below `boundary_ready` until the new targeted Rust tests are compiled/executed and executable runtime evidence is recorded.

## Область работ

- удерживать `rustok-email` как capability-only core module без собственного UI;
- синхронизировать SMTP/rendering contract, local docs и manifest metadata;
- не размывать границу между email delivery и host-level authorization logic.

## Текущее состояние

- `EmailModule` зарегистрирован как обязательный core-модуль;
- SMTP transport, template rendering, typed email helpers и email-owned delivery DTOs живут внутри модуля;
- root `README.md`, local docs и `rustok-module.toml` входят в scoped audit path;
- RBAC остаётся в вызывающем модуле или host runtime, а shared write-policy context/error baseline приходит из `rustok-api`, не перенося delivery business logic в shared слой.

## Этапы

### 1. Contract stability

- [x] вернуть `rustok-module.toml` и local docs в module standard path;
- [x] зафиксировать capability-only статус и отсутствие собственного UI;
- [x] добавить targeted contract tests для delivery port write-policy mapping и disabled noop fallback;
- [ ] выполнить targeted contract tests и удерживать sync между delivery contract и host integration tests.

### 2. Integration hardening

- [ ] расширять typed email payloads только вместе с local docs и host tests;
- [ ] не переносить SMTP/rendering logic обратно в `apps/server`;
- [ ] документировать новые delivery flows до их публикации в host runtime.

## Проверка

- `cargo xtask module validate email`
- `cargo xtask module test email`
- `cargo test -p rustok-email ports::tests` для targeted delivery-port contract tests
- targeted host tests для auth/email delivery flows при изменении runtime wiring

## Правила обновления

1. При изменении SMTP/rendering contract сначала обновлять этот файл.
2. При изменении public/runtime contract синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.


## Quality backlog

- [x] Актуализировать targeted coverage для delivery port policy/validation/noop receipt сценариев.
- [ ] Выполнить targeted coverage после снятия ограничения на компиляции.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
