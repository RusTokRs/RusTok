# План миграции RBAC на relation/Casbin runtime

- Дата: 2026-03-18
- Статус: In progress
- Область: `apps/server`, `crates/rustok-rbac`, `crates/rustok-core`, `apps/server/migration`
- Цель: завершить переход к модульной RBAC-схеме, где relation-данные остаются каноническим permission graph, а runtime проходит путь `relation_only -> casbin_shadow -> casbin_only`.

---

## 1. Текущая модель

### 1.1 Source of truth

- Канонические RBAC-связи хранятся только в `roles`, `permissions`, `user_roles`, `role_permissions`.
- Публичный server-side фасад для RBAC: `apps/server/src/services/rbac_service.rs`.
- Policy/use-case ядро и shadow runtime живут в `crates/rustok-rbac`.
- Runtime и schema используют только relation graph; effective role полностью выводится из permission relations.

### 1.2 Runtime режимы

- `relation_only`: relation-resolver принимает решение, shadow-пути выключены.
- `casbin_shadow`: relation-resolver остаётся авторитативным, дополнительно пишется relation-vs-casbin parity.
- `casbin_only`: runtime decision принимает Casbin path поверх того же relation-derived permission set.

### 1.3 Канонический control plane

- Единственный rollout key: `RUSTOK_RBAC_AUTHZ_MODE`.
- Допустимые значения: `relation_only`, `casbin_shadow`, `casbin_only` и их document-approved aliases (`relation-only|relation`, `casbin-shadow|casbin_shadow_read`, `casbin-only|casbin`).
- Transitional env flags и legacy compatibility aliases удалены.

---

## 2. Что уже завершено (актуализировано на 2026-03-18)

### 2.1 Domain и runtime extraction

- `PermissionResolver`, `permission_policy`, `permission_evaluator`, `permission_authorizer` вынесены в `rustok-rbac`.
- `RuntimePermissionResolver` и relation/cache adapters используются как модульный runtime contract.
- `RbacService`, `rbac_runtime`, `rbac_persistence` отделены в `apps/server`.
- Legacy server shim `services::auth` удалён.
- Core naming приведён к новой схеме `Identity*`.

### 2.2 Observability и rollout safety

- Permission cache, decision, denied-reason и latency metrics публикуются из server runtime.
- Casbin parity path пишет structured mismatch event `rbac_engine_mismatch`.
- `/metrics` публикует canonical parity counters:
  - `rustok_rbac_engine_decisions_relation_total`
  - `rustok_rbac_engine_decisions_casbin_total`
  - `rustok_rbac_engine_mismatch_total`
  - `rustok_rbac_engine_eval_duration_ms_total`
  - `rustok_rbac_engine_eval_duration_samples`
- Baseline helper `scripts/rbac_cutover_baseline.sh` переведён на engine-mismatch gate и теперь валидирует decision volume (`total_decisions_delta` / `permission_checks_total_delta`) вместе с reset detection по счётчикам.
- Cutover gate helper `scripts/rbac_cutover_gate.sh` собирает единый Go/No-Go bundle из staging artifacts, cutover baseline и auth release-gate report и пишет markdown/json decision artifacts.

### 2.3 Cleanup уже выполненного legacy слоя

- Удалены transitional legacy runtime paths и связанный cache/load слой.
- Удалены obsolete mismatch signals прошлой migration-схемы.
- `shadow_runtime` сведён к relation-vs-casbin parity.
- Актуальные server/library callsites используют `RbacService`.
- Старый staging helper `scripts/rbac_relation_staging.sh` и его smoke-тесты больше не присутствуют в репозитории; staging rehearsal теперь рассматривается как artifact bundle (`artifacts/rbac-staging/*`), который валидируется на этапе `scripts/rbac_cutover_gate.sh`.

---

## 3. Текущее состояние по фазам

### Фаза A. Relation baseline

Статус: завершено.

- Relation-resolver работает как текущий authoritative path.
- Сохранён только lightweight consistency/report tooling:
  - `cleanup target=rbac-report`

### Фаза B. Casbin parity

Статус: в работе, но кодовая база для parity/cutover уже собрана.

- `casbin_shadow_evaluator` и `shadow_runtime` уже модульные.
- Server пишет parity telemetry и structured mismatch logs.
- Authorizer path уже mode-aware: `casbin_only` реально переключает active engine, а не существует только как rollout enum.
- Cutover baseline helper считает deltas по `rustok_rbac_engine_mismatch_total`.

Открытые задачи:

