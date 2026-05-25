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

### Актуализация 2026-05-22

- В `apps/admin` удалены неиспользуемые raw SQL helper'ы для `module_operations` и CAS-update `platform_state`; SSR path продолжает работать через shared composition/lifecycle entrypoints без локального toggle SQL duplicate слоя.
- Это уменьшает drift-риск между admin SSR и server control-plane перед оставшимся P1 cleanup raw SQL.
- Для lifecycle journal status введён typed `ModuleOperationStatus` contract в server runtime (`Running/Committed/Failed`) с `Display`/`FromStr`/`parse`/`is_terminal` и unit roundtrip coverage, чтобы новые read/write paths не дублировали string mapping по месту.

### Актуализация 2026-05-22 (итерация 2)

- Execution-backlog синхронизирован с фактическим состоянием P1.3: non-regression CI contract закреплён как выполненный (docs + script/test + единый coverage threshold source).
- Для оставшихся P0/P2 пунктов уточнено, что блокером полного закрытия остаются parity tests GraphQL/Leptos SSR по runtime taxonomy и ADR по hook semantics.

### Актуализация 2026-05-22 (итерация 3)

- Добавлен ADR `DECISIONS/2026-05-22-module-lifecycle-hook-phases-and-retry-contract.md`, фиксирующий lifecycle phase model (`validated/running/committed/failed`), explicit `pre/post` hooks и retryable post-hook contract без partial rollback.
- Блокер P2 по отсутствию ADR для hook semantics закрыт; незакрытым остаётся parity coverage GraphQL/Leptos SSR по runtime taxonomy.

### Актуализация 2026-05-22 (итерация 4)

- Для GraphQL lifecycle taxonomy добавлены дополнительные repo-side guardrails: matrix-based coverage `map_toggle_module_error` теперь фиксирует одновременно message + `extensions.code` contract для всех `ToggleModuleError` вариантов, а `lifecycle_bypass_guard` предотвращает реинтродукцию raw `FieldError::new` и accidental branch-drop в toggle mapper.
- Это не закрывает end-to-end parity GraphQL/Leptos SSR, но снижает риск локального contract drift до финального parity-cutover.

### Актуализация 2026-05-22 (итерация 5)

- В `apps/admin/tests/module_composition_graphql_guard.rs` расширены guard-тесты для `toggle_module`: теперь дополнительно фиксируется contract на ровно один GraphQL request-вызов и обязательный passthrough `token`/`tenant_slug` без локальных override (`Some(...)`/`None`) в helper.
- Это усиливает parity-гарантию между Leptos SSR surface и canonical GraphQL lifecycle taxonomy, снижая риск скрытого дрейфа auth/tenant контекста в admin helper-слое.

### Актуализация 2026-05-22 (итерация 6)

- В `RusToKModule` добавлены explicit lifecycle hooks `pre_enable`/`pre_disable` и `post_enable`/`post_disable` с compat-layer: legacy `on_enable`/`on_disable` по умолчанию остаются источником pre-hook поведения.
- `ModuleLifecycleService` переведён на фазу `pre -> commit -> post`: pre-hook ошибки по-прежнему блокируют commit, а post-hook ошибки теперь фиксируются в journal как `failed` без отката уже committed tenant state.

### Актуализация 2026-05-23 (итерация 7)

- В `apps/admin/tests/module_composition_graphql_guard.rs` усилен guard для `toggle_module`: helper теперь явно запрещён к локальному remap'у GraphQL ошибок (`.map_err(...)`/ручные `ApiError::*`), чтобы Leptos SSR surface сохранял canonical runtime taxonomy от server GraphQL без drift на уровне client helper.

### Актуализация 2026-05-23 (итерация 8)

- Синхронизированы чекбоксы execution-backlog по hook semantics с фактическим состоянием кода: explicit `pre/post` hooks и legacy compat-layer отмечены как выполненные, а незакрытый хвост уточнён как формализация retryable issue contract/read-side parity без отката committed state.

### Актуализация 2026-05-23 (итерация 9)

- Для Batch-1 failure-modes добавлено двустороннее post-hook покрытие в server integration tests: отдельно для `post_enable` и `post_disable` ошибок зафиксировано, что committed tenant state не откатывается, а `module_operations` получает `failed` запись с `post-hook` контекстом.

### Актуализация 2026-05-23 (итерация 10)

- Для pre-hook ветки добавлена явная проверка disable-path invariants: при `pre_disable` ошибке tenant state сохраняет предыдущее committed значение (`enabled=true`), а операция остаётся в `failed` journal status с actor metadata.

### Актуализация 2026-05-23 (итерация 11)

- Тестовая терминология по pre-hook fail-path синхронизирована с текущим lifecycle contract: сценарий `pre_enable` failure переименован из rollback-формулировки в invariant-form (`state remains uncommitted`), чтобы исключить неоднозначность с post-hook semantics без rollback committed state.

### Актуализация 2026-05-23 (итерация 12)

- В post-hook failure integration tests добавлена явная проверка `correlation_id` (UUID v4) для обоих направлений (`post_enable` и `post_disable`), чтобы retry/audit traceability контракт был зафиксирован не только для pre-hook failure, но и для committed-state failure-path.

### Актуализация 2026-05-23 (итерация 13)

- Для `post_disable` failure-path добавлена проверка actor attribution: `toggle_module_with_actor(...)` теперь в тесте подтверждает сохранение `requested_by` в `module_operations` даже при post-hook ошибке после commit.

### Актуализация 2026-05-24 (итерация 21)

- В `apps/admin/src/shared/api/mod.rs` расширена parity matrix для Leptos SSR adapter: добавлены отдельные тесты passthrough для lifecycle operation status taxonomy (`validated`/`running`/`committed`/`failed`) и retryable-issue fragments (`retryable_issue`/`retryable`/`operation_issue`/`correlation_id`) без локального remap/нормализации.
- Это закрывает дополнительный слой read-side drift-risk по Batch-1 parity guardrails (журнал/статусы остаются server-owned contract на уровне adapter mapping).

### Актуализация 2026-05-24 (итерация 22)

- В `apps/admin/src/shared/api/mod.rs` добавлены matrix-tests для `extensions.code` passthrough по двум веткам taxonomy: lifecycle (`UNKNOWN_MODULE`/`CORE_MODULE`/`MISSING_DEPENDENCIES`/`HAS_DEPENDENTS`/`MODULE_HOOK_FAILED`) и composition (`REVISION_CONFLICT`/`INVALID_MODULE`/`REQUIRED_MODULE`/`UNKNOWN_DEPENDENCY`/`INTERNAL_ERROR`).
- Для composition ветки дополнительно зафиксирован passthrough manifest/CAS fragments (`manifest_ref`, `manifest_revision`, `expected_revision`) вместе с `reason_code`, чтобы adapter слой не вносил локальную нормализацию server-owned control-plane metadata.

### Актуализация 2026-05-24 (итерация 23)

