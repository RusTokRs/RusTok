# План устранения недостатков control plane и module lifecycle

Дата перепроверки: 2026-05-18.
Основание: повторная проверка `docs/research/deep-research-report (4).md` по текущему коду репозитория.

## Резюме перепроверки

Исходный TODO уже частично закрыт. В текущем коде появились `platform_state`, `module_operations`,
`manifest_revision`/`manifest_snapshot`, DB-backed CAS для состава платформы, effective policy service,
CI-gates для `xtask`, coverage, SBOM/provenance, а Dependabot больше не ссылается на `/apps/mcp`.
Оставшиеся риски теперь уже не совпадают один-в-один с исходным списком: главный хвост — атомарность
операций control plane, единый lifecycle entrypoint для всех admin/runtime поверхностей и формализация
migration dependency metadata вместо локального hardcoded списка.

## Что подтверждено как уже закрыто

| Исходный пункт | Текущее состояние | Проверенные файлы |
| --- | --- | --- |
| Runtime/admin пишут `modules.toml` как runtime target | Закрыто частично: runtime-снимок читается из `platform_state`, а `modules.toml` остаётся bootstrap/dev input. Production Dockerfile по-прежнему не копирует `modules.toml`, что теперь соответствует DB-backed runtime модели. | `apps/server/src/models/_entities/platform_state.rs`, `apps/server/src/services/platform_composition.rs`, `apps/admin/src/features/modules/api.rs`, `apps/server/Dockerfile` |
| Нет revision/CAS для build enqueue | Закрыто частично: `platform_state.revision` и CAS есть, stale write возвращает conflict. Остался риск неатомарного `platform_state update -> build insert/request_build`. | `apps/server/src/services/platform_composition.rs`, `apps/server/src/graphql/mutations.rs`, `apps/admin/src/features/modules/api.rs` |
| `settings.default_enabled` расходится с tenant overrides | Закрыто частично: появился `EffectiveModulePolicyService` с правилом `core + manifest.default_enabled + tenant overrides`. Нужно убрать локальные/legacy обходы и расширить тесты parity. | `apps/server/src/services/effective_module_policy.rs`, `apps/server/src/services/module_lifecycle.rs`, `apps/admin/src/features/modules/api.rs` |
| Enable/disable обходят lifecycle и не пишут journal | Закрыто частично: GraphQL и server seed/installer идут через `ModuleLifecycleService`, journal есть. Остались public model-level `tenant_modules::toggle` и дублированная Leptos SSR реализация toggle в `apps/admin`. | `apps/server/src/services/module_lifecycle.rs`, `apps/server/src/models/tenant_modules.rs`, `apps/server/src/graphql/mutations.rs`, `apps/admin/src/features/modules/api.rs` |
| Hooks вызываются после записи состояния, rollback только флага | Не закрыто: текущий `ModuleLifecycleService` и Leptos SSR toggle сначала пишут `tenant_modules`, затем вызывают hook и при ошибке откатывают только enabled flag. | `apps/server/src/services/module_lifecycle.rs`, `apps/admin/src/features/modules/api.rs` |
| Locale rollback сужал `VARCHAR(5)` | Закрыто: down-migration стала irreversible/no-op, чтобы не сужать BCP47-like locale values. | `apps/server/migration/src/m20260405_000001_expand_locale_storage_columns.rs` |
| Server migrator сортирует lexical + ad-hoc special-case | Закрыто частично: есть dependency-aware pass, но dependency metadata всё ещё зашита в `migration_dependencies()` одним hardcoded match. | `apps/server/migration/src/lib.rs` |
| CI не запускает manifest/module validation, coverage threshold, SBOM/provenance | Закрыто: `platform-contract`, coverage threshold/artifact и SBOM provenance присутствуют в CI. | `.github/workflows/ci.yml`, `scripts/ci/check-coverage.sh` |
| Dependabot ссылается на `/apps/mcp` | Закрыто: stale path отсутствует. | `.github/dependabot.yml` |
| Нет repository-level license policy | Ранее уже было устаревшим: `deny.toml` и `cargo-deny-action` есть. | `deny.toml`, `.github/workflows/ci.yml` |

