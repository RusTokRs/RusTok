# rustok-workflow — API overview

> Этот документ — **curated overview**, а не ручной слепок сигнатур.
> Канонический reference берётся из кода и сгенерированных артефактов.

## Source of truth

- Код crate: `crates/rustok-workflow/src/**`
- Rustdoc (локально):
  - `cargo doc -p rustok-workflow --no-deps`
- Server runtime wiring:
  - `apps/server/src/modules/workflow.rs`

## Module registration contract

`WorkflowModule` регистрируется как optional module в module registry и публикует:

- slug/name/kind модуля;
- миграции workflow bounded context;
- permissions для workflow authoring/execution read paths.

Для точных значений (slug, permissions, migration list) используйте код,
а не дублирование в markdown.

## Public surface map

Публичный API crate разбит на следующие области:

- `controllers` — REST endpoints и route wiring;
- `graphql` — query/mutation surface для workflow authoring и reads;
- `services` — доменный orchestration слой (`WorkflowService`, execution/use-case logic);
- `steps` — runtime step contracts и registries;
- `templates` — builtin workflow templates и metadata;
- `dto`/`entities`/`error` — transport contracts, storage model и domain errors.

## Domain invariants (must-hold)

- Все write/read операции tenant-scoped.
- Workflow execution path отделён от authoring path.
- Step execution идёт через типизированный step runtime (`WorkflowStep` contract).
- Trigger paths (manual/webhook/event/cron) сходятся в единый execution orchestration.
- Ошибки исполнения и конфигурации должны возвращаться как typed domain errors,
  без silent fallback.

## Execution model

- `WorkflowService` отвечает за CRUD и orchestration use-cases.
- `WorkflowEngine` выполняет шаги и управляет step registry/runtime dispatch.
- `WorkflowTriggerHandler` интегрирует event-driven trigger path.
- `WorkflowCronScheduler` закрывает schedule-driven trigger path.

Важно: актуальные методы/сигнатуры смотрим в исходниках и rustdoc.
Этот документ фиксирует роли и boundaries, а не API-by-hand.

## Extensibility contract

Кастомные шаги подключаются через `WorkflowStep` trait и регистрацию
в engine runtime (`with_step(...)`).

Требования к шагам:

- deterministic behavior при одинаковом входном контексте;
- корректная обработка ошибок через `WorkflowResult`;
- отсутствие tenant-boundary bypass;
- отсутствие скрытых side effects вне контракта шага.

## Transport entry points

- GraphQL: workflow query/mutation roots.
- REST: workflow controllers/routes, включая webhook trigger path.

Точные route/query имена и payload contracts смотреть в source + generated schema.

## Documentation maintenance rule

При изменении workflow transport contract, execution semantics или error model:

1. Обновить этот overview (роли, инварианты, boundaries).
2. Не дублировать ручные сигнатуры в markdown.
3. Добавить/обновить ссылки на generated reference в релевантных docs.

## Hotspot contract (DOC-12 / H4)

- Hotspot: `H4` (Workflow/Public API contracts).
- Doc contracts updated: `crates/rustok-workflow/CRATE_API.md`.
- Owner scope: workflow module owner.
- Residual drift risk:
  - до закрытия DOC-09 (B12 CI artifacts) возможен разрыв между curated overview
    и фактическими exported reference-артефактами в PR;
  - при изменении transport payload-форм без обновления generated references
    risk остаётся высоким.