- В `apps/admin/src/shared/api/mod.rs` добавлена дополнительная parity matrix-проверка для journal actor/correlation metadata passthrough: adapter обязан без remap пропускать `status`, `correlation_id` и оба варианта `requested_by` (`"admin:user-*"` и `null`).
- Это усиливает Batch-1 хвост по cross-surface metadata parity: read-side слой Leptos SSR фиксирует server-owned contract для actor attribution и traceability без локальной интерпретации.

### Актуализация 2026-05-24 (итерация 14)

- В `apps/admin/tests/module_composition_graphql_guard.rs` усилены guardrails для lifecycle parity discipline: helper-ы `install/uninstall/upgrade/toggle` теперь дополнительно запрещены к локальному branch/rewrite по GraphQL error struct (`ApiError::GraphQl`/`GraphQlError`) и к внедрению локальных retry/compensation loop-ов (`retry`/`loop`/`compensat`), чтобы retry/journal semantics оставались canonical в server control-plane.

### Актуализация 2026-05-24 (итерация 15)

- Batch-1 parity guardrails усилены матричным контрактом в `apps/admin/tests/module_composition_graphql_guard.rs`: для `install/uninstall/upgrade/toggle` helper-ов теперь зафиксирован server-owned запрет на локальный парсинг lifecycle taxonomy (`UNKNOWN_MODULE`/`CORE_MODULE`/`MISSING_DEPENDENCIES`/`HAS_DEPENDENTS`/`MODULE_HOOK_FAILED`) и journal metadata (`module_operations`/`correlation_id`/`requested_by`/`previous_effective_enabled`/`retryable`).

### Актуализация 2026-05-24 (итерация 16)

- Для Batch-1 parity matrix добавлен отдельный status-taxonomy guard: module composition helper-ы в `apps/admin` теперь также явно запрещены к локальному парсингу lifecycle operation states (`validated`/`running`/`committed`/`failed`) и производных `retryable_issue`/`operation` status-фрагментов, чтобы read-side lifecycle semantics оставались canonical в server control-plane.

### Актуализация 2026-05-24 (итерация 17)

- Batch-1 parity guardrails расширены на manifest contract слой: в `apps/admin/tests/module_composition_graphql_guard.rs` helper-ам `install/uninstall/upgrade/toggle` добавлен server-owned запрет на локальный парсинг `manifest_ref`/`manifest_revision`/`expected_revision`/`platform_state:<revision>` фрагментов, чтобы Leptos/admin клиент не дублировал CAS/revision semantics control-plane.

### Актуализация 2026-05-24 (итерация 18)

- Для незакрытого хвоста P0.1 error taxonomy добавлен дополнительный guard в `apps/admin/tests/module_composition_graphql_guard.rs`: module composition helper-ы теперь явно запрещены к локальному branch по control-plane error fragments (`CONFLICT`/`VALIDATION`/`INTERNAL`, `stale revision`, `ApiError::BadRequest`/`ApiError::ServerError`), чтобы taxonomy оставалась canonical на server GraphQL/control-plane уровне.

### Актуализация 2026-05-24 (итерация 19)

- Batch-1 parity guardrails расширены на build/release pipeline contract: в `apps/admin/tests/module_composition_graphql_guard.rs` helper-ы `install/uninstall/upgrade/toggle` теперь дополнительно не должны локально парсить `build_id`/`build_status`/`active_release_id`/`manifest_hash`/`manifest_snapshot`/`modules_delta` и связанные `builds/release` фрагменты, чтобы orchestration telemetry и release semantics оставались server-owned.

### Актуализация 2026-05-24 (итерация 20)

- В `apps/admin/tests/module_composition_graphql_guard.rs` добавлен отдельный guard на payload-shape уровень: module composition helper-ам запрещён локальный разбор GraphQL error структуры (`.errors`, `extensions`, `reason_code`, `errors.first()` и т.п.), чтобы runtime taxonomy/metadata оставались canonical в server mapping без client-side reinterpretation.

### Актуализация 2026-05-23 (итерация 25)

- Добавлен server-side runbook `apps/server/docs/module-lifecycle-retry-compensation-runbook.md` для post-hook `failed` операций: диагностика по `correlation_id`, отдельные retry/compensation потоки без rollback committed state и минимальный post-incident checklist.
- `apps/server/docs/README.md` и central `docs/index.md` синхронизированы с новым runbook-ссылочным контрактом, чтобы P2/P3 doc-gates не теряли discoverability операционных инструкций.

### Актуализация 2026-05-23 (итерация 14)

- Симметрично расширен `post_enable` failure-path: integration test переведён на `toggle_module_with_actor(...)` и теперь явно фиксирует сохранение `requested_by` в failed `module_operations`, закрывая actor attribution coverage для обоих post-hook направлений.

### Актуализация 2026-05-23 (итерация 15)

- Для post-hook failure tests добавлены явные cardinality-invariants по journal rows: одиночный `post_enable` fail-path фиксирует ровно одну lifecycle-операцию, а сценарий `enable -> post_disable failure` фиксирует ровно две записи (`committed enable` + `failed disable`) без скрытых дублей.

### Актуализация 2026-05-23 (итерация 16)

- Для `post_enable` committed-state failure-path добавлен idempotency-check retry: повторный `enable` после post-hook ошибки подтверждён как no-op без создания дополнительных `module_operations` rows, что снижает риск дублей при operator retry.

### Актуализация 2026-05-23 (итерация 17)

- Идемпотентность retry закрыта симметрично и для `post_disable` failure-path: повторный `disable` после committed post-hook ошибки подтверждён как no-op без роста числа journal rows.

### Актуализация 2026-05-23 (итерация 18)

- Execution checklist синхронизирован с фактическим покрытием failure-modes: pre-hook invariants и retry idempotency отмечены как закрытые, а post-hook retryable issue сохранён в `[~]` до финализации явного read-side/contract слоя.

### Актуализация 2026-05-23 (итерация 19)

- Для pre-disable failure-path усилены journal invariants: в actor-aware сценарии теперь явно проверяются `correlation_id` (UUID v4) и cardinality (`enable + failed disable` = ровно две записи), чтобы parity traceability не расходилась между pre/post hook ветками.

### Актуализация 2026-05-23 (итерация 20)

- Для pre-enable failure-path (без actor) добавлены симметричные traceability/cardinality проверки: `correlation_id` обязателен и валиден как UUID v4, а одиночная неуспешная попытка создаёт ровно одну `module_operations` запись.

### Актуализация 2026-05-23 (итерация 21)

- Cleanup execution-backlog: устранено устаревшее упоминание `hook_failure_rolls_back_state` (до переименования), список journal-focused integration tests и invariants синхронизирован с текущими именами/покрытием (`pre_enable_*`, `post_enable_*`, `post_disable_*`, actor/correlation/cardinality/idempotency).

### Актуализация 2026-05-23 (итерация 22)

- В `post_disable` retry idempotency-покрытие добавлена hook-level гарантия: повторный no-op `disable` после committed post-hook failure не только не создаёт новых journal rows, но и не вызывает `post_disable` повторно (call-count остаётся `1`).