## Оставшиеся недостатки и план исправления

### P0 — сделать composition update и build enqueue атомарными

**Проблема.** GraphQL path вызывает `PlatformCompositionService::update_manifest()` и затем отдельно
`BuildService::request_build()`. Leptos SSR path в `apps/admin` делает raw SQL update `platform_state`,
а затем отдельный insert в `builds`. Если build insert/request падает, активная ревизия платформы уже
изменена, но build job может не появиться.

**План.**

1. Вынести сценарий `validate manifest -> CAS update platform_state -> enqueue build` в единый сервис
   `PlatformCompositionBuildService` в server/control-plane слое.
2. Выполнять CAS-update и создание `builds` в одной DB transaction.
3. Возвращать conflict-style ошибку до мутации, если `expected_revision` устарел.
4. Для Leptos SSR заменить raw SQL helper `save_manifest_and_enqueue_build()` на вызов этого сервиса
   или на тонкий shared adapter, если прямой import server crate невозможен.
5. Добавить regression tests:
   - stale revision не создаёт build;
   - ошибка build insert не меняет `platform_state.revision`;
   - GraphQL и Leptos SSR возвращают одинаковый `manifest_ref = platform_state:<revision>`.

**Критерии готовности.** Нет кода, где `platform_state` обновляется отдельно от build enqueue; оба public
admin surfaces проходят один contract-test набор.

### P0 — унифицировать enable/disable lifecycle entrypoint

**Проблема.** `ModuleLifecycleService` уже существует, но Leptos SSR toggle в `apps/admin` содержит
собственную копию lifecycle logic: dependency checks, journal insert/update, hook invocation и rollback.
Публичные helpers `tenant_modules::toggle` всё ещё позволяют записать flag без policy/journal/hooks,
даже если сейчас они почти не используются.

**План.**

1. Оставить один canonical entrypoint: `ModuleLifecycleService::toggle_module_with_actor()`.
2. Переподключить Leptos SSR `toggle_module_native()` к canonical entrypoint или вынести общий
   lifecycle adapter в crate, доступный Leptos SSR build-у.
3. Сделать model-level `tenant_modules::toggle` private/test-only либо переименовать в явно опасный
   `upsert_flag_without_lifecycle_for_migrations_only` с ограниченной видимостью.
4. Добавить repo-side `rg` guard/test, который запрещает production-вызовы прямого tenant module toggle.
5. Расширить tests на parity GraphQL/Leptos SSR: одинаковые ошибки для unknown/core/dependency/dependent
   cases и обязательная запись `module_operations`.

**Критерии готовности.** Все runtime/admin enable/disable операции проходят через один сервис и всегда
создают audit/journal record при изменении effective state.

### P0 — пересобрать hook semantics без частичного rollback

**Проблема.** Текущая последовательность остаётся `persist tenant_modules -> run hook -> rollback enabled flag on error`.
Это не откатывает побочные эффекты hook-а, settings/metadata и внешние события, а journal фиксирует только
`failed` после уже выполненной компенсации флага.

**План.**

1. Разделить lifecycle на фазы: `validated`, `running`, `committed`, `failed`.
2. Записывать `module_operations` до мутации tenant state со статусом `running` и correlation id.
3. Ввести явную hook policy:
   - `pre_enable`/`pre_disable` — до коммита state, без ожидания enabled state;
   - `post_enable`/`post_disable` — после коммита, только idempotent side effects;
   - для существующих `on_enable`/`on_disable` временно задокументировать compat-слой.
4. Выполнять state mutation и перевод operation в `committed` в одной transaction после успешной pre-фазы.
5. Для post-фазы хранить retryable failure отдельно, не откатывая committed state без отдельной compensating operation.
6. Обновить tests на failure modes: pre-hook failure не меняет effective state; post-hook failure создаёт retryable operation issue.

