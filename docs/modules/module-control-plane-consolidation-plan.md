---
id: doc://docs/modules/module-control-plane-consolidation-plan.md
kind: implementation_plan
language: markdown
last_verified_snapshot: snap_jsonl_00000040
source_language: markdown
status: verified
---
# План консолидации управления модулями

## Execution checkpoint

- Current phase: `not_started`
- Last checkpoint: зафиксирована текущая фрагментация module control plane между foundation-контрактами, server services, GraphQL и admin SSR.
- Next step: подготовить ADR с целевой ownership boundary и инвентаризацией production entrypoints.
- Open blockers: целевая граница shared contracts/server orchestration должна быть утверждена до переноса кода.
- Hand-off notes for next agent: не смешивать эту работу с временным production-remediation планом; не начинать с создания нового crate без ADR.
- Last updated at (UTC): 2026-06-27T00:00:00Z

## Проблема

Управление модулями имеет несколько законных уровней, но их orchestration и
transport-реализация распределены шире необходимого:

- `rustok-core` владеет базовыми module contracts и `ModuleRegistry`;
- `apps/server/src/modules` собирает runtime registry и валидирует manifest;
- `ModuleLifecycleService` управляет tenant enable/disable и recovery;
- `PlatformCompositionService` управляет platform snapshot и build enqueue;
- `RegistryGovernanceService` управляет публикацией, releases и ownership;
- GraphQL публикует server API;
- `apps/admin/src/features/modules/api` дополнительно содержит собственные manifest
  DTO, hashing, SQL, build/release и marketplace orchestration.

Главный дефект границы: admin host частично выполняет backend/control-plane
обязанности вместо потребления единого server-owned API. Это создаёт несколько
источников taxonomy, validation и state mapping.

## Цель

Сформировать единый server-owned module control plane с явными поддоменами и
одним набором transport-neutral contracts. Admin и другие hosts должны только
вызывать API и отображать canonical payload без SQL, manifest parsing, hashing
или lifecycle taxonomy.

## Не входит в план

- перенос module-owned business logic или UI в platform host;
- объединение platform composition и tenant enablement в один state;
- удаление GraphQL при добавлении native `#[server]` функций;
- превращение capability crate-ов в tenant-toggled modules;
- создание нового foundation crate до архитектурного решения.

## Целевая модель

Control plane сохраняет четыре отдельные области состояния:

1. build/runtime composition — какие модули входят в platform snapshot;
2. registry governance — package/release/owner/publish lifecycle;
3. tenant lifecycle — enable/disable/settings/recovery для `Optional` modules;
4. effective policy — итоговая доступность с учётом platform, tenant и channel.

Один server facade координирует эти области, но не смешивает их таблицы и
инварианты. Shared слой содержит только DTO, pure validation и error taxonomy;
БД, транзакции, build jobs и hooks остаются в server-owned implementation.

## Этапы

### 0. Архитектурная фиксация

- [ ] Составить inventory всех read/write entrypoints, SQL и manifest DTO.
- [ ] Зафиксировать ADR: ownership, dependency direction и transaction boundaries.
- [ ] Определить canonical error taxonomy и revision/CAS semantics.
- [ ] Зафиксировать compatibility policy для GraphQL и native server functions.

### 1. Общие контракты

- [ ] Выделить transport-neutral snapshots для catalog, platform composition,
  tenant lifecycle, recovery и governance.
- [ ] Оставить pure manifest/registry validation в shared contract layer.
- [ ] Удалить дублирующиеся DTO и локальные taxonomy mappings после перевода consumers.
- [ ] Добавить contract tests на serialization, error codes и revision conflicts.

### 2. Server-owned orchestration

- [ ] Ввести единый facade над существующими lifecycle/composition/governance services.
- [ ] Зафиксировать один write entrypoint на каждую операцию.
- [ ] Сохранить atomic boundaries для platform CAS + build enqueue и tenant journal + state.
- [ ] Запретить прямые записи в control-plane таблицы вне owner services статическим guardrail.

### 3. Transport surfaces

- [ ] Перевести GraphQL queries/mutations на canonical facade.
- [ ] Добавить native `#[server]` adapters для Leptos без удаления GraphQL.
- [ ] Обеспечить одинаковые payload/error/recovery semantics на обоих transports.
- [ ] Добавить parity tests GraphQL ↔ native adapters.

### 4. Упрощение admin host

- [ ] Удалить из admin SSR прямой SQL к platform/build/registry tables.
- [ ] Удалить admin-owned manifest loading, canonical hashing и build planning.
- [ ] Оставить в admin transport facade, view models и UI effects.
- [ ] Запретить local remap lifecycle taxonomy и recovery metadata.

### 5. Effective policy и consumers

- [ ] Свести module availability checks к одному typed effective-policy contract.
- [ ] Разделить platform installed, tenant enabled и channel bound в API и UI.
- [ ] Проверить Core/Optional invariants и dependency graph на всех write paths.
- [ ] Добавить tenant/channel isolation и stale revision tests.

### 6. Миграция и удаление legacy paths

- [ ] Перевести consumers поэтапно с dual-read comparison без dual-write.
- [ ] Добавить telemetry на использование legacy entrypoints.
- [ ] Удалить legacy paths после нулевого usage window.
- [ ] Обновить central/local docs и operational runbook.

## Verification gates

- [ ] Один production write path для каждой control-plane операции.
- [ ] В `apps/admin` отсутствуют прямые SQL и canonical manifest/build algorithms.
- [ ] GraphQL/native parity подтверждена contract tests.
- [ ] Platform CAS/build enqueue и tenant lifecycle journal/state остаются атомарными.
- [ ] Core нельзя отключить; Optional dependencies нельзя нарушить.
- [ ] Recovery/retry/compensation сохраняют canonical taxonomy.
- [ ] `cargo check -p rustok-server --lib` и targeted module/admin tests проходят.

## Definition of done

План завершён, когда server является единственным владельцем module-management
orchestration, shared layer содержит только переиспользуемые contracts/pure
validation, admin не имеет backend bypass paths, а все transports подтверждают
одинаковые lifecycle, revision и recovery semantics.