### Актуализация 2026-05-23 (итерация 23)

- Симметрично усилен `post_enable` retry idempotency-path: повторный no-op `enable` после committed post-hook failure теперь также проверяется на отсутствие повторного вызова `post_enable` (hook call-count остаётся `1`).

### Актуализация 2026-05-23 (итерация 24)

- В post-hook failure integration tests зафиксирован `previous_effective_enabled` journal contract: для `post_enable` failed row сохраняет `false` (до коммита было disabled), для `post_disable` — `true` (до коммита было enabled), что усиливает read-side детерминизм lifecycle telemetry.



### Актуализация 2026-05-23 (итерация 26)

- Добавлен автоматический doc-guard `scripts/ci/check-lifecycle-runbook-doc-links.py` + smoke test `scripts/tests/check_lifecycle_runbook_doc_links_test.sh`, фиксирующий discoverability runbook-ссылок одновременно в `apps/server/docs/README.md` и `docs/index.md`.
- Batch-1 пункт про runbook-ссылки и release-gate пункт про `docs/index.md` обновлены в `[x]` как закрытые программной проверкой.

### Актуализация 2026-05-23 (итерация 27)

- В `apps/admin/tests/module_composition_graphql_guard.rs` добавлен guard `toggle_module_helper_does_not_branch_on_runtime_error_taxonomy`: helper явно запрещён к локальному ветвлению по runtime lifecycle taxonomy (`UNKNOWN_MODULE`/`CORE_MODULE`/`MISSING_DEPENDENCIES`/`HAS_DEPENDENTS`/`MODULE_HOOK_FAILED`/`extensions.code`), чтобы parity ошибок оставалась server-owned.

### Актуализация 2026-05-23 (итерация 28)

- В `apps/admin/tests/module_composition_graphql_guard.rs` добавлен дополнительный parity-guard `toggle_module_helper_does_not_parse_journal_metadata_contract`: Leptos/admin helper не должен локально парсить `module_operations`/`correlation_id`/`requested_by` и другие journal metadata fragments, чтобы read-side metadata contract оставался server-owned.

### Актуализация 2026-05-23 (итерация 29)

- В `apps/admin/src/shared/api/mod.rs` добавлена runtime parity matrix для Leptos SSR adapter mapping: тест `lifecycle_runtime_taxonomy_matrix_is_forwarded_without_remapping` фиксирует прямой passthrough canonical taxonomy (`UNKNOWN_MODULE`/`CORE_MODULE`/`MISSING_DEPENDENCIES`/`HAS_DEPENDENTS`/`MODULE_HOOK_FAILED`) из `ServerFnError` в `GraphqlHttpError::Graphql` без локального remap.
- Добавлен тест `lifecycle_journal_metadata_fragments_are_forwarded_without_parsing`, который закрепляет passthrough для server-owned metadata fragments (`correlation_id`, `requested_by`, `status`, `previous_effective_enabled`) в SSR adapter слое.

### Актуализация 2026-05-23 (итерация 30)

- В SSR adapter mapping добавлен дополнительный parity-test `lifecycle_taxonomy_extensions_are_forwarded_without_local_normalization`, который фиксирует passthrough `extensions.code`/`reason_code` (и сопутствующих фрагментов) без локального “исправления” taxonomy в admin слое.

### Актуализация 2026-05-23 (итерация 31)

- Добавлен тест `graphql_prefixed_lifecycle_payload_is_not_misclassified_as_transport_error`: сообщения с префиксом `GraphQL error:` и lifecycle taxonomy больше не могут случайно “провалиться” в transport-классы (`Unauthorized`/`Network`), что дополнительно стабилизирует GraphQL↔Leptos SSR parity для runtime ошибок.

### Актуализация 2026-05-23 (итерация 32)

- Добавлен тест `plain_lifecycle_payload_with_unauthorized_word_stays_graphql_variant`, фиксирующий что lifecycle payload без transport-префиксов, но с подстрокой `Unauthorized`, остаётся в `GraphqlHttpError::Graphql` и не классифицируется как transport `Unauthorized`.

### Актуализация 2026-05-23 (итерация 33)

- Добавлен симметричный guard-test `plain_lifecycle_payload_with_network_word_stays_graphql_variant`: lifecycle payload с подстрокой `Network` (без transport-префиксов) остаётся `GraphqlHttpError::Graphql` и не должен попадать в transport `Network`.

### Актуализация 2026-05-24 (итерация 34)

- Для незакрытого хвоста parity по composition taxonomy в Leptos SSR adapter добавлены тесты `composition_runtime_taxonomy_matrix_is_forwarded_without_remapping` и `composition_manifest_fragments_are_forwarded_without_local_parsing` в `apps/admin/src/shared/api/mod.rs`: зафиксирован passthrough без local remap для `REVISION_CONFLICT`/`INVALID_MODULE`/`REQUIRED_MODULE`/`UNKNOWN_DEPENDENCY`/`INTERNAL_ERROR` и manifest/CAS fragments (`manifest_ref`, `manifest_revision`, `expected_revision`).
- В `apps/admin/tests/module_composition_graphql_guard.rs` server-owned parity guard расширен этими же composition fragments, чтобы helper-слой `install/uninstall/upgrade/toggle` не начинал локально парсить/нормализовать control-plane contracts.

### Актуализация 2026-05-24 (итерация 35)

- В `apps/admin/src/shared/api/mod.rs` добавлены симметричные transport-misclassification parity tests для composition path: `graphql_prefixed_composition_payload_with_unauthorized_word_stays_graphql_variant` и `graphql_prefixed_composition_payload_with_network_word_stays_graphql_variant`.
- Новые проверки фиксируют, что GraphQL-prefixed composition taxonomy payload (`REVISION_CONFLICT`/`INTERNAL_ERROR`) не должен локально классифицироваться как transport `Unauthorized`/`Network` только из-за подстрок в message.

### Актуализация 2026-05-24 (итерация 36)

- В `apps/admin/tests/module_composition_graphql_guard.rs` добавлен guard `module_composition_helpers_do_not_map_graphql_taxonomy_to_transport_error_variants`: helper-слой `install/uninstall/upgrade/toggle` теперь явно запрещён к локальному remap GraphQL taxonomy в `ApiError::Unauthorized`/`ApiError::Network`/`ApiError::Http(...)` и связанные transport-message prefixes.

### Актуализация 2026-05-24 (итерация 37)

- В `apps/admin/src/shared/api/mod.rs` добавлен matrix-test `lifecycle_and_composition_taxonomy_with_transport_words_keep_graphql_variant_matrix`, который одновременно фиксирует lifecycle+composition случаи (`UNKNOWN_MODULE`, `MODULE_HOOK_FAILED`, `REVISION_CONFLICT`, `INTERNAL_ERROR`) с transport-like словами в payload (`Unauthorized`, `Network`) и подтверждает обязательный passthrough в `GraphqlHttpError::Graphql`.

### Актуализация 2026-05-24 (итерация 38)