**Критерии готовности.** Hook failure больше не оставляет систему в состоянии “флаг откатили, побочные эффекты неизвестны”,
а recovery описан через journal/retry/compensation.

### P1 — убрать дубли raw SQL и стабилизировать manifest hash

**Проблема.** В `apps/admin` есть локальные SQL helpers для `platform_state`, `module_operations` и `builds`,
а server-side `PlatformCompositionService::manifest_hash()` использует короткий hash от отсортированного подмножества
manifest fields. Это повышает риск drift между surfaces и не даёт сильного immutable artifact hash.

**План.**

1. Описать canonical `ManifestSnapshot` serializer: canonical JSON для всего состава, включая `settings`, build profile,
   module dependency metadata и source pins.
2. Заменить текущий short hash на SHA-256 hex (64 chars, совпадает с длиной DB column).
3. Перевести GraphQL и Leptos SSR на общий serializer/hash builder.
4. Добавить тест “один manifest -> один hash/ref/snapshot” для GraphQL, Leptos SSR и BuildService.
5. Удалить или сузить raw SQL helpers после появления shared service.

**Критерии готовности.** `manifest_hash` одинаково считается во всех путях, snapshot является полным immutable
снимком состава, а DB column length используется по назначению.

### P1 — формализовать dependency-aware migration ordering

**Проблема.** После исправления lexical ordering появился dependency-aware pass, но список зависимостей
остаётся централизованным hardcoded `match`, сейчас только для `product_tags -> taxonomy_tables`.

**План.**

1. Ввести lightweight metadata contract для migration dependencies: например `MigrationDescriptor { migration, after }`
   в module-owned migration exporters.
2. Сохранять lexical ordering как default tie-breaker, но строить полный topological sort по descriptor metadata.
3. Валидировать missing dependency и cycle как test/runtime error, а не “append remaining”.
4. Перевести текущую taxonomy/product-tags зависимость на descriptor.
5. Добавить тесты на missing dependency, cycle и cross-module ordering.

**Критерии готовности.** В `apps/server/migration/src/lib.rs` нет module-specific hardcoded dependency match;
новые module-owned migrations могут объявлять порядок рядом с владельцем модуля.

### P1 — закрепить CI-gates как non-regression contract

**Проблема.** Исходные CI gaps закрыты, но для предотвращения регресса это нужно считать contract, а не разовой настройкой.

**План.**

1. Добавить в `docs/verification/platform-quality-operations-verification-plan.md` explicit non-regression пункт:
   `cargo xtask validate-manifest`, `cargo xtask module validate`, coverage threshold, SBOM provenance,
   `cargo-deny-action`, отсутствие stale Dependabot directories.
2. Добавить лёгкий script/test, который проверяет, что `.github/dependabot.yml` directories существуют.
3. Для coverage threshold вынести минимальный процент в один env/constant, чтобы docs и workflow не расходились.

**Критерии готовности.** Изменение CI workflow, удаляющее эти gates, будет заметно в docs/tests review.

### P2 — обновить документацию и ADR по control plane

**Проблема.** `deep-research-report (4).md` больше не является точным backlog-ом: часть пунктов уже закрыта,
часть переформулирована после перепроверки.

**План.**

1. Оставить research report как historical input и ссылаться из него на этот remediation plan.
2. Обновить `docs/architecture/modules.md`, `docs/modules/manifest.md` и server/admin local docs после реализации P0/P1.
3. Если меняется hook contract или migration descriptor contract, оформить ADR в `DECISIONS/`.
4. Обновить `docs/index.md` при каждом добавлении/переименовании документов.

**Критерии готовности.** Центральная документация описывает фактический runtime control plane, а не устаревшую
`modules.toml`-как-runtime-source модель.

## Рекомендуемый порядок работ

