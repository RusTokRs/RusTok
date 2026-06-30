# План реализации `rustok-workflow`

Статус: workflow module уже имеет рабочий execution/runtime baseline; ключевая
задача сейчас — удерживать границу между orchestration layer, event contracts и
capability integrations без дрейфа и битой документации.

## Execution checkpoint

- Current phase: phase_b_ready + fba_provider_static_evidence + compile_free_runtime_smoke
- Last checkpoint: Workflow admin FFA Phase B считается закрытой; FBA slice #1 добавил `WorkflowReadPort` / `workflow.read_projection.v1`, provider registry `crates/rustok-workflow/contracts/workflow-fba-registry.json`, static matrix `crates/rustok-workflow/contracts/evidence/workflow-contract-test-static-matrix.json`, compile-free runtime/fallback smoke packet `crates/rustok-workflow/contracts/evidence/workflow-read-projection-runtime-smoke.json` и fast gate `npm run verify:workflow:fba` без long compilation.
- Next step: Заменить compile-free runtime/fallback smoke живым backend evidence: native server-function list_workflows, принудительный GraphQL fallback и typed PortError mapping; не повышать FBA выше `in_progress` до live evidence.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок; избегать долгих full-workspace компиляций, использовать targeted checks/timeouts.
- Last updated at (UTC): 2026-06-20T00:00:00Z


## FFA/FBA status

- FFA status: `phase_b_ready`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- Evidence:
  - пакетный no-compile gate `scripts/verify/verify-owner-fba-runtime-order.mjs` проверяет `crates/rustok-workflow/contracts/evidence/workflow-provider-runtime-order-smoke.json`: shared read policy → tenant scope → owner `WorkflowService` invocation → typed error mapping и fallback/degraded registry parity; статус остаётся `in_progress` до live native/GraphQL execution;
  - module plan синхронизирован с central FFA/FBA readiness board; UI surface уже опубликован и ведётся в migration/backlog ритме;
  - FFA admin slice: status badge presentation, workflow table row mapping, template category styling, template-name normalization, module route toggle/legacy href policy, transport request context, transport error presentation и template create command/name policy теперь живут в framework-agnostic `admin/src/core/` с unit tests;
  - transport slice: текущий GraphQL adapter живёт в `admin/src/transport/graphql_adapter.rs`, native server-function adapter добавлен в `admin/src/transport/native_server_adapter.rs`, а `admin/src/transport/mod.rs` стал native-first facade с GraphQL fallback; Leptos UI больше не зависит от raw adapter modules напрямую;
  - UI adapter slice: Leptos-only render code перенесён в `admin/src/ui/leptos.rs`, а crate root оставлен composition/re-export layer для дальнейшего добавления других host adapters;
  - fast boundary guardrail: `scripts/verify/verify-workflow-admin-boundary.mjs` и fixture tests закрепляют отсутствие legacy `api.rs`/flat `transport.rs`, Leptos-free `core/`, raw-adapter-free UI и split native/GraphQL transport adapters;
  - Phase B closure decision: workflow admin FFA больше не расширяется без нового workflow-owned UI/transport surface; дальнейшее повышение до `parity_verified` требует runtime parity evidence для native/server-function + GraphQL fallback и обновления local+central docs в том же change.
  - FBA provider slice: `crates/rustok-workflow/src/ports.rs` declares `WorkflowReadPort` / `workflow.read_projection.v1` for workflow admin read projection consumers with typed `PortContext`/`PortError`, tenant-scope preservation and read deadline semantics; `crates/rustok-workflow/contracts/workflow-fba-registry.json` plus `crates/rustok-workflow/contracts/evidence/workflow-contract-test-static-matrix.json` lock planned contract cases and fallback profiles under `npm run verify:workflow:fba` while runtime execution/fallback smoke remains pending before `boundary_ready`.
  - Compile-free runtime/fallback smoke slice: `crates/rustok-workflow/contracts/evidence/workflow-read-projection-runtime-smoke.json` pins native server-function entrypoints, GraphQL fallback entrypoints, native-first facade assertions and live-evidence closeout criteria without promoting FBA beyond `in_progress`.
- Last verified at (UTC): 2026-06-19T00:00:00Z
- Owner: `rustok-workflow` module team

## Область работ

- удерживать `rustok-workflow` как owner workflow execution domain;
- синхронизировать triggers, steps, transport/UI surfaces и local docs;
- не допускать превращения workflow в отдельный event transport или generic scripting bucket.

## Текущее состояние

- workflow storage и execution journal уже определены внутри модуля;
- engine, trigger handlers, cron/manual/webhook/event triggers и базовые step types уже составляют рабочий baseline;
- GraphQL, REST/webhook ingress и module-owned admin UI уже живут внутри модуля;
- webhook ingress уже закреплён как module-owned transport surface, а cron path удерживается отдельно от `event_listener` и от server webhook shim;
- интеграция с `alloy` уже является capability-level step integration, а не registry-level hard dependency.

## Этапы

### 1. Contract stability

- [x] закрепить workflow engine и execution journal как module-owned runtime;
- [x] зафиксировать transport adapters и admin UI внутри модуля;
- [x] нормализовать local docs и убрать битую кодировку из module docs;
- [~] удерживать sync между workflow runtime contract, UI surfaces и module metadata; текущий FFA slice вынес presentation/view-model helpers, module route policy, transport request context, error presentation и template create command policy из Leptos render path в `admin/src/core/`, добавил native-first transport facade с GraphQL fallback и выделил `ui/leptos` adapter без изменения внешнего GraphQL contract.

### 2. Execution hardening

- [~] довести integration tests для реальной БД и execution history flows; добавлен
  SQLite integration-сценарий duplicate delivery после пересоздания event handler-а,
  а `(workflow_id, trigger_event_id)` закреплён unique index; live PostgreSQL history
  matrix остаётся отдельным verification gate;
- [ ] завершить production-grade реализацию `alloy_script` и `notify` шагов;
- [ ] оценить DAG/branching expansion только при реальном product pressure, не ломая текущий linear-step contract.

### 3. Operability

- [ ] развивать системные события `workflow.execution.*` и execution observability;
- [ ] документировать новые runtime guarantees одновременно с изменением trigger/step semantics;
- [ ] удерживать local docs и `README.md` синхронизированными с live code.

## Проверка

- `cargo xtask module validate workflow`
- `cargo xtask module test workflow`
- targeted tests для triggers, steps, execution journal, tenant isolation и admin/runtime contracts

## Правила обновления

1. При изменении workflow runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
4. При изменении event/alloy integration expectations обновлять связанные docs у foundation и capability modules.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