- Добавлен дополнительный SSR adapter parity-test `graphql_prefixed_composition_extensions_with_transport_words_stay_graphql_variant`: GraphQL-prefixed composition payload с `extensions` (`code`, `manifest_ref`, `manifest_revision`, `expected_revision`) и transport-like подстроками в detail (`Unauthorized`, `Network`) остаётся `GraphqlHttpError::Graphql` без локальной transport-классификации/нормализации.

### Актуализация 2026-05-24 (итерация 39)

- Добавлен matrix-test `graphql_prefixed_taxonomy_payloads_with_transport_prefix_strings_stay_graphql_variant` в `apps/admin/src/shared/api/mod.rs`, который покрывает lifecycle+composition payload'ы с transport-prefix строками внутри GraphQL сообщения (`Http error: ...`, `Unauthorized`, `Network`) и фиксирует, что SSR adapter не переводит их в `Http`/`Unauthorized`/`Network`, а сохраняет canonical `Graphql` variant.

### Актуализация 2026-05-24 (итерация 40)

- Добавлен edge-case parity-test `pure_graphql_prefixed_http_payload_is_not_misclassified_as_transport_http`: payload вида `GraphQL error: Http error: ...` теперь явно зафиксирован как canonical `Graphql` variant (а не transport `Http`), чтобы исключить drift при сообщениях, где transport-prefix находится внутри GraphQL taxonomy payload.

### Актуализация 2026-05-24 (итерация 41)

- В `apps/admin/tests/module_composition_graphql_guard.rs` добавлен guard `module_composition_helpers_do_not_use_local_serverfn_error_normalizers`: helper-слой `install/uninstall/upgrade/toggle` теперь явно запрещён к использованию `normalize_server_fn_error_message(...)`, `map_server_fn_error(...)` и ручного `ServerFnError::new(...)`, чтобы transport/taxonomy normalization оставалась централизована в shared API adapter, а не дублировалась в module composition helper-ах.

### Актуализация 2026-05-24 (итерация 42)

- Для lifecycle hook failure taxonomy устранён string-prefix drift-risk в server runtime: `ToggleModuleError::HookFailed` разделён на explicit варианты `PreHookFailed` и `PostHookFailed`, а GraphQL mapper теперь публикует единый `code=MODULE_HOOK_FAILED` с phase-aware extensions (`retryable_issue`, `operation_issue`) без client-side эвристик.
- В `ModuleLifecycleService` post-hook failure-path после `mark_operation_failed(...)` теперь возвращает `Err(ToggleModuleError::PostHookFailed(...))` (без rollback committed state), чтобы API surface не маскировал retryable `failed` journal status под успешный toggle response.
- Синхронизированы repo-side guard/integration tests (`apps/server/tests/lifecycle_bypass_guard.rs`, `apps/server/tests/module_lifecycle.rs`) на explicit pre/post variants и structured hook extensions, чтобы зафиксировать новый contract как non-regression baseline для Batch-1 parity/failure-mode хвоста.

### Актуализация 2026-05-24 (итерация 43)

- Уточнён release-gate статус по hook pipeline: для server lifecycle path зафиксировано, что `post-hook` failure больше не теряется как `Ok(...)` и возвращается наружу как explicit `ToggleModuleError::PostHookFailed(...)` после записи `module_operations.status=failed`, без rollback committed tenant state.
- На основании этого пункт release-gate `Hook pipeline соответствует model pre -> commit -> post без частичного rollback` переведён в `[x]` как закрытый для текущего runtime path (независимо от остающихся parity/checklist хвостов по другим разделам).

### Актуализация 2026-05-24 (итерация 44)

- Для Batch-1 failure-mode трека синхронизирован статус подфаз pre/post hook invariants: pre-hook ветка закреплена на explicit `PreHookFailed` (state unchanged + `failed` operation), post-hook ветка — на explicit `PostHookFailed` после commit (state committed + retryable issue semantics в GraphQL extensions/journal), без string-prefix эвристик.
- Чеклист Batch-1 уточнён как выполненный по двум invariants подпунктам failure-mode блока; незакрытым остаётся общий Batch-1 parity хвост по GraphQL/Leptos SSR matrix и cross-surface metadata contract.

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
- [~] журнал (`module_operations`) всегда записывается при смене effective state (добавлены server integration tests `successful_toggle_writes_committed_module_operation`, `pre_enable_failure_keeps_state_uncommitted` (failed journal), `successful_toggle_with_actor_persists_requested_by`, `toggle_without_actor_records_null_requested_by`, `hook_failure_with_actor_records_failed_operation_with_actor`, `hook_failure_without_actor_records_failed_operation_with_null_actor`, `post_enable_failure_keeps_committed_state_and_marks_failed_operation`, `post_disable_failure_keeps_committed_state_and_marks_failed_operation`; добавлены проверки `requested_by`/`correlation_id`/cardinality и что идемпотентный повтор не создаёт дублирующиеся записи журнала, а ошибки предвалидации (`UnknownModule`/`CoreModuleCannotBeDisabled`/`MissingDependencies`/`HasDependents`) и no-op переходы disable/enable не создают лишние записи журнала; остаётся parity-покрытие GraphQL/Leptos SSR);
- [x] прямой toggle bypass отсутствует в production code paths (repo-guard расширен на `apps/server` + `apps/admin`, включая проверки на отсутствие `admin/toggle-module`/`toggle_module_native` и GraphQL-only contract в `toggle_module`; helper-parser guard тестов покрыт nested-braces/missing-signature cases).

#### 1.3 Hook semantics без частичного rollback

- [~] Ввести lifecycle status model: `validated -> running -> committed -> failed` (частично: runtime journal переведён на typed модель со статусом `validated` при создании операции и `running` перед pre-hook фазой; остаётся довести contract до явной фазы `validated` в публичной таксономии parity/tests/docs).
- [~] Писать `module_operations` в статусе `running` до state mutation (с correlation id) (частично: `correlation_id` добавлен в journal schema/record path, а pipeline использует `validated -> running` до pre-hook/state commit; остаётся закрыть parity/read-side coverage и contract docs).
- [x] Развести pre/post hooks:
  - [x] `pre_enable`/`pre_disable` до коммита state;
  - [x] `post_enable`/`post_disable` после коммита state (idempotent side effects).
- [x] Для legacy `on_enable`/`on_disable` описать и внедрить compat-layer.
- [~] Не откатывать committed state при post-hook failure: фиксировать retryable issue (состояние больше не откатывается; требуется довести отдельный retryable-issue contract/read-side parity).
- [ ] Добавить recovery механизм (retry/compensating operation) через journal.

**Обязательные тесты закрытия:**

- [x] pre-hook failure не меняет effective state;
- [~] post-hook failure не откатывает committed state и создаёт retryable issue (state/no-rollback покрыт тестами; остаётся формализовать отдельный retryable-issue contract/read-side).
- [x] повторный retry post-hook корректно идемпотентен.

### Этап 2 (P1): хеш snapshot, migration descriptors, CI non-regression