1. P0.1: atomic `PlatformCompositionBuildService` + tests для GraphQL path.
2. P0.2: Leptos SSR install/uninstall/upgrade перевод на тот же service.
3. P0.3: canonical lifecycle entrypoint для enable/disable и удаление прямых bypass helpers.
4. P0.4: hook semantics redesign + ADR.
5. P1.1: SHA-256 canonical manifest snapshot/hash.
6. P1.2: migration descriptors/topological sort.
7. P1.3: CI non-regression docs/scripts.
8. P2: финальная синхронизация центральных и локальных docs.

## Минимальный verification набор для каждой итерации

- `cargo fmt --all -- --check`
- `cargo test -p migration`
- `cargo test -p rustok-server module_lifecycle`
- `cargo test -p rustok-server platform_composition`
- `cargo xtask validate-manifest`
- `cargo xtask module validate`
- для изменений CI/coverage: локальная проверка `bash scripts/ci/check-coverage.sh <lcov-file> 75` на сгенерированном LCOV.

---

## Исполнительный трек до полной реализации

Ниже — детализированный execution backlog, который продолжает исходный план до состояния “done”
с явными deliverables, проверками и критериями закрытия. Этот раздел предназначен как рабочий чеклист
для последовательной реализации, а не как новый отдельный документ.

### Статусы

- `[ ]` — не начато
- `[~]` — в работе
- `[x]` — реализовано и проверено тестами

### Актуализация 2026-05-20

- Обновлён статус исходного `deep-research-report (4).md`: документ зафиксирован как **historical input**.
- Этот файл остаётся **единственным актуальным execution backlog** для remediation-трека control plane/module lifecycle.
- Приоритет работ не меняется: сначала P0 (atomic composition + lifecycle unification + hook semantics), затем P1/P2.

### Этап 1 (P0): атомарность control-plane и единый lifecycle entrypoint

#### 1.1 Atomic composition + build enqueue

- [x] Добавить `PlatformCompositionBuildService` (server/control-plane слой).
- [x] Перенести `validate manifest -> CAS update platform_state -> build enqueue` в один transaction boundary.
- [x] Гарантировать одинаковый `manifest_ref = platform_state:<revision>` для GraphQL и Leptos SSR.
- [x] Перевести admin SSR path с raw SQL helper на shared service/adapter.
- [x] Удалить/ограничить legacy helper `save_manifest_and_enqueue_build()` после перевода.

**Обязательные тесты закрытия:**

- [x] stale revision не создаёт build job;
- [x] ошибка enqueue/build insert не меняет `platform_state.revision`;
- [~] GraphQL/Leptos SSR parity for `manifest_ref`, revision и error mapping (GraphQL error mapping tests добавлены; Leptos SSR parity coverage остаётся незакрытым).

#### 1.2 Canonical lifecycle entrypoint для enable/disable

- [x] Оставить `ModuleLifecycleService::toggle_module_with_actor()` как единственный production entrypoint.
- [x] Переподключить `toggle_module_native()` в `apps/admin` к canonical lifecycle service.
- [x] Перевести/закрыть прямой model-level toggle bypass в `tenant_modules`.
- [x] Добавить repo-guard (test/script) против production вызовов bypass API.

**Обязательные тесты закрытия:**