1. Закрыть parity evidence на staging/production окне наблюдения и сохранить timestamp-consistent artifacts в `artifacts/rbac-staging/*` и `artifacts/rbac-cutover/*`; `scripts/rbac_staging_rehearsal.sh` теперь собирает staging bundle с `rbac_relation_stage_report_<ts>.md`, `rbac_relation_stage_report_<ts>.json`, `rbac_report_pre_<ts>.json` и `rbac_report_post_<ts>.json`; `scripts/rbac_cutover_gate.sh` читает summary JSON как основной источник rehearsal metadata и fallback'ается на markdown только для legacy bundles (legacy alias `rbac_report_post_rollback_<ts>.json` всё ещё принимается gate-скриптом).
2. Подтвердить нулевой `engine_mismatch_delta` и `shadow_compare_failures_delta` в baseline окне.
3. Сформировать финальный Go/No-Go bundle через `scripts/rbac_cutover_gate.sh --auth-gate-report <report>` или единым orchestration path через `scripts/rbac_cutover_workflow.sh`.

### Фаза C. Casbin cutover

Статус: не начато в production, но operational gate уже автоматизирован.

Ожидаемый переход:

1. `relation_only`
2. `casbin_shadow`
3. `casbin_only`

Go/No-Go условия описаны в ADR `DECISIONS/2026-03-05-rbac-relation-only-final-cutover-gate.md`.

### Фаза D. Post-cutover cleanup

Статус: в работе.

Уже закрыто:

- transitional env aliases;
- server compatibility shims;
- dual-read/legacy-role runtime;
- server-owned RBAC policy duplication.
- auth/token/response path и user schema полностью переведены на relation-derived role.
- legacy relation backfill/rollback tooling удалён вместе с staging helper script.

Осталось:

1. добить документацию и verification docs под relation/casbin-only модель;
2. добить оставшиеся docs/UI references, если где-то ещё описан старый column-based role path;
3. закрыть release evidence и перевести план в steady-state сопровождение.

---

## 4. Ближайший рабочий backlog

### 4.1 Обязательно до `casbin_only`

1. Прогнать staging rehearsal и приложить invariant artifacts.
2. Снять production baseline через `scripts/rbac_cutover_baseline.sh`.
3. Подтвердить:
   - `engine_mismatch_delta == 0`
   - `shadow_compare_failures_delta == 0`
   - decision volume >= `min-decision-delta`
4. Пройти cutover gate (`scripts/rbac_cutover_gate.sh`) и зафиксировать Go/No-Go decision artifacts.

### 4.2 Можно делать параллельно

1. Убирать остаточные текстовые упоминания старого column-based role path.
2. Сжимать runbooks и verification docs под финальную модель.
3. Подчищать naming вокруг relation/casbin runtime boundary.
4. Удалять устаревшие упоминания `scripts/rbac_relation_staging.sh` и связанных smoke tests из локальной документации.
5. Держать app/module tests сериализованными там, где используется общий manifest env (`RUSTOK_MODULES_MANIFEST`).

---

## 5. Артефакты и источники истины

### 5.1 Основные документы

- ADR runtime source-of-truth: `DECISIONS/2026-02-26-rbac-relation-source-of-truth-cutover.md`
- ADR final cutover gate: `DECISIONS/2026-03-05-rbac-relation-only-final-cutover-gate.md`
- Module docs: `crates/rustok-rbac/docs/README.md`
- Server module docs: `apps/server/docs/README.md`

### 5.2 Операционные артефакты

- `artifacts/rbac-staging/*`
- `artifacts/rbac-cutover/*`
- staging artifacts из `scripts/rbac_staging_rehearsal.sh`
- decision artifacts из `scripts/rbac_cutover_gate.sh`
- auth release-gate bundle из `scripts/auth_release_gate.sh --require-all-gates`

### 5.3 Проверенные расхождения относительно старого плана

При актуализации плана 2026-03-18 подтверждено следующее:

- `RbacAuthzMode` по-прежнему поддерживает только `relation_only`, `casbin_shadow`, `casbin_only` и задокументированные aliases; fallback для неизвестных значений остаётся `RelationOnly`.
- Server runtime уже пишет canonical engine counters и сохраняет compatibility aliases в `/metrics`, поэтому operational docs должны ссылаться на canonical метрики, а alias-метрики считать временным слоем совместимости.
- В репозитории появился отдельный helper `scripts/rbac_cutover_gate.sh`, которого не было в ранних версиях плана; он фактически закрывает часть manual checklist из фаз B/C.
- В репозитории больше нет `scripts/rbac_relation_staging.sh`, поэтому старые инструкции, завязанные на этот helper, считаются устаревшими и подлежат вычищению перед продолжением migration-doc work.

---

## 6. Критерии закрытия плана

План считается закрытым, когда одновременно выполнено всё ниже:

1. Runtime mode `casbin_only` включён и стабилен.
2. Relation-vs-Casbin parity закрыт нулевым baseline окном.
3. Legacy cleanup завершён:
   - dual-read path отсутствует в коде и docs;
   - `services::auth` отсутствует;
   - transitional env flags отсутствуют;
   - obsolete mismatch metrics отсутствуют.
4. Документация и verification планы синхронизированы с финальной схемой.
5. Пост-cutover окно стабилизации закрыто без rollback.