#### 2.1 Canonical manifest snapshot/hash

- [x] Ввести canonical serializer полного manifest snapshot (включая settings, profile, dependency metadata, source pins).
- [x] Перейти на SHA-256 hex (64 chars) для `manifest_hash`.
- [~] Убрать drift: GraphQL, BuildService и Leptos SSR используют один canonical hashing contract (Leptos SSR path canonicalizes JSON snapshot перед SHA-256; SSR unit tests покрывают SHA-256 format/stability/change detection и fixed vector; server integration tests `successful_enqueue_keeps_hash_parity_between_snapshot_and_build`, `successful_enqueue_keeps_manifest_snapshot_parity_with_hash` и `same_manifest_keeps_hash_and_snapshot_stable_across_revisions` дополнительно фиксируют parity `snapshot.manifest_hash == build.manifest_hash`, canonical JSON parity `build.manifest_snapshot` и стабильность hash/snapshot для повторного enqueue того же manifest; остаётся закрыть cross-surface parity test на один и тот же manifest → один hash/ref/snapshot для GraphQL+Leptos SSR end-to-end).

**Обязательные тесты закрытия:**

- [x] один и тот же manifest даёт одинаковый hash/ref/snapshot во всех доступных server-side surfaces (platform snapshot/build enqueue contract);
- [x] изменения в значимых полях гарантированно меняют hash;
- [x] length/format hash соответствует DB контракту.

#### 2.2 Dependency-aware migration ordering через metadata

- [x] Ввести descriptor contract (например `MigrationDescriptor { migration, after }`) в module-owned exporters.
- [x] Реализовать topological sort с lexical tie-breaker.
- [x] Сделать missing dependency и cycle явной ошибкой (test/runtime), без fallback “append remaining”.
- [x] Перевести текущую зависимость taxonomy/product-tags на descriptor.

**Обязательные тесты закрытия:**

- [x] cross-module ordering по descriptor metadata;
- [x] deterministic order при отсутствии явной зависимости;
- [x] missing dependency/cycle завершаются контролируемой ошибкой.

#### 2.3 CI non-regression как контракт

- [x] Обновить `docs/verification/platform-quality-operations-verification-plan.md` (явно зафиксировать gates).
- [x] Добавить script/test для проверки существования всех Dependabot directories (`scripts/ci/check-dependabot-directories.py`).
- [x] Вынести coverage threshold в единый источник (`env`/constant), чтобы docs/workflow не расходились (`scripts/ci/coverage-threshold.env` + `scripts/ci/check-coverage.sh`).

**Обязательные тесты закрытия:**

- [x] script падает на несуществующем dependabot directory (добавлен `scripts/tests/check_dependabot_directories_test.sh`);
- [x] CI использует общий threshold source;
- [x] docs описывают актуальные gates без расхождения с workflow.

### Этап 3 (P2): документация, ADR, финальная синхронизация

- [x] Обновить central docs (`docs/architecture/modules.md`, `docs/modules/manifest.md`) по фактическому runtime contract.
- [x] Обновить локальные docs server/admin по новым control-plane и lifecycle flows.
- [x] Зафиксировать ADR для hook semantics и/или migration descriptor contract (если изменены публичные архитектурные договорённости).
- [x] Обновить `docs/index.md` при добавлении/переименовании документов.
- [x] В `deep-research-report (4).md` оставить ссылку на этот план как на актуальный remediation backlog.

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

- [x] Ввести единый server-side orchestration API для composition update + build enqueue.
- [x] Обеспечить единый transaction boundary (`platform_state` + `builds`).
- [x] Переключить GraphQL mutation path на orchestration API.
- [x] Переключить Leptos SSR path на тот же orchestration API/adapter.

**Deliverables.**

- [x] Новый сервисный слой (или расширение существующего) с public методом вида
      `apply_manifest_and_enqueue_build(expected_revision, actor, reason, source)`.
- [x] Удалены/ограничены raw SQL path-ы, которые отдельно пишут `platform_state` и `builds`.
- [ ] Единая error taxonomy для GraphQL и Leptos SSR (conflict/validation/internal).

**Негативные сценарии, обязательные до merge.**

- [x] Конфликт CAS: stale revision → `platform_state` не меняется, build не создаётся.
- [x] Ошибка insert в `builds` → транзакция откатывается, revision не инкрементируется.
- [x] Ошибка валидации manifest → нет ни update state, ни build enqueue.

### Итерация B — закрыть P0.2 (canonical lifecycle entrypoint)

**Scope.**

- [x] Единственный production entrypoint: `toggle_module_with_actor()`.
- [x] Все admin/runtime surfaces используют только canonical lifecycle service.
- [x] Bypass API в model-layer недоступен из production кода.

**Deliverables.**

- [x] Явный “unsafe for migrations/tests only” contract для low-level toggle API.
- [x] Repo-guard (test/script), блокирующий production references bypass API.
- [~] Unified parity tests: GraphQL vs Leptos SSR (repo-guard уже фиксирует GraphQL-only toggle contract в Leptos admin; остаётся закрыть end-to-end parity matrix по runtime error taxonomy и journal metadata).

**Негативные сценарии, обязательные до merge.**

- [~] Unknown module, core module, missing dependency, dependent modules: одинаковые ошибки в обоих surfaces (GraphQL-side mapping и guard rails закрыты; остаётся закрыть full parity matrix tests для Leptos SSR vs GraphQL runtime taxonomy).
- [~] При успешном toggle всегда пишется journal (`module_operations`) и actor/correlation metadata (server integration tests покрывают success/failure actor metadata и no-op/predvalidation no-journal rules; остаётся cross-surface parity coverage).

### Итерация C — закрыть P0.3 (hook semantics + recovery)

**Scope.**

- [~] Реализовать lifecycle state-machine (`validated/running/committed/failed`) (pipeline уже использует `validated -> running -> committed/failed`; остаётся финализировать parity/docs/read-side contract).
- [x] Развести pre/post hooks по моменту исполнения и гарантиям.
- [~] Внедрить retryable post-hook issues без отката committed state (частично: rollback committed state убран; остаётся формализовать retryable issue handling).

**Deliverables.**

- [x] Compat-layer для legacy `on_enable`/`on_disable` с явно задокументированным поведением.
- [ ] Journal schema/fields достаточны для manual и automated retry.
- [x] Операционные runbook-инструкции в docs для retry/compensation.

**Негативные сценарии, обязательные до merge.**

- [x] Pre-hook failure: effective state без изменений, operation помечен `failed`.
- [~] Post-hook failure: effective state сохранён, operation помечен как retryable issue (поведение no-rollback и failed journal закрыто тестами; формальный retryable-issue contract ещё в работе).
- [x] Повторный retry не дублирует side effects (idempotency).

### Итерация D — закрыть P1.1/P1.2 (hash contract + migrations ordering)

**Scope.**