- [~] parity ошибок GraphQL/Leptos SSR (`unknown/core/dependency/dependent`): GraphQL error mapping вынесен в `map_toggle_module_error` и покрыт unit tests (unknown/core/dependency/dependent/hook_failed + internal mapping для database/policy); native toggle path в `apps/admin` удалён, toggle идёт через canonical GraphQL entrypoint, а repo-guard tests блокируют реинтродукцию `admin/toggle-module`/`toggle_module_native`; остаётся закрыть full parity tests Leptos SSR vs GraphQL для runtime error taxonomy;
- [~] журнал (`module_operations`) всегда записывается при смене effective state (добавлены server integration tests `successful_toggle_writes_done_module_operation`, `hook_failure_rolls_back_state` (failed journal), `successful_toggle_with_actor_persists_requested_by`, `toggle_without_actor_records_null_requested_by`, `hook_failure_with_actor_records_failed_operation_with_actor`, `hook_failure_without_actor_records_failed_operation_with_null_actor`; добавлены проверки, что идемпотентный повтор не создаёт дублирующиеся записи журнала и что ошибки предвалидации (`UnknownModule`/`CoreModuleCannotBeDisabled`/`MissingDependencies`/`HasDependents`) и no-op переходы disable/enable не создают лишние записи журнала; остаётся parity-покрытие GraphQL/Leptos SSR);
- [x] прямой toggle bypass отсутствует в production code paths (repo-guard расширен на `apps/server` + `apps/admin`, включая проверки на отсутствие `admin/toggle-module`/`toggle_module_native` и GraphQL-only contract в `toggle_module`; helper-parser guard тестов покрыт nested-braces/missing-signature cases).

#### 1.3 Hook semantics без частичного rollback

- [ ] Ввести lifecycle status model: `validated -> running -> committed -> failed`.
- [ ] Писать `module_operations` в статусе `running` до state mutation (с correlation id).
- [ ] Развести pre/post hooks:
  - [ ] `pre_enable`/`pre_disable` до коммита state;
  - [ ] `post_enable`/`post_disable` после коммита state (idempotent side effects).
- [ ] Для legacy `on_enable`/`on_disable` описать и внедрить compat-layer.
- [ ] Не откатывать committed state при post-hook failure: фиксировать retryable issue.
- [ ] Добавить recovery механизм (retry/compensating operation) через journal.

**Обязательные тесты закрытия:**

- [ ] pre-hook failure не меняет effective state;
- [ ] post-hook failure не откатывает committed state и создаёт retryable issue;
- [ ] повторный retry post-hook корректно идемпотентен.

### Этап 2 (P1): хеш snapshot, migration descriptors, CI non-regression

#### 2.1 Canonical manifest snapshot/hash

- [ ] Ввести canonical serializer полного manifest snapshot (включая settings, profile, dependency metadata, source pins).
- [x] Перейти на SHA-256 hex (64 chars) для `manifest_hash`.
- [~] Убрать drift: GraphQL, BuildService и Leptos SSR используют один canonical hashing contract (Leptos SSR path canonicalizes JSON snapshot перед SHA-256; SSR unit tests покрывают SHA-256 format/stability/change detection и fixed vector; server integration tests `successful_enqueue_keeps_hash_parity_between_snapshot_and_build` и `successful_enqueue_keeps_manifest_snapshot_parity_with_hash` дополнительно фиксируют parity `snapshot.manifest_hash == build.manifest_hash` и `build.manifest_snapshot` canonical JSON parity; остаётся закрыть cross-surface parity test на один и тот же manifest → один hash/ref/snapshot для GraphQL+Leptos SSR end-to-end).

**Обязательные тесты закрытия:**

- [ ] один и тот же manifest даёт одинаковый hash/ref/snapshot во всех surfaces;
- [x] изменения в значимых полях гарантированно меняют hash;
- [x] length/format hash соответствует DB контракту.

#### 2.2 Dependency-aware migration ordering через metadata

- [ ] Ввести descriptor contract (например `MigrationDescriptor { migration, after }`) в module-owned exporters.
- [ ] Реализовать topological sort с lexical tie-breaker.
- [ ] Сделать missing dependency и cycle явной ошибкой (test/runtime), без fallback “append remaining”.
- [ ] Перевести текущую зависимость taxonomy/product-tags на descriptor.

**Обязательные тесты закрытия:**

- [ ] cross-module ordering по descriptor metadata;
- [ ] deterministic order при отсутствии явной зависимости;
- [ ] missing dependency/cycle завершаются контролируемой ошибкой.

#### 2.3 CI non-regression как контракт

