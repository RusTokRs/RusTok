# План реализации `rustok-forum`

Статус: forum-owned persistence и основные product capabilities уже
зафиксированы; модуль находится в режиме steady-state hardening.

## Execution checkpoint

- Current phase: in_progress
- Last checkpoint: План синхронизирован с central FBA-track для forum widget-driven consumer path (раздел 13 в `docs/modules/tiptap-page-builder-implementation-plan.md`).
- Next step: До закрытия `P5 / Wave 1 readiness` в центральном треке (`docs/modules/tiptap-page-builder-implementation-plan.md`, раздел 14.2) держать FW-1..FW-4 в статусе `deferred`; разрешены только contract/design-prep обновления в документации.
- Open blockers: Activation delivery по FW-1..FW-4 заблокирован до закрытия `P5`; для старта нужны parity evidence Next/Leptos/Flutter + owner sign-off + Wave 1 Go/No-Go.
- Hand-off notes for next agent: Держать forum domain ownership неизменным; любые widget-изменения проводить как capability-consumer слой и синхронно обновлять central docs.
- Last updated at (UTC): 2026-05-24T00:00:00Z
- Last updated at (UTC): 2026-05-25T12:40:00Z

## Область работ

- удерживать `rustok-forum` как самостоятельный forum/Q&A bounded context;
- синхронизировать topic/reply/moderation contracts, UI packages и local docs;
- развивать forum capabilities без возврата к shared content storage.

## Текущее состояние

- categories, topics, replies и связанные relation/capability tables уже module-owned;
- transport adapters и Leptos admin/storefront packages уже живут внутри модуля;
- forum tags уже работают через shared taxonomy dictionary при forum-owned attachment ownership;
- observability и public read-path semantics уже учитывают visibility, permission filtering и page-sized derived fields.

## Этапы

### 1. Contract stability

- [x] закрыть storage split и forum-owned persistence boundary;
- [x] встроить votes, solutions, subscriptions и user stats как forum-owned capabilities;
- [x] закрепить slug/locale и visibility semantics;
- [ ] удерживать sync между runtime contracts, UI packages и module metadata.

### 2. Product hardening

- [ ] расширять moderation/read-model guarantees только через forum-owned services;
- [ ] удерживать service-level RBAC и public visibility покрытыми regression tests;
- [ ] продолжать выносить тяжёлые derived metrics в отдельные read-model flows только при реальном runtime pressure.

### 3. Operability

- [ ] развивать module-level observability для write-path и capability-specific incidents;
- [ ] документировать новые moderation/visibility guarantees одновременно с изменением runtime surface;
- [ ] удерживать локальные docs и central references синхронизированными.

## Проверка

- [ ] Contract tests cover the current public use-cases
- `cargo xtask module validate forum`
- `cargo xtask module test forum`
- targeted tests для lifecycle, moderation, votes, subscriptions, user stats и visibility filtering

## Правила обновления

1. При изменении forum runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении dependency graph, visibility semantics или metadata синхронизировать `rustok-module.toml`.
4. При изменении forum/content conversion expectations обновлять связанные docs в `rustok-content`.
5. При изменении forum widget/page-builder integration expectations синхронно обновлять `docs/modules/tiptap-page-builder-implementation-plan.md` (раздел Forum widget-driven consumer).


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.

## Forum widget-driven backlog (FBA continuation)


### Deferred policy (до закрытия P5 в central track)

- [x] FW-1/FW-2/FW-3/FW-4 помечены как `deferred` для delivery-активностей.
- [x] Разрешены только contract-design задачи: widget catalog/schema/error mapping в docs без runtime rollout.
- [x] Любая попытка открыть tenant pilot для forum widgets до `P5` считается release-blocker.

### FW-1 — Contract freeze

- [ ] Утвердить widget catalog v1: `forum.topic_list`, `forum.topic_detail`, `forum.reply_stream`.
- [ ] Зафиксировать `data_contract_version` и compatibility matrix для consumer adapters.
- [ ] Утвердить `props_schema` validation и typed error mapping (`validation/sanitize/rbac/runtime`).

### FW-2 — Fallback hardening

- [ ] Подтвердить `builder_off` и `publish_off` без 5xx для forum read/moderation paths.
- [ ] Зафиксировать fallback semantics (`readonly/hidden/degraded`) по каждому widget type.
- [ ] Добавить regression checklist для visibility/RBAC parity под partial disable capability layer.

### FW-3 — Pilot readiness

- [ ] Подготовить Wave evidence packet (`metadata/fallback/observability/rollback`) для 1–2 low-traffic tenant.
- [ ] Подтвердить observability correlation: `builder write -> forum read/publish/moderation`.
- [ ] Провести Go/No-Go review с Platform + Builder + Forum + Frontend owners.