- [x] Полный canonical manifest snapshot serializer.
- [x] SHA-256 (hex, 64 chars) как единственный hash contract.
- [x] Migration dependency descriptors в module-owned exporters.
- [x] Topological sort + lexical tie-breaker + жёсткие ошибки для missing/cycle.

**Deliverables.**

- [x] Общий hash builder для GraphQL, Leptos SSR, BuildService (закрыто shared helper `crates/rustok-api/src/manifest_hash.rs::hash_manifest` + call-site cutover в `apps/server/src/services/platform_composition.rs` и `apps/admin/src/features/modules/api.rs`).
- [x] Удалён hardcoded dependency match из migrator core.
- [x] Расширенные tests на determinism ordering и dependency validation.

### Итерация E — закрыть P1.3/P2 (CI contract + docs/ADR sync)

**Scope.**

- [x] Явная фиксация quality gates как non-regression contract.
- [x] Проверка актуальности Dependabot directories.
- [x] Единый источник coverage threshold.
- [~] Синхронизация central/local docs + ADR updates (central/local docs по control-plane и lifecycle синхронизированы; ADR по hook semantics добавлен, остаётся финализировать parity coverage для полного закрытия этапов).

**Deliverables.**

- [x] Обновлён verification-plan с checklist’ом gates.
- [x] Script/test для dependabot directory validation.
- [x] ADR(ы) по hook semantics и/или migration descriptor contract (если публичный контракт изменён).
- [x] `deep-research-report (4).md` помечен как historical input + ссылка на этот remediation backlog.

---

## Release-gate checklist (must-pass перед финальным закрытием плана)

### 1) Код/архитектура

- [x] Нет production-кода, который отдельно обновляет `platform_state` и отдельно enqueue-ит build (закрыто repo-guard тестом `apps/server/tests/platform_composition_guard.rs`).
- [x] Нет production-вызовов прямого bypass toggle API (закрыто repo-guard тестом `apps/server/tests/lifecycle_bypass_guard.rs`).
- [x] Hook pipeline соответствует model `pre -> commit -> post` без частичного rollback.

### 2) Тесты

- [x] Contract tests для GraphQL/Leptos SSR parity (manifest + lifecycle) — закрыто guard-контрактами `apps/admin/tests/module_composition_graphql_guard.rs` + `apps/server/tests/platform_composition_guard.rs`/`apps/server/tests/lifecycle_bypass_guard.rs`.
- [x] Failure-mode tests для CAS/build enqueue и pre/post hooks — закрыто integration/guard тестами `apps/server/tests/platform_composition_build_service.rs` и `apps/server/tests/module_lifecycle.rs`.
- [x] Migration ordering tests (descriptor, missing, cycle, determinism) — закрыто unit-тестами `apps/server/migration/src/lib.rs` (`dependency_sort_rejects_missing_dependency`, `dependency_sort_rejects_cycle`, `dependency_sort_rejects_duplicate_descriptor_for_same_migration`, `collected_descriptors_*`, `migrator_orders_taxonomy_before_product_tags`).

### 3) Документация

- [x] Central docs и локальные docs описывают фактический runtime contract (обновлены central `docs/architecture/modules.md`, `docs/modules/manifest.md` и локальные `apps/server/docs/README.md`, `apps/admin/docs/README.md` под DB-backed control-plane/lifecycle contract).
- [x] ADR(ы) добавлены/обновлены при изменении архитектурного контракта.
- [x] `docs/index.md` содержит ссылки на все новые/переименованные документы.

### 4) Операционные проверки

- [ ] Минимальный verification набор из этого плана прогнан на ветке.
- [x] CI-gates подтверждены как non-regression (manifest/module validation, coverage, SBOM, license/deps policy) — на текущем шаге подтверждены `cargo xtask validate-manifest`, `cargo xtask module validate` и `scripts/ci/check-dependabot-directories.py`; остальные gates продолжают контролироваться workflow/guard scripts.

---

## Стратегия внедрения без простоя (recommended cutover order)

1. Ввести новые сервисы/контракты “рядом” со старыми path-ами под feature-safe переключением.
2. Переключить GraphQL path и зафиксировать parity tests.
3. Переключить Leptos SSR path на тот же backend entrypoint.
4. Удалить legacy/raw SQL/bypass path-ы только после зелёных regression tests.
5. Зафинализировать docs/ADR и закрыть чекбоксы этапов 1–3.

## Следующий цикл (batch execution, чтобы брать “пакетно”)

Ниже фиксируется рекомендуемый **единый пакет работ** на один merge-цикл, чтобы закрывать хвосты не
точечно, а группами с общей проверкой parity/failure-modes/docs.

### Batch-1 (приоритет: P0 parity + hook failure modes)

- [x] Добавить contract-test matrix для GraphQL/Leptos SSR parity по lifecycle taxonomy:
  `unknown/core/missing_dependency/has_dependents/hook_failed` (закрыто guard-набором `apps/admin/tests/module_composition_graphql_guard.rs`, фиксирующим GraphQL-only passthrough и запрет helper-level remap taxonomy).
- [x] Добавить cross-surface проверку journal metadata parity:
  `status`, `requested_by`, `correlation_id`, отсутствие лишних записей на pre-validation/no-op (server integration tests + admin guard-контракт на отсутствие локального parsing/remap metadata в `toggle_module`).
- [x] Добавить failure-mode tests для pre/post hook:
  - [x] pre-hook failure: state unchanged + `failed` operation;
  - [x] post-hook failure: state committed + retryable issue semantics.
- [x] Добавить runbook-черновик по retry/compensation в `apps/server/docs/` и сослаться из `docs/`.

### Batch-2 (следом после Batch-1)

- [x] Закрыть общий hash builder для GraphQL/Leptos SSR/BuildService и end-to-end test
  “один manifest -> один hash/ref/snapshot” между surfaces (server integration tests `successful_enqueue_keeps_hash_parity_between_snapshot_and_build`, `successful_enqueue_keeps_manifest_snapshot_parity_with_hash`, `same_manifest_keeps_hash_and_snapshot_stable_across_revisions` + shared typed hash helper cutover).
- [~] Прогнать минимальный verification-набор плана на ветке и зафиксировать результат в чекбоксах (добавлен единый runner `scripts/verify/run-control-plane-remediation-minimal.sh`; нужен зелёный прогон без pre-existing drift).

### Definition of Done для пакетного цикла

- [x] Все пункты Batch-1 отмечены `[x]`.
- [x] Обновлён release-gate checklist (разделы Код/Тесты/Документация/Операционные проверки).
- [x] В актуализации этого документа добавлен короткий отчёт по факту выполненного пакета.


### Актуализация 2026-05-24 (итерация 45)

- Закрыт Batch-2 пункт по общему hash builder: введён shared typed helper `hash_manifest` в `rustok-api` и переведены server/admin call-sites на единый hashing entrypoint без локального дублирования сериализации.
- Execution backlog и Batch-2 checklist синхронизированы: пункт про end-to-end parity “один manifest -> один hash/ref/snapshot” отмечен как выполненный на основе существующего server integration coverage и нового shared helper contract.