- [x] Обновить `docs/verification/platform-quality-operations-verification-plan.md` (явно зафиксировать gates).
- [x] Добавить script/test для проверки существования всех Dependabot directories (`scripts/ci/check-dependabot-directories.py`).
- [x] Вынести coverage threshold в единый источник (`env`/constant), чтобы docs/workflow не расходились (`scripts/ci/coverage-threshold.env` + `scripts/ci/check-coverage.sh`).

**Обязательные тесты закрытия:**

- [x] script падает на несуществующем dependabot directory (добавлен `scripts/tests/check_dependabot_directories_test.sh`);
- [x] CI использует общий threshold source;
- [x] docs описывают актуальные gates без расхождения с workflow.

### Этап 3 (P2): документация, ADR, финальная синхронизация

- [ ] Обновить central docs (`docs/architecture/modules.md`, `docs/modules/manifest.md`) по фактическому runtime contract.
- [ ] Обновить локальные docs server/admin по новым control-plane и lifecycle flows.
- [ ] Зафиксировать ADR для hook semantics и/или migration descriptor contract (если изменены публичные архитектурные договорённости).
- [ ] Обновить `docs/index.md` при добавлении/переименовании документов.
- [ ] В `deep-research-report (4).md` оставить ссылку на этот план как на актуальный remediation backlog.

### Определение “полной реализации” для этого плана

План считается полностью реализованным только когда одновременно выполнены условия:

1. Все чекбоксы этапов 1–3 отмечены `[x]`.
2. P0/P1/P2 критерии готовности из этого документа закрыты.
3. Минимальный verification набор выполнен на ветке реализации.
4. GraphQL и Leptos SSR surface parity подтверждена тестами и отсутствием bypass-путей.
5. Центральная и локальная документация синхронизирована с фактическим кодом.

---

## Продолжение execution-плана до полного закрытия (operational track)

Ниже добавлен практический трек “как довести до done без провалов в parity/регрессиях”.
Этот блок не заменяет этапы выше, а задаёт последовательность внедрения, expected artifacts,
rollback-стратегии и Definition of Done по итерациям.

### Итерация A — закрыть P0.1 (atomic composition/build)

**Scope.**

- [ ] Ввести единый server-side orchestration API для composition update + build enqueue.
- [ ] Обеспечить единый transaction boundary (`platform_state` + `builds`).
- [ ] Переключить GraphQL mutation path на orchestration API.
- [ ] Переключить Leptos SSR path на тот же orchestration API/adapter.

**Deliverables.**

- [ ] Новый сервисный слой (или расширение существующего) с public методом вида
      `apply_manifest_and_enqueue_build(expected_revision, actor, reason, source)`.
- [ ] Удалены/ограничены raw SQL path-ы, которые отдельно пишут `platform_state` и `builds`.
- [ ] Единая error taxonomy для GraphQL и Leptos SSR (conflict/validation/internal).

**Негативные сценарии, обязательные до merge.**

- [ ] Конфликт CAS: stale revision → `platform_state` не меняется, build не создаётся.
- [ ] Ошибка insert в `builds` → транзакция откатывается, revision не инкрементируется.
- [ ] Ошибка валидации manifest → нет ни update state, ни build enqueue.

### Итерация B — закрыть P0.2 (canonical lifecycle entrypoint)

**Scope.**

- [x] Единственный production entrypoint: `toggle_module_with_actor()`.
- [x] Все admin/runtime surfaces используют только canonical lifecycle service.
- [x] Bypass API в model-layer недоступен из production кода.

**Deliverables.**

- [x] Явный “unsafe for migrations/tests only” contract для low-level toggle API.
- [x] Repo-guard (test/script), блокирующий production references bypass API.
- [ ] Unified parity tests: GraphQL vs Leptos SSR.

**Негативные сценарии, обязательные до merge.**