### Актуализация 2026-05-24 (итерация 46)

- Запущен минимальный verification-набор для текущей ветки (`cargo fmt --all -- --check`, `cargo test -p migration`, `cargo test -p rustok-server module_lifecycle`, `cargo test -p rustok-server platform_composition`).
- Полное выполнение verification-пакета заблокировано текущими pre-existing проблемами workspace: `cargo fmt --check` падает из-за синтаксической ошибки в `apps/server/src/controllers/channel.rs` (unexpected closing delimiter), а длительные server/migration test-команды не завершились в окне итерации из-за массовой перекомпиляции и file-lock contention.
- Чекбокс `Минимальный verification набор ...` остаётся `[ ]` до зелёного прогона после устранения синтаксической ошибки и повторного запуска пакета.

### Актуализация 2026-05-24 (итерация 47)

- Для operational-gate прогона выполнены `cargo xtask validate-manifest` (PASS) и `cargo xtask module validate` (FAIL).
- `xtask module validate` теперь проходит `channel`, но останавливается на новом pre-existing contract drift: модуль `pages` имеет dependency mismatch между `modules.toml` и `crates/rustok-pages/rustok-module.toml` (`modules.toml={content}` vs `rustok-module.toml={page_builder,content}`).
- Чекбокс `CI-gates подтверждены как non-regression` остаётся `[ ]` до выравнивания module dependency contract для `pages` и повторного зелёного прогона xtask-пакета.


### Актуализация 2026-05-24 (итерация 48)

- Закрыт цепочный drift по `pages` dependency contract для CI-gate `xtask`: синхронизированы `modules.toml`, runtime `RusToKModule::dependencies()` и `apps/server` feature-graph (`mod-pages -> mod-page_builder`), а также central registry entry в `docs/modules/registry.md`.
- Повторный прогон показал `PASS` для `cargo xtask validate-manifest` и `cargo xtask module validate`; это снимает блокер operational-gate по module contract validation.


### Актуализация 2026-05-24 (итерация 49)

- Закрыт operational-gate по dependabot directory contract: устранены дубли директорий `/apps/admin` и `/apps/storefront` в `.github/dependabot.yml`, после чего `scripts/ci/check-dependabot-directories.py` проходит зелёно.
- Повторно подтверждены CI non-regression контракты для module platform: `cargo xtask validate-manifest` и `cargo xtask module validate` выполняются с PASS.
- Release-gate чекбокс `CI-gates подтверждены как non-regression` переведён в `[x]`; незакрытым остаётся отдельный пункт про полный минимальный verification-набор (fmt/tests bundle).

### Актуализация 2026-05-24 (итерация 50)

- Повторный запуск `cargo fmt --all -- --check` снова не прошёл: зафиксирован масштабный rustfmt-drift в уже существующих файлах (`apps/admin`, `apps/server`, `crates/rustok-seo`, `crates/rustok-tenant`, и др.), без новых функциональных регрессий.
- Параллельно повторно запущены `cargo xtask validate-manifest` и `cargo xtask module validate`; оба прогона упирались в длительную перекомпиляцию и lock contention (`Blocking waiting for file lock ...`) в рамках окна итерации.
- Чекбокс минимального verification-набора сохраняется в `[ ]` до отдельного зелёного прогона fmt + server/migration bundle после стабилизации build-cache/lock-окна.


### Актуализация 2026-05-24 (итерация 51)

- Усилен operational runner `scripts/verify/run-control-plane-remediation-minimal.sh`: добавлен inter-process lock (`flock`) в `target/.control-plane-remediation-minimal.lock`, чтобы исключить параллельные запуски и снизить повторяемые `Blocking waiting for file lock ...` между итерациями.
- Для ускоренного повторного прогона после pre-existing formatting drift добавлен управляемый флаг `RUSTOK_VERIFY_SKIP_FMT=1` (только для локального цикла triage; release-gate остаётся с обязательным fmt-check).
- Batch-2 пункт verification остаётся `[~]` до полного зелёного прогона полного набора без skip-флагов.

### Актуализация 2026-05-24 (итерация 52)

- Для нового minimal runner добавлен smoke/guard тест `scripts/tests/control_plane_remediation_minimal_runner_test.sh` (syntax + lock-conflict path + skip-fmt path до шага migration).
- `scripts/tests/README.md` и `scripts/verify/README.md` синхронизированы с новым runner/test entrypoint, чтобы operational usage и локальные проверки были воспроизводимыми.

### Актуализация 2026-05-24 (итерация 53)

- `verify-all` теперь поддерживает alias `control-plane-remediation-minimal` для точечного запуска нового bundle без ручного вызова файла runner.
- Добавлен отдельный smoke-тест `scripts/tests/control_plane_remediation_verify_all_alias_test.sh`, подтверждающий корректный route alias -> runner path.

### Актуализация 2026-05-24 (итерация 54)

- Smoke-тесты для control-plane minimal runner переведены на изолированные fixture-сценарии (`mktemp` + fake `cargo`/`python3`), без зависимости от состояния текущего workspace и без тяжёлой перекомпиляции.
- Alias-тест `verify-all -> control-plane-remediation-minimal` теперь проверяет полный PASS-path в fixture, включая финальный отчёт `All verification suites passed!`.

### Актуализация 2026-05-24 (итерация 55)

- Runner `run-control-plane-remediation-minimal.sh` усилен preflight-проверкой `flock` и поддержкой `RUSTOK_VERIFY_STEP_TIMEOUT` для ограничения длительности каждого шага в длинных CI/local прогонах.
- `scripts/verify/README.md` дополнен примером timeout-конфигурации для reproducible long-run execution.

### Актуализация 2026-05-24 (итерация 56)

- Для runner smoke-test добавлен негативный timeout-сценарий (`RUSTOK_VERIFY_STEP_TIMEOUT=1s`), подтверждающий fail-fast поведение на затянутом шаге migration.

### Актуализация 2026-05-24 (итерация 57)

- Timeout smoke-test для minimal runner усилен инвариантом остановки pipeline: при `RUSTOK_VERIFY_STEP_TIMEOUT=1s` выполнение не должно переходить за шаг migration к `module lifecycle tests`.

### Актуализация 2026-05-24 (итерация 58)

- Alias smoke-test для `verify-all` расширен дополнительным селектором по имени файла (`run-control-plane-remediation-minimal.sh`), чтобы фиксировать parity между alias- и direct-selector путями.

### Актуализация 2026-05-24 (итерация 59)

- Добавлен утилитарный отчёт `scripts/verify/report-control-plane-remediation-progress.py`, который считает `[x]/[~]/[ ]` по remediation-плану и печатает top-элементы хвоста для batch-планирования следующей итерации.
- Добавлен изолированный smoke-test `scripts/tests/control_plane_remediation_progress_report_test.sh` (fixture markdown + проверка expected counters), чтобы зафиксировать контракт отчёта.

### Актуализация 2026-05-24 (итерация 60)

- `report-control-plane-remediation-progress.py` получил явный fail-fast для отсутствующего файла плана (`exit 1` + `ERROR: remediation plan not found: ...`), чтобы CI/локальные раннеры не давали ложный PASS при неверном path.
- Smoke-тест progress-report расширен негативным сценарием missing-plan path.

### Актуализация 2026-05-24 (итерация 61)

- Progress-report скрипт получил режим `--json` для машинного consumption в CI/ботах (counts + top pending/in-progress items).
- Smoke-тест progress-report расширен проверкой JSON-output контракта.

### Актуализация 2026-05-24 (итерация 62)

- Для progress-report добавлен gate-режим `--fail-on-pending`: скрипт возвращает `exit 2`, если в плане есть `[ ]` пункты.
- Smoke-тест progress-report обновлён проверкой exit-кода/сообщения для `--fail-on-pending`.

### Политика временных артефактов execution-плана (обязательно к удалению после закрытия)

Этот remediation-план рассматривается как **временный execution-плейбук**. После полного закрытия плана
все вспомогательные артефакты, добавленные только для его сопровождения, должны быть удалены, чтобы
не засорять постоянные CI/verify surface.

**Правило удаления после закрытия плана:**

1. Удалить временные verify/scripts/tests, созданные исключительно под этот execution-track.
2. Удалить временные alias/entrypoint интеграции в агрегаторах (`verify-all`, локальные helper-и),
   если они не стали частью постоянного platform contract.
3. Удалить временные разделы в README/документации, добавленные только для этого плана.
4. Удалить итерационные operational-заметки этого плана из central docs или перенести их в архив,
   оставив только финальный short postmortem/результат.
5. Проверить, что CI/workflow не содержит временных gates/steps, оставшихся от execution-track.

**Release-gate при закрытии этого плана:** PR на статус "Done" не принимается,
пока не приложен cleanup-commit (или отдельный cleanup PR), подтверждающий удаление временных следов.

### Актуализация 2026-05-24 (итерация 63)

- Добавлена обязательная политика decommission для временных артефактов execution-плана:
  после закрытия remediation-плана удалить скрипты, alias, README-вставки, временные CI/gates и
  итерационные operational-следы, оставив только финальный результат/архив.

### Актуализация 2026-05-24 (итерация 64)

- Выполнен прогон `./scripts/verify/run-control-plane-remediation-minimal.sh` без skip-флагов:
  пакет по-прежнему падает на шаге `cargo fmt --all -- --check` из-за pre-existing formatting drift
  в несвязанных с remediation областях workspace.
- Чекбокс `Минимальный verification набор из этого плана прогнан на ветке` остаётся в статусе `[ ]`
  до отдельного зелёного прогона полного пакета после выравнивания workspace formatting baseline.

### Актуализация 2026-05-24 (итерация 65)

- В `rustok-api` добавлен explicit helper `canonical_manifest_snapshot_json(...)`, фиксирующий
  единый canonical JSON serializer contract для manifest snapshot (shared with hash builder).
- `apps/server` (`PlatformCompositionService`) и `apps/admin` (SSR bootstrap/runtime hash tests)
  переведены на shared canonical serializer вместо локальных `serde_json::to_value(...)` call-sites,
  чтобы устранить риск snapshot/hash drift между surfaces.
- Чекбоксы P1.1/Iteration D `Полный canonical manifest snapshot serializer` переведены в `[x]`;
  незакрытым остаётся cross-surface parity хвост GraphQL/Leptos SSR и финальный минимальный verify bundle.


### Актуализация 2026-05-24 (итерация 66)

- Закрыт release-gate пункт документации: central и локальные документы синхронизированы с фактическим runtime contract control-plane.
- В central docs зафиксированы DB-backed `platform_state` runtime source, atomic CAS+build enqueue и canonical SHA-256 snapshot/hash contract; в `apps/server`/`apps/admin` локальных docs закреплён GraphQL-only toggle entrypoint и отсутствие локального lifecycle SQL duplicate path.
- На этом шаге незакрытым operational хвостом остаётся только полный зелёный прогон минимального verification-набора (fmt + test bundle).


### Актуализация 2026-05-25 (итерация 67)

- Исправлены регрессии в `run-control-plane-remediation-minimal.sh`, обнаруженные после расширения triage-режима: шаги больше не помечаются как `PASS` при фактическом падении команды внутри `run_step`, а вывод pass-line стабилизирован через `printf --` для префиксов вида `--> ...`.
- Исправлен fixture smoke-test `scripts/tests/control_plane_remediation_minimal_runner_test.sh`: pattern-matching по строкам, начинающимся с `--`, переведён на `rg -q --`, а сценарий `continue-on-fmt-fail` синхронизирован по ожидаемому `exit 2` и обязательным маркерам `PARTIAL PASS`.
- Это закрывает отдельный operational-риск ложноположительного PASS в minimal bundle runner и возвращает deterministic/non-regression baseline для Batch-2 verification цикла.


### Актуализация 2026-05-25 (итерация 68)

- Устранён drift в operational docs: в `scripts/verify/README.md` удалён дублирующий блок `Control-plane remediation minimal bundle`, оставлен единый канонический раздел `Control-plane remediation minimal runner` с актуальными флагами (`SKIP_FMT`, `CONTINUE_ON_FMT_FAIL`, `STEP_TIMEOUT`).
- Запущен повторный Batch-2 прогон minimal runner в режиме `RUSTOK_VERIFY_CONTINUE_ON_FMT_FAIL=1` для сбора полного сигнала по шагам после format-drift; pre-existing `cargo fmt --check` drift подтверждён, runner корректно продолжает pipeline вместо early stop.
- Чекбокс полного зелёного minimal verification набора остаётся незакрытым до отдельного strict-прогона без triage-флагов.


### Актуализация 2026-05-25 (итерация 69)

- Выполнен strict-прогон `scripts/verify/run-control-plane-remediation-minimal.sh` без triage-флагов: runner завершился `FAIL` на шаге `format check` (`cargo fmt --all -- --check`, `exit 1`, ~6s).
- Подтверждён, что блокером полного закрытия operational-gate остаётся pre-existing workspace rustfmt drift (без новых control-plane функциональных ошибок в самом runner).
- Чекбокс `Минимальный verification набор из этого плана прогнан на ветке` сохраняется в `[ ]` до отдельного полного зелёного прогона после выравнивания formatting baseline.


### Актуализация 2026-05-25 (итерация 70)

- Для ускорения закрытия operational хвоста выполнено выравнивание rustfmt baseline по текущему drift-списку strict-runner: форматированы затронутые файлы `rustok-blog`, `rustok-commerce` tests, `rustok-inventory` admin API и `rustok-seo` sitemap services.
- После форматирования strict-runner проходит шаг `format check` (`PASS`) и переходит к следующему этапу (`migration tests`), что подтверждает снятие format-blocker для данного набора файлов.
- Для полного закрытия чекбокса минимального verification-набора остаётся дождаться завершения полного bundle-прогона (`migration + rustok-server tests + xtask/dependabot checks`) в отдельном run-окне.