- [ ] Unknown module, core module, missing dependency, dependent modules: одинаковые ошибки в обоих surfaces.
- [ ] При успешном toggle всегда пишется journal (`module_operations`) и actor/correlation metadata.

### Итерация C — закрыть P0.3 (hook semantics + recovery)

**Scope.**

- [ ] Реализовать lifecycle state-machine (`validated/running/committed/failed`).
- [ ] Развести pre/post hooks по моменту исполнения и гарантиям.
- [ ] Внедрить retryable post-hook issues без отката committed state.

**Deliverables.**

- [ ] Compat-layer для legacy `on_enable`/`on_disable` с явно задокументированным поведением.
- [ ] Journal schema/fields достаточны для manual и automated retry.
- [ ] Операционные runbook-инструкции в docs для retry/compensation.

**Негативные сценарии, обязательные до merge.**

- [ ] Pre-hook failure: effective state без изменений, operation помечен `failed`.
- [ ] Post-hook failure: effective state сохранён, operation помечен как retryable issue.
- [ ] Повторный retry не дублирует side effects (idempotency).

### Итерация D — закрыть P1.1/P1.2 (hash contract + migrations ordering)

**Scope.**

- [ ] Полный canonical manifest snapshot serializer.
- [ ] SHA-256 (hex, 64 chars) как единственный hash contract.
- [ ] Migration dependency descriptors в module-owned exporters.
- [ ] Topological sort + lexical tie-breaker + жёсткие ошибки для missing/cycle.

**Deliverables.**

- [ ] Общий hash builder для GraphQL, Leptos SSR, BuildService.
- [ ] Удалён hardcoded dependency match из migrator core.
- [ ] Расширенные tests на determinism ordering и dependency validation.

### Итерация E — закрыть P1.3/P2 (CI contract + docs/ADR sync)

**Scope.**

- [ ] Явная фиксация quality gates как non-regression contract.
- [ ] Проверка актуальности Dependabot directories.
- [ ] Единый источник coverage threshold.
- [ ] Синхронизация central/local docs + ADR updates.

**Deliverables.**

- [ ] Обновлён verification-plan с checklist’ом gates.
- [ ] Script/test для dependabot directory validation.
- [ ] ADR(ы) по hook semantics и/или migration descriptor contract (если публичный контракт изменён).
- [ ] `deep-research-report (4).md` помечен как historical input + ссылка на этот remediation backlog.

---

## Release-gate checklist (must-pass перед финальным закрытием плана)

### 1) Код/архитектура

- [ ] Нет production-кода, который отдельно обновляет `platform_state` и отдельно enqueue-ит build.
- [ ] Нет production-вызовов прямого bypass toggle API.
- [ ] Hook pipeline соответствует model `pre -> commit -> post` без частичного rollback.

### 2) Тесты

- [ ] Contract tests для GraphQL/Leptos SSR parity (manifest + lifecycle).
- [ ] Failure-mode tests для CAS/build enqueue и pre/post hooks.
- [ ] Migration ordering tests (descriptor, missing, cycle, determinism).

### 3) Документация

- [ ] Central docs и локальные docs описывают фактический runtime contract.
- [ ] ADR(ы) добавлены/обновлены при изменении архитектурного контракта.
- [ ] `docs/index.md` содержит ссылки на все новые/переименованные документы.

### 4) Операционные проверки

- [ ] Минимальный verification набор из этого плана прогнан на ветке.
- [ ] CI-gates подтверждены как non-regression (manifest/module validation, coverage, SBOM, license/deps policy).

---

## Стратегия внедрения без простоя (recommended cutover order)

1. Ввести новые сервисы/контракты “рядом” со старыми path-ами под feature-safe переключением.
2. Переключить GraphQL path и зафиксировать parity tests.
3. Переключить Leptos SSR path на тот же backend entrypoint.
4. Удалить legacy/raw SQL/bypass path-ы только после зелёных regression tests.
5. Зафинализировать docs/ADR и закрыть чекбоксы этапов 1–3.
